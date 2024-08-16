//! Encrypt a file for a shared network public key

use std::path::PathBuf;

use anyhow::{anyhow, Context};
use bpaf::Bpaf;
use ecies::PublicKey;

#[derive(Debug, Bpaf)]
#[bpaf(options)]
struct Args {
    /// Base58 shared network public key. Should be in compressed format.
    #[bpaf(
        short,
        long,
        argument::<String>,
        fallback("27fjvoWaGcupCpT9ZMfok4gAHGcUhuFt1wgpoVjb4Bhka".into()),
        display_fallback,
        parse(|s| {
            let bytes = bs58::decode(&s)
                .into_vec()
                .context("invalid base58")?;
            let slice = bytes.as_slice()
                .try_into()
                .context("invalid key length")?;
            PublicKey::parse_compressed(&slice)
                .map_err(|e| anyhow!("invalid public key: {e}"))
        })
    )]
    pubkey: PublicKey,

    /// Optional path to write encrypted output to. [default: *.cipher]
    #[bpaf(short, long)]
    output: Option<PathBuf>,

    /// Path to input file to encrypt.
    #[bpaf(
        positional::<PathBuf>("PATH"),
        guard(|p| p.exists(), "file not found"),
        parse(|p| {
            std::fs::read(&p)
                .map(|b| (p, b))
                .context("failed to read file")
        })
    )]
    input: (PathBuf, Vec<u8>),
}

fn main() -> anyhow::Result<()> {
    let Args {
        pubkey,
        output,
        input: (input, bytes),
    } = args().fallback_to_usage().run();

    // Encrypt file
    let cipher = ecies::encrypt(&pubkey.serialize_compressed(), &bytes)
        .map_err(|e| anyhow!("failed to encrypt data: {e}"))?;

    // Write the file
    std::fs::write(output.unwrap_or(input.with_extension("cipher")), cipher)
        .context("failed to write file")
}
