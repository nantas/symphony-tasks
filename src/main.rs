use clap::Parser;
use symphony_tasks::app::config::AppConfig;
use symphony_tasks::cli::args::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let config = AppConfig::from_path(&cli.config);

    match cli.command {
        Command::Daemon => run_daemon(config),
        Command::ReconcileOnce => run_reconcile_once(config),
        Command::ValidateConfig => run_validate_config(config),
    }
}

fn run_daemon(_config: AppConfig) {}

fn run_reconcile_once(_config: AppConfig) {}

fn run_validate_config(_config: AppConfig) {}
