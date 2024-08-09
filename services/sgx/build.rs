use std::path::Path;

/// Heap size for enclave
const HEAP_SIZE: &str = "0x20000000"; // 512 MiB
/// Stack size for enclave
const STACK_SIZE: &str = "0x1000000"; // 10 MiB
/// Number of threads to support in enclave
const THREADS: &str = "8";

/// Output binary path from build
const CARGO_OUTPUT: &str =
    "../../target/x86_64-fortanix-unknown-sgx/release/fleek-service-sgx-enclave";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=enclave/*");

    let out_dir = std::env::var("OUT_DIR").unwrap();

    assert!(
        std::process::Command::new("cargo")
            .args(["build", "--release"])
            .current_dir("./enclave")
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap())
            .status()
            .unwrap()
            .success(),
        "failed to build enclave module"
    );

    assert!(
        std::process::Command::new("ftxsgx-elf2sgxs")
            .args([
                CARGO_OUTPUT,
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

    std::fs::rename(
        Path::new(CARGO_OUTPUT).with_extension("sgxs"),
        Path::new(&out_dir).join("enclave.sgxs"),
    )
    .expect("failed to move sgxs to output directory");
}
