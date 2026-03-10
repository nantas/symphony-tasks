use clap::Parser;
use symphony_tasks::app::config::{AppConfig, validate_config_file};
use symphony_tasks::cli::args::{Cli, Command};
use symphony_tasks::logging::init_logging;

fn main() {
    let cli = Cli::parse();
    let config = AppConfig::from_path(&cli.config);
    init_logging("info", matches!(cli.command, Command::Daemon));

    match cli.command {
        Command::Daemon => run_daemon(config),
        Command::ReconcileOnce => run_reconcile_once(config),
        Command::ValidateConfig => run_validate_config(config),
    }
}

fn run_daemon(_config: AppConfig) {}

fn run_reconcile_once(_config: AppConfig) {}

fn run_validate_config(config: AppConfig) {
    if let Err(error) = validate_config_file(&config.config_path) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
