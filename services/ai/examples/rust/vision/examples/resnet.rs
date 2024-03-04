use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use common::imagenet::CLASSES;
use common::service_api::{EncodedArrayExt, Input};
use common::to_array_d;
use image::GenericImageView;
use ndarray::{Array, Axis};
use ndarray_npy::WriteNpyExt;

#[tokio::main]
async fn main() {
    let mut args = std::env::args();
    args.next();

    let Some(path) = args.next() else {
        println!("missing image path argument");
        std::process::exit(1);
    };

    // Open image.
    let original_img = image::open(Path::new(&path)).unwrap();

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
    input.write_npy(Cursor::new(&mut buffer)).unwrap();

    let payload_buffer = rmp_serde::to_vec_named(&Input::Array {
        data: EncodedArrayExt((10, buffer.into())),
    })
    .unwrap();

    // Send service a request.
    let resp = reqwest::Client::new().post("http://127.0.0.1:4220/services/2/blake3/f2700c0d695006d953ca920b7eb73602b5aef7dbe7b6506d296528f13ebf0d95")
        .body(payload_buffer)
        .send().await.unwrap();

    if !resp.status().is_success() {
        let msg = String::from_utf8(resp.bytes().await.unwrap().into()).unwrap();
        panic!("invalid response status: {:?}", msg);
    }

    // Process response and extract encoded array.
    let data = resp.bytes().await.unwrap();
    let mut outputs =
        rmp_serde::from_slice::<HashMap<String, EncodedArrayExt>>(data.as_ref()).unwrap();
    let EncodedArrayExt((encoding, encoded_array)) = outputs.remove("output").unwrap();
    // Assert that the array was encoded as a npy file.
    assert_eq!(encoding, 10);

    // Decode output.
    let npy_file = npyz::NpyFile::new(Cursor::new(encoded_array)).unwrap();

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
}
