use clap::Parser;

use crate::commands::{build, run};

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(subcommand)]
    pub(crate) command: Command,
}

impl Cli {
    pub fn exec(self) -> anyhow::Result<()> {
        match self.command {
            Command::Build { target, release } => build::build_bpf_program(target, release),
            Command::Run { target, release } => run::run(target, release),
        }
    }
}

#[derive(Debug, Parser)]
pub enum Command {
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
    },
}

#[derive(Debug, Copy, Clone)]
pub enum Target {
    BpfEl,
    BpfEb,
}

impl std::str::FromStr for Target {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
