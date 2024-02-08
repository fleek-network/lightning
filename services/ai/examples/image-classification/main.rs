// This example requires `mode_resnet18.pt` to be stored in the node.
use std::io::Cursor;

use reqwest::Body;
use tch::vision::imagenet;
use tch::Tensor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    args.next();

    let Some(path) = args.next() else {
        println!("missing image path argument");
        std::process::exit(1);
    };

    let img = tokio::fs::read(path).await?;
    let input = imagenet::load_image_and_resize224_from_memory(&img)?;
    let input = input.unsqueeze(0);

    let mut buffer = Vec::new();
    input.save_to_stream(Cursor::new(&mut buffer))?;

    let response = reqwest::Client::new()
        .post("http://127.0.0.1:4220/services/2/infer/blake3/fb7c113fd7f7b9b284eba5df851210d8f7138855f48725242bf4324a0877d4d0")
        .body(Body::from(buffer))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Received {}", response.status(),)
    }

    let response_data = response.bytes().await?;
    let output = Tensor::load_from_stream(Cursor::new(response_data))?;

    println!("{:?}", imagenet::top(&output, 2));
    Ok(())
}
