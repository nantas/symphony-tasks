use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "symphony-tasks")]
pub struct Cli {
    #[arg(long, default_value = "config/orchestrator.toml")]
    pub config: PathBuf,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Daemon,
    ReconcileOnce,
    ValidateConfig,
}
