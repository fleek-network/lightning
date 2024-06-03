use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};
use clap::Subcommand;

use crate::commands::admin::{BIND_PATH, PATH_CONFIG};

pub fn exec(cmd: EbpfCmd) -> Result<()> {
    match cmd {
        EbpfCmd::Build { target, release } => build_bpf_program(target, release),
        EbpfCmd::Run {
            target,
            release,
            iface,
        } => run(target, release, iface),
    }
}

#[derive(Debug, Subcommand)]
pub enum EbpfCmd {
    /// Build eBPF program.
    Build {
        /// Set the target triple of the BPF program.
        #[clap(default_value = "bpfel-unknown-none", long)]
        target: Target,
        /// Build in release mode.
        #[clap(long)]
        release: bool,
    },
    /// Compile and run eBPF program and userspace application.
    Run {
        /// Set the target triple of the BPF program.
        #[clap(default_value = "bpfel-unknown-none", long)]
        target: Target,
        /// Run in release mode.
        #[clap(long)]
        release: bool,
        /// Interface to attach packet filter program.
        #[clap(short, long, default_value = "eth0")]
        iface: String,
    },
}

#[derive(Debug, Copy, Clone)]
pub enum Target {
    BpfEl,
    BpfEb,
}

impl std::str::FromStr for Target {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "bpfel-unknown-none" => Target::BpfEl,
            "bpfeb-unknown-none" => Target::BpfEb,
            _ => return Err("unsupported target".to_owned()),
        })
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Target::BpfEl => "bpfel-unknown-none",
            Target::BpfEb => "bpfeb-unknown-none",
        })
    }
}

pub fn build_bpf_program(target: Target, release: bool) -> Result<()> {
    let dir = PathBuf::from("etc/ebpf/ebpf");
    let target = format!("--target={}", target);

    let mut args = vec!["build", target.as_str(), "-Z", "build-std=core"];

    if release {
        args.push("--release")
    }

    let status = Command::new("cargo")
        .env("RUSTFLAGS", "-C debuginfo=2 -C link-arg=--btf")
        .current_dir(&dir)
        .env_remove("RUSTUP_TOOLCHAIN")
        .args(&args)
        .status()?;

    if !status.success() {
        bail!("failed to build eBFP program");
    }

    Ok(())
}

fn build_userspace_application(release: bool) -> Result<()> {
    let mut args = vec![
        "build",
        "--bin=control_application",
        "--features=control",
        "--features=server",
    ];

    if release {
        args.push("--release")
    }

    let status = Command::new("cargo")
        .args(&args)
        .current_dir("etc/ebpf/service")
        .status()?;

    if !status.success() {
        bail!("failed to build control application");
    }

    Ok(())
}

pub fn run(target: Target, release: bool, iface: String) -> Result<()> {
    build_bpf_program(target, release)?;
    build_userspace_application(release)?;

    let mode = if release { "release" } else { "debug" };
    let bin_path = format!("etc/ebpf/service/target/{mode}/control_application");

    let path_config = PATH_CONFIG.get().expect("Static to be initialized");
    let xdp_args = format!("--iface={iface}");
    let pf = format!("--pf={}", path_config.packet_filter.display());
    let tmp = format!("--tmp={}", path_config.tmp_dir.display());
    let profile = format!("--profile={}", path_config.profiles_dir.display());
    let bind = format!(
        "--bind={}",
        BIND_PATH.get().expect("Static to be initialized").display()
    );
    let mut args: Vec<_> = vec!["sudo", "-E"];
    args.push(bin_path.as_str());
    args.push(&xdp_args);
    args.push(&pf);
    args.push(&tmp);
    args.push(&profile);
    args.push(&bind);

    let status = Command::new(args.first().expect("args are hardcoded"))
        .args(args.iter().skip(1))
        .status()?;

    if !status.success() {
        bail!(format!("failed to run `{}`", args.join(" ")));
    }

    Ok(())
}
