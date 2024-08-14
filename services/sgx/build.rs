use std::path::Path;

/// Heap size for enclave
const HEAP_SIZE: &str = "0x20000000"; // 512 MiB
/// Stack size for enclave
const STACK_SIZE: &str = "0x1000000"; // 10 MiB
/// Number of threads to support in enclave
const THREADS: &str = "8";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=enclave/*");
    println!("cargo::rerun-if-env-changed=FN_ENCLAVE_BIN_PATH");

    let path = std::env::var("FN_ENCLAVE_BIN_PATH")
        .ok()
        .and_then(|v| (!v.is_empty()).then_some(v))
        .unwrap_or_else(|| {
            // local build for the enclave bin
            assert!(
                std::process::Command::new("cargo")
                    .args(["build", "--release", "--locked"])
                    .current_dir("./enclave")
                    .env_clear()
                    .env("PATH", std::env::var("PATH").unwrap())
                    .status()
                    .unwrap()
                    .success(),
                "failed to build enclave module"
            );

            // cargo output path
            "enclave/target/x86_64-fortanix-unknown-sgx/release/fleek-service-sgx-enclave".into()
        });

    let new_path = Path::new(&std::env::var("OUT_DIR").unwrap()).join("enclave");
    std::fs::copy(&path, &new_path)
        .unwrap_or_else(|_| panic!("failed to copy {path} to output directory"));

    // convert `enclave` to `enclave.sgxs`
    assert!(
        std::process::Command::new("ftxsgx-elf2sgxs")
            .arg(&new_path)
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
}
