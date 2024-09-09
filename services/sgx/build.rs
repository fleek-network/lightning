use std::path::PathBuf;

/// Heap size for enclave
const HEAP_SIZE: &str = "0x100000000"; // 4 GiB
/// Stack size for enclave
const STACK_SIZE: &str = "0x1000000"; // 10 MiB
/// Number of threads to support in enclave
const THREADS: &str = "16";

/// URL of latest enclave
/// TODO: Also download mrsigner signature and get checksum
const ENCLAVE_URL: &str =
    "https://bafybeid37ogyu3ogfctq4ecqa3t3ozneegbkj3gswg3h6lxwx5gq5f4rdm.ipfs.flk-ipfs.xyz";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=FN_ENCLAVE_SOURCE");
    println!("cargo::rerun-if-env-changed=FN_ENCLAVE_SGXS");

    // Build from source
    if let Ok(path) = std::env::var("FN_ENCLAVE_SOURCE").map(PathBuf::from) {
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
        let bin = path.join("target/x86_64-fortanix-unknown-sgx/release/fleek-service-sgx-enclave");

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

    // Use precompiled enclave
    if let Ok(path) = std::env::var("FN_ENCLAVE_SGXS").map(PathBuf::from) {
        if !path.is_file() {
            panic!("enclave must be a file");
        }

        println!("cargo::rerun-if-changed={}", path.to_string_lossy());
        std::fs::copy(path, "./enclave.sgxs").expect("failed to copy provided enclave.sgx");

        return;
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
