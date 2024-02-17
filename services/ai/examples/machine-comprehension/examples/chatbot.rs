#![allow(unused)]

use std::collections::HashMap;
use std::io;
use std::io::{Cursor, Write};

use anyhow::bail;
use base64::Engine;
use bytes::Bytes;
use common::{to_array_d, Output};
use ndarray::{array, concatenate, s, Array1, Axis};
use ndarray_npy::WriteNpyExt;
use reqwest::Body;
use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;

const EOS: usize = 50256;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut stdout = io::stdout();

    let tokenizer =
        Tokenizer::from_file("/Users/acadia/models/dialogpt-medium/tokenizer.json").unwrap();

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

        let mut history = String::new();
        history.push_str(&input);
        history.push_str(" ");

        print!("Bot > ");
        stdout.flush().unwrap();

        'inner: loop {
            // Create encoding from current history.
            let encoding = tokenizer.encode(history.as_str(), true).unwrap();

            // Build inputs.

            // Gather attention mask.
            let attention_mask = encoding
                .get_attention_mask()
                .to_vec()
                .into_iter()
                .map(|mask| mask as i64)
                .collect::<Vec<_>>();
            let attention_mask = Array1::from(attention_mask);
            let attention_mask = attention_mask.view().insert_axis(Axis(0));
            let mut buffer_attention_mask = Vec::new();
            attention_mask.write_npy(Cursor::new(&mut buffer_attention_mask))?;

            // Gather position ids.
            let position_ids = encoding
                .get_word_ids()
                .to_vec()
                .into_iter()
                .map(|id| id.unwrap() as i64)
                .collect::<Vec<_>>();
            let position_ids = Array1::from(position_ids);
            let position_ids = position_ids.view().insert_axis(Axis(0));
            let mut buffer_position_ids = Vec::new();
            position_ids.write_npy(Cursor::new(&mut buffer_position_ids))?;

            // Todo: This might not be needed.
            // Get tokens attention mask.
            let tokens = encoding
                .get_ids()
                .iter()
                .map(|i| *i as i64)
                .collect::<Vec<_>>();
            let mut tokens = Array1::from_iter(tokens.iter().cloned());

            // Gather input ids.
            let input_ids = tokens.view().insert_axis(Axis(0));
            let mut buffer_input_ids = Vec::new();
            input_ids.write_npy(Cursor::new(&mut buffer_input_ids))?;

            // Build payload.
            let mut inputs = HashMap::new();
            inputs.insert("input_ids".to_string(), buffer_input_ids.into());
            inputs.insert("attention_mask".to_string(), buffer_attention_mask.into());
            inputs.insert("position_ids".to_string(), buffer_position_ids.into());
            let payload = serde_json::to_string(&Input::Map(inputs))?;

            // Send service a request.
            let resp = reqwest::Client::new().post("http://127.0.0.1:4220/services/2/blake3/387cbc21bd420764043db21330ccfbaaceafa9aa6c858a0cc16d8fc611c0dbb8")
                .body(Body::from(payload.into_bytes()))
                .send().await?;

            if !resp.status().is_success() {
                let msg = String::from_utf8(resp.bytes().await?.into())?;
                bail!("invalid response status: {:?}", msg);
            }

            // Process json response and prepare output.
            let data = resp.bytes().await?;
            let outputs: Output = serde_json::from_slice(data.as_ref())?;

            // Decode output.
            let npy = base64::prelude::BASE64_STANDARD
                .decode(outputs.outputs.get("logits").unwrap())
                .unwrap();
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

            // Greedy search or beam search?
            let token = probabilities[0].0;

            // Add new token to history.
            tokens = concatenate![Axis(0), tokens, array![token.try_into().unwrap()]];

            // Check model token.
            if token == EOS {
                // Todo: What do we do here?
                break 'inner;
            }

            // Decode token.
            let token_str = tokenizer.decode(&[token as _], true).unwrap();

            // Add to history.
            history.push_str(&format!(" {token_str}"));

            // Display to user.
            print!("{}", token_str);
            stdout.flush().unwrap();
        }
        println!();
    }

    println!();

    Ok(())
}

#[derive(Deserialize, Serialize)]
pub enum Input {
    Raw(Bytes),
    Map(HashMap<String, Bytes>),
}
