// You need to store the model on the node.
// Export the model along with config files using the optimum cli.
// See https://huggingface.co/docs/transformers/en/serialization#exporting-a--transformers-model-to-onnx-with-cli.
// You can find more info about the model here https://huggingface.co/microsoft/DialoGPT-medium.
// `tokenizer.json` should be included when you export the model.
use std::io;
use std::io::{Cursor, Write};
use std::net::SocketAddr;

use anyhow::{anyhow, bail};
use base64::Engine;
use bson::spec::BinarySubtype;
use bson::{Binary, Bson, Document};
use cdk_rust::schema::ResponseFrame;
use cdk_rust::transport::tcp::TcpTransport;
use cdk_rust::Builder;
use common::service_api::{Device, Origin, StartSession};
use common::to_array_d;
use ndarray::{s, Array1, Axis};
use ndarray_npy::WriteNpyExt;
use tokenizers::Tokenizer;

const EOS: usize = 50256;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tokenizer = Tokenizer::from_file("tokenizer.json").unwrap();

    let target: SocketAddr = "127.0.0.1:4221".parse()?;
    let transport = TcpTransport::new(target);
    let connector = Builder::primary([0u8; 32], 2)
        .transport(transport)
        .build()?;
    let (mut sender, mut receiver) = connector.connect().await?.split();

    // Start the session.
    let start_session = StartSession {
        model: "387cbc21bd420764043db21330ccfbaaceafa9aa6c858a0cc16d8fc611c0dbb8".to_string(),
        origin: Origin::Blake3,
        device: Device::Cpu,
    };
    sender.send(bson::to_vec(&start_session)?.into()).await?;

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

            // Gather attention mask.
            let attention_mask = encoding
                .get_attention_mask()
                .iter()
                .copied()
                .map(|mask| mask as i64)
                .collect::<Vec<_>>();
            let attention_mask = Array1::from(attention_mask);
            let attention_mask = attention_mask.view().insert_axis(Axis(0));
            let mut buffer_attention_mask = Vec::new();
            attention_mask.write_npy(Cursor::new(&mut buffer_attention_mask))?;

            // Gather position ids.
            let position_ids = encoding
                .get_word_ids()
                .iter()
                .copied()
                .map(|id| id.unwrap() as i64)
                .collect::<Vec<_>>();
            let position_ids = Array1::from(position_ids);
            let position_ids = position_ids.view().insert_axis(Axis(0));
            let mut buffer_position_ids = Vec::new();
            position_ids.write_npy(Cursor::new(&mut buffer_position_ids))?;

            // Gather input ids.
            let tokens = encoding
                .get_ids()
                .iter()
                .map(|i| *i as i64)
                .collect::<Vec<_>>();
            let tokens = Array1::from_iter(tokens.iter().cloned());
            let input_ids = tokens.view().insert_axis(Axis(0));
            let mut buffer_input_ids = Vec::new();
            input_ids.write_npy(Cursor::new(&mut buffer_input_ids))?;

            // Build payload.
            let inputs = bson::bson!({
                "input_ids": Bson::Binary(Binary{ subtype: BinarySubtype::Generic, bytes: buffer_input_ids }),
                "attention_mask": Bson::Binary(Binary{ subtype: BinarySubtype::Generic, bytes: buffer_attention_mask }),
                "position_ids": Bson::Binary(Binary{ subtype: BinarySubtype::Generic, bytes: buffer_position_ids })
            });
            let bson = bson::bson!({
                "type": "map",
                "encoding": "npy",
                "data": inputs,
            });
            let payload = bson::to_vec(&bson)?;

            // Send service a request.
            sender.send(payload.into()).await?;
            let resp = receiver
                .recv()
                .await
                .ok_or(anyhow!("expected a response from the service"))??;
            let npy = match resp {
                ResponseFrame::ServicePayload { bytes } => {
                    // Process json response and prepare output.
                    let document = Document::from_reader(Cursor::new(bytes))?;
                    let outputs = document.get_document("outputs")?;
                    base64::prelude::BASE64_STANDARD
                        .decode(outputs.get_str("logits")?)
                        .unwrap()
                },
                ResponseFrame::Termination { reason } => {
                    bail!("service terminated the connection: {reason:?}")
                },
                _ => bail!("expected a service payload frame"),
            };

            // Decode output.
            let npy_file = npyz::NpyFile::new(Cursor::new(npy)).unwrap();

            // Convert to ndarray::Array.
            let shape = npy_file.shape().to_vec();
            let order = npy_file.order();
            let data = npy_file.into_vec::<f32>().unwrap();
            let output = to_array_d(data, shape, order);
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

    Ok(())
}
