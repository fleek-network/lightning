//! Encrypt a file for a shared network public key

use std::io::{stdin, stdout, Read, Write};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use bip32::{ChildNumber, XPub};
use bpaf::{literal, positional, Bpaf, Parser};

/// Parse public key argument
fn pubkey() -> impl Parser<XPub> {
    // TODO: replace with real key once bip32 changes are deployed
    const DUMMY_KEY: &str = "xpub6AD3KdpTj2YsdWEX8rf2i9ym49bWek3fL5oTvPTvawZh3n3rNrcUWuUJg7DAj6qoNBuNnn8AoBwPtFuyZBYwyn7YwyZAAcgRhGAqv6EWLk7";

    bpaf::short('p')
        .long("pubkey")
        .argument::<String>("XPUB")
        .help("Shared extended public key (BIP32 XPUB)")
        .fallback(DUMMY_KEY.into())
        .display_fallback()
        .parse(|s| XPub::from_str(&s).context("invalid extended public key"))
}

/// Parse an input
fn input() -> impl Parser<Input> {
    let path = positional::<PathBuf>("FILE")
        .help("Path to a file to read and encrypt")
        .guard(|p| p.exists(), "file not found")
        .map(Input::File);
    let dash = literal("-")
        .help("Read and encrypt from stdin")
        .map(|_| Input::Stdin);
    bpaf::construct!([path, dash])
}

#[derive(Debug, Clone)]
enum Input {
    File(PathBuf),
    Stdin,
}

impl Input {
    fn read(self) -> anyhow::Result<(PathBuf, Vec<u8>)> {
        match self {
            Input::Stdin => {
                let mut buf = Vec::new();
                stdin()
                    .read_to_end(&mut buf)
                    .map(|_| ("stdin".into(), buf))
                    .context("failed to read from stdin")
            },
            Input::File(file) => std::fs::read(&file)
                .map(|d| (file, d))
                .context("failed to read file"),
        }
    }
}

/// Output modes
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

#[derive(Debug, Clone, Bpaf)]
#[bpaf(
    options,
    version,
    descr(env!("CARGO_PKG_DESCRIPTION")),
    fallback_to_usage
)]
enum SgxEncrypt {
    /// Encrypt wasm modules for the shared key directly
    #[bpaf(command("module"), fallback_to_usage)]
    WasmModule {
        #[bpaf(external)]
        pubkey: XPub,
        #[bpaf(external)]
        output: Output,
        /// Path to input file to encrypt.
        #[bpaf(external)]
        input: Input,
    },
    /// Encrypt data to be accessed inside multiple wasm modules
    #[bpaf(command("shared"), fallback_to_usage)]
    SharedData {
        #[bpaf(external)]
        pubkey: XPub,
        #[bpaf(external)]
        output: Output,
        /// Path to input file to encrypt.
        #[bpaf(external)]
        input: Input,
        /// List of approved wasm modules to access the data
        #[bpaf(
            positional::<String>("WASM HASH"),
            parse(|s| {
                let v = hex::decode(s).context("invalid blake3 hex")?;
                v.try_into().map_err(|_| anyhow!("invalid blake3 hex length"))
            }),
            some("At least one approved hash is required"),

        )]
        approved: Vec<[u8; 32]>,
    },
    /// Encrypt data for a specific wasm module's derived key
    #[bpaf(command("wasm"), fallback_to_usage)]
    WasmData {
        /// Optional hex-encoded wasm key derivation path
        #[bpaf(
            short,
            long("drv"),
            argument::<String>("DERIVATION"),
            parse(hex::decode),
            guard(
                |v| v.len() <= 256 || v.len() % 2 != 0,
                "Key derivation path length must be <= 256 and even"
            ),
            optional
        )]
        derivation_path: Option<Vec<u8>>,
        #[bpaf(external)]
        pubkey: XPub,
        #[bpaf(external)]
        output: Output,
        /// Wasm module to encrypt data for
        #[bpaf(
            positional::<String>("WASM HASH"),
            parse(|s| {
                let v = hex::decode(s).context("invalid blake3 hex")?;
                v.try_into().map_err(|_| anyhow!("invalid blake3 hex length"))
            }),
        )]
        wasm_module: [u8; 32],
        /// Path to input file to encrypt.
        #[bpaf(external)]
        input: Input,
    },
}

impl SgxEncrypt {
    /// Derive encryption key, encrypt the input, and write the output
    fn execute(self) -> anyhow::Result<()> {
        let (pubkey, input, bytes, output) = match self {
            SgxEncrypt::WasmModule {
                pubkey,
                input,
                output,
            } => {
                // No derivation or encoding logic is needed for modules themselves
                let (file, bytes) = input.read()?;
                (pubkey.to_bytes(), file, bytes, output)
            },
            SgxEncrypt::SharedData {
                pubkey,
                approved,
                input,
                output,
            } => {
                const HEADER_PREFIX: [u8; 27] = *b"FLEEK_ENCLAVE_APPROVED_WASM";

                let (file, mut bytes) = input.read()?;

                // Encode header + content as plaintext
                let mut buf =
                    Vec::with_capacity(HEADER_PREFIX.len() + 1 + approved.len() * 32 + bytes.len());

                // write header
                buf.extend_from_slice(&HEADER_PREFIX);
                buf.push(approved.len() as u8);
                for hash in approved {
                    buf.extend_from_slice(&hash);
                }

                // append user data
                buf.append(&mut bytes);

                (pubkey.to_bytes(), file, buf, output)
            },
            SgxEncrypt::WasmData {
                pubkey,
                wasm_module,
                derivation_path,
                input,
                output,
            } => {
                let (file, bytes) = input.read()?;

                // Derive wasm hash scope
                let mut pk = pubkey.derive_child(bip32::ChildNumber(0)).unwrap();
                for n in wasm_module
                    .chunks_exact(2)
                    .map(|v| u16::from_be_bytes(v.try_into().unwrap()).into())
                {
                    pk = pk.derive_child(ChildNumber(n))?;
                }

                // If provided, derive user path as chunks of unsized 16 bit integers
                if let Some(path) = derivation_path {
                    for n in path
                        .chunks(2)
                        .map(|v| u16::from_be_bytes(v.try_into().unwrap()).into())
                    {
                        pk = pk.derive_child(bip32::ChildNumber(n))?;
                    }
                }

                (pk.to_bytes(), file, bytes, output)
            },
        };

        // Encrypt the content
        let cipher = ecies::encrypt(&pubkey, &bytes)
            .map_err(|e| anyhow!("failed to encrypt content: {e}"))?;

        // Write the output to a file or stdout
        match output {
            Output::Stdout => stdout().write_all(&cipher)?,
            Output::File { path } => {
                let path = path.unwrap_or(input.with_extension("cipher"));
                std::fs::write(path, cipher).context("failed to write file")?;
            },
        }

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    sgx_encrypt().run().execute()
}
