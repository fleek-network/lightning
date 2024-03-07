// You need to store the model on the node.
// Export the model along with config files using the optimum cli.
// See https://huggingface.co/docs/transformers/en/serialization#exporting-a--transformers-model-to-onnx-with-cli.
// You can find more info about the model here https://huggingface.co/microsoft/DialoGPT-medium.
// `tokenizer.json` should be included when you export the model.
use std::io;
use std::io::Write;
use std::net::SocketAddr;

use cdk_rust::schema::ResponseFrame;
use cdk_rust::transport::tcp::TcpTransport;
use cdk_rust::Builder;
use ndarray::{s, Array1, Axis};
use safetensors::{Dtype, SafeTensors};
use safetensors_ndarray::collection::Collection;
use tokenizers::Tokenizer;

const EOS: usize = 50256;

#[tokio::main]
async fn main() {
    let tokenizer =
        Tokenizer::from_file("/Users/acadia/models/dialogpt-medium-texttask/tokenizer.json")
            .unwrap();

    let target: SocketAddr = "127.0.0.1:4221".parse().unwrap();
    let transport = TcpTransport::new(target);
    let connector = Builder::primary([0u8; 32], 2)
        .transport(transport)
        .build()
        .unwrap();
    let (mut sender, mut receiver) = connector.connect().await.unwrap().split();

    // Start the session.
    let start_session = serde_json::to_string(&serde_json::json!( {
        "model": "387cbc21bd420764043db21330ccfbaaceafa9aa6c858a0cc16d8fc611c0dbb8".to_string(),
        "origin": "blake3",
        "device": "cpu",
        "content_format": "bin",
        "model_io_encoding": "safetensors"
    }))
    .unwrap();
    sender
        .send(start_session.into_bytes().into())
        .await
        .unwrap();

    let mut stdout = io::stdout();

    loop {
        print!("User > ");
        stdout.flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");
        input = input.trim().to_string();

        if input == "q" || input == "quit" {
            break;
        }

        let mut conversation = String::new();
        // This token is also the BOS.
        conversation.push_str("<|endoftext|>");
        conversation.push_str(&input);
        conversation.push_str("<|endoftext|>");

        print!("Bot > ");
        stdout.flush().unwrap();

        'inner: loop {
            // Create encoding from current conversation.
            let encoding = tokenizer.encode(conversation.as_str(), true).unwrap();

            let mut safetensors = Collection::new();

            // Gather attention mask.
            let attention_mask = encoding
                .get_attention_mask()
                .iter()
                .copied()
                .map(|mask| mask as i64)
                .collect::<Vec<_>>();
            let attention_mask = Array1::from(attention_mask).insert_axis(Axis(0));
            safetensors.insert_array_i64("attention_mask".to_string(), attention_mask.into_dyn());

            // Gather position ids.
            let position_ids = encoding
                .get_word_ids()
                .iter()
                .copied()
                .map(|id| id.unwrap() as i64)
                .collect::<Vec<_>>();
            let position_ids = Array1::from(position_ids).insert_axis(Axis(0));
            safetensors.insert_array_i64("position_ids".to_string(), position_ids.into_dyn());

            // Gather input ids.
            let tokens = encoding
                .get_ids()
                .iter()
                .map(|i| *i as i64)
                .collect::<Vec<_>>();
            let tokens = Array1::from_iter(tokens.iter().cloned());
            let input_ids = tokens.view().insert_axis(Axis(0)).to_owned();
            safetensors.insert_array_i64("input_ids".to_string(), input_ids.into_dyn());

            // Send service a request.
            let serialized_input = safetensors.serialize(&None).unwrap();
            sender.send(serialized_input.into()).await.unwrap();

            // Read response frame.
            let resp = receiver.recv().await.unwrap().unwrap();

            // Derive output array from response data.
            let output = match resp {
                ResponseFrame::ServicePayload { bytes } => {
                    let outputs = SafeTensors::deserialize(bytes.as_ref()).unwrap();
                    let view = outputs.tensor("logits").unwrap();
                    assert_eq!(view.dtype(), Dtype::F32);
                    safetensors_ndarray::utils::deserialize_f32(view.shape(), view.data()).unwrap()
                },
                ResponseFrame::Termination { reason } => {
                    panic!("service terminated the connection: {reason:?}")
                },
                _ => panic!("expected a service payload frame"),
            };

            // Convert to ndarray::Array.
            let generated_tokens = output.view();

            // Sort logits.
            let probabilities = &mut generated_tokens
                .slice(s![0, -1, ..])
                .insert_axis(Axis(0))
                .to_owned()
                .iter()
                .cloned()
                .enumerate()
                .collect::<Vec<_>>();
            probabilities
                .sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Less));

            // Greedy search. We could implement beam search.
            let token = probabilities[0].0;

            // The bot is done talking.
            if token == EOS {
                break 'inner;
            }

            // Decode token.
            let token_str = tokenizer.decode(&[token as _], true).unwrap();

            // Add to history.
            conversation.push_str(&token_str);

            // Print next token from bot.
            print!("{}", token_str);
            stdout.flush().unwrap();
        }
        println!();
    }

    println!();
}
