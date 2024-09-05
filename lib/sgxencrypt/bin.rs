//! Encrypt a file for a shared network public key

use std::io::{stdout, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use bpaf::Bpaf;
use ecies::PublicKey;

/// Output modes:
#[derive(Debug, Clone, Bpaf)]
enum Output {
    File {
        /// Optional path to write encrypted output to. [default: *.cipher]
        #[bpaf(short('o'), long("output"), argument("PATH"))]
        path: Option<PathBuf>,
    },
    /// Enable writing output directly to stdout.
    #[bpaf(long)]
    Stdout,
}

#[derive(Debug, Bpaf)]
#[bpaf(options, version, descr(env!("CARGO_PKG_DESCRIPTION")))]
struct Args {
    /// Hex-encoded network sealing public key. Should be in compressed format.
    #[bpaf(
        short,
        long,
        argument::<String>("PUBKEY"),
        fallback("03e1d7803dfa7e5d3ca4b4afa99073caf57b9afcede3b416ad29bb9ce9e4fe0f86".into()),
        display_fallback,
        parse(|s| {
            let bytes = hex::decode(&s).context("invalid base58")?;
            let slice = bytes.try_into().map_err(|_| anyhow!("invalid key length"))?;
            PublicKey::parse_compressed(&slice).map_err(|e| anyhow!("invalid public key: {e}"))
        })
    )]
    pubkey: PublicKey,

    #[bpaf(external)]
    output: Output,

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
    match output {
        Output::Stdout => stdout().write_all(&cipher)?,
        Output::File { path } => {
            let path = path.unwrap_or(input.with_extension("cipher"));
            std::fs::write(path, cipher).context("failed to write file")?;
        },
    }

    Ok(())
}
