use std::path::PathBuf;

/// Heap size for enclave
const HEAP_SIZE: &str = "0x100000000"; // 4 GiB
/// Stack size for enclave
const STACK_SIZE: &str = "0x200000"; // 2 MiB
/// Maximum number of threads to reserve for the enclave
/// 1 main + 1 worker + 1 tls + 1 mtls + 128 wasm
const THREADS: &str = "134";

/// URL of latest enclave
/// TODO: Also download mrsigner signature and get checksum
const ENCLAVE_URL: &str =
    "https://bafybeifepixyjdgq5cfyvb4tlxfezkgwmyy6gvnua45pceh6fuxkbvwtly.ipfs.flk-ipfs.xyz";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=enclave.sgxs");
    println!("cargo::rerun-if-env-changed=FN_ENCLAVE_SOURCE");
    println!("cargo::rerun-if-env-changed=FN_ENCLAVE_SGXS");

    // Build from source
    if let Ok(path) = std::env::var("FN_ENCLAVE_SOURCE") {
        if !path.is_empty() {
            let path = PathBuf::from(path);

            if !path.is_dir() {
                panic!("enclave source must be a directory")
            }
            if path.is_relative() {
                panic!("enclave path must be absolute")
            }

            println!("cargo::rerun-if-changed={}/*", path.to_string_lossy());

            // local build for the enclave bin
            assert!(
                std::process::Command::new("cargo")
                    .args(["build", "--release"])
                    .current_dir(&path)
                    .status()
                    .unwrap()
                    .success(),
                "failed to build enclave module"
            );

            // cargo output path
            let bin = path.join("target/x86_64-fortanix-unknown-sgx/release/lightning-pod-enclave");

            // convert `enclave` to `enclave.sgxs`
            assert!(
                std::process::Command::new("ftxsgx-elf2sgxs")
                    .arg(&bin)
                    .args([
                        "--heap-size",
                        HEAP_SIZE,
                        "--stack-size",
                        STACK_SIZE,
                        "--threads",
                        THREADS,
                    ])
                    .status()
                    .expect("failed to find ftxsgx-elf2sgxs binary")
                    .success(),
                "failed to convert elf to sgxs"
            );

            // copy new enclave into the project
            std::fs::copy(bin.with_extension("sgxs"), "./enclave.sgxs")
                .unwrap_or_else(|_| panic!("failed to copy enclave to output directory"));

            return;
        }
    }

    // Use precompiled enclave
    if let Ok(path) = std::env::var("FN_ENCLAVE_SGXS") {
        if !path.is_empty() {
            if !PathBuf::from(&path).is_file() {
                panic!("enclave must be a file");
            }

            println!("cargo::rerun-if-changed={}", path);
            std::fs::copy(path, "./enclave.sgxs").expect("failed to copy provided enclave.sgx");

            return;
        }
    }

    // If enclave is not provided, fetch latest precompile from the specified url
    if !PathBuf::from("./enclave.sgxs").is_file() {
        let mut buf = Vec::new();
        ureq::get(ENCLAVE_URL)
            .send_bytes(&[])
            .expect("failed to download enclave.sgxs")
            .into_reader()
            .read_to_end(&mut buf)
            .expect("failed to download enclave.sgxs");

        // TODO: verify checksum

        std::fs::write("./enclave.sgxs", buf).expect("failed to write enclave.sgxs to disk");
    }
}
