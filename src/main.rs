use clap::Parser;
use symphony_tasks::app::config::{AppConfig, OrchestratorConfig, validate_config_file};
use symphony_tasks::app::{reconcile_once, run_daemon};
use symphony_tasks::cli::args::{Cli, Command};
use symphony_tasks::logging::init_logging;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let config = AppConfig::from_path(&cli.config);
    init_logging("info", matches!(cli.command, Command::Daemon));

    let result = match cli.command {
        Command::Daemon => run_daemon_command(config).await,
        Command::ReconcileOnce => run_reconcile_once(config).await,
        Command::ValidateConfig => run_validate_config(config),
    };

    if let Err(error) = result {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

async fn run_daemon_command(config: AppConfig) -> anyhow::Result<()> {
    let loaded = OrchestratorConfig::load_from_file(&config.config_path)?;
    run_daemon(&loaded, &loaded.lock_path).await
}

async fn run_reconcile_once(config: AppConfig) -> anyhow::Result<()> {
    let loaded = OrchestratorConfig::load_from_file(&config.config_path)?;
    reconcile_once(&loaded).await?;
    Ok(())
}

fn run_validate_config(config: AppConfig) -> anyhow::Result<()> {
    validate_config_file(&config.config_path)?;
    Ok(())
}
