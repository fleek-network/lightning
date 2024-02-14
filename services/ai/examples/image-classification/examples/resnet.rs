use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use anyhow::bail;
use base64::Engine;
use example_image_classification::imagenet::CLASSES;
use example_image_classification::to_array_d;
use image::GenericImageView;
use ndarray::{Array, Axis};
use ndarray_npy::WriteNpyExt;
use reqwest::Body;
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    args.next();

    let Some(path) = args.next() else {
        println!("missing image path argument");
        std::process::exit(1);
    };

    // Open image.
    let original_img = image::open(Path::new(&path))?;

    // Preprocessing.
    let img = original_img.thumbnail(224, 224);
    let mut input = Array::zeros((1, 3, 224, 224));
    for pixel in img.pixels() {
        let x = pixel.0 as _;
        let y = pixel.1 as _;
        let [r, g, b, _] = pixel.2.0;
        input[[0, 0, y, x]] = (r as f32) / 255.;
        input[[0, 1, y, x]] = (g as f32) / 255.;
        input[[0, 2, y, x]] = (b as f32) / 255.;
    }

    // Encode input into npy.
    let mut buffer = Vec::new();
    input.write_npy(Cursor::new(&mut buffer))?;

    // Send service a request.
    let resp = reqwest::Client::new().post("http://127.0.0.1:4220/services/2/blake3/f2700c0d695006d953ca920b7eb73602b5aef7dbe7b6506d296528f13ebf0d95")
        .body(Body::from(buffer))
        .send().await?;

    if !resp.status().is_success() {
        let msg = String::from_utf8(resp.bytes().await?.into())?;
        bail!("invalid response status: {:?}", msg);
    }

    // Process json response and prepare output.
    let data = resp.bytes().await?;
    let outputs: Output = serde_json::from_slice(data.as_ref())?;

    // Decode output.
    let npy = base64::prelude::BASE64_STANDARD.decode(outputs.outputs.get("output").unwrap())?;
    let npy_file = npyz::NpyFile::new(Cursor::new(npy)).unwrap();

    // Conver to ndarray::Array.
    let shape = npy_file.shape().to_vec();
    let order = npy_file.order();
    let data = npy_file.into_vec::<f32>().unwrap();
    let output = to_array_d(data, shape, order);

    // Shape is (1, 1000) so drop first axis.
    let output = output.remove_axis(Axis(0));

    let mut output = output.into_iter().enumerate().collect::<Vec<_>>();
    output.sort_by(|(_, v1), (_, v2)| v2.partial_cmp(v1).unwrap());

    // Take the first 3 best guesses.
    let output = output
        .into_iter()
        .take(3)
        .map(|(i, v)| (v, CLASSES[i]))
        .collect::<Vec<_>>();
    println!("{:?}", output);

    Ok(())
}

#[derive(Deserialize, Serialize)]
pub struct Output {
    pub format: String,
    pub outputs: HashMap<String, String>,
}
