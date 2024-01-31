use std::path::PathBuf;
use std::process::Command;

use crate::cli::Target;

pub fn build_bpf_program(target: Target, release: bool) -> anyhow::Result<()> {
    let dir = PathBuf::from("../xdp");
    let target = format!("--target={}", target);

    let mut args = vec!["build", target.as_str(), "-Z", "build-std=core"];

    if release {
        args.push("--release")
    }

    let status = Command::new("cargo")
        .current_dir(&dir)
        .env_remove("RUSTUP_TOOLCHAIN")
        .args(&args)
        .status()?;

    if !status.success() {
        anyhow::bail!("failed to build eBFP program");
    }

    Ok(())
}
