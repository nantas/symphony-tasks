use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub config_path: PathBuf,
}

impl AppConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_path: path.as_ref().to_path_buf(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestratorConfig {
    pub poll_interval_secs: u64,
    pub global_concurrency: usize,
    pub log_level: String,
    pub state_root: PathBuf,
    pub workspace_root: PathBuf,
    pub lock_path: PathBuf,
    pub gitcode_token_env: String,
    pub default_runner: String,
    pub repositories_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RawOrchestratorConfig {
    poll_interval_secs: u64,
    global_concurrency: usize,
    log_level: String,
    state_root: PathBuf,
    workspace_root: PathBuf,
    lock_path: PathBuf,
    gitcode_token_env: String,
    default_runner: String,
    repositories_dir: PathBuf,
}

impl OrchestratorConfig {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = absolutize(path.as_ref());
        let base_dir = config_root(&path);
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file {}", path.display()))?;
        let raw: RawOrchestratorConfig =
            toml::from_str(&contents).context("failed to parse orchestrator config")?;

        if raw.global_concurrency == 0 {
            bail!("global_concurrency must be greater than zero");
        }

        Ok(Self {
            poll_interval_secs: raw.poll_interval_secs,
            global_concurrency: raw.global_concurrency,
            log_level: raw.log_level,
            state_root: resolve_path(&base_dir, &raw.state_root),
            workspace_root: resolve_path(&base_dir, &raw.workspace_root),
            lock_path: resolve_path(&base_dir, &raw.lock_path),
            gitcode_token_env: raw.gitcode_token_env,
            default_runner: raw.default_runner,
            repositories_dir: resolve_path(&base_dir, &raw.repositories_dir),
        })
    }
}

pub fn resolve_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

pub fn config_root(config_path: &Path) -> PathBuf {
    let parent = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    match parent.file_name().and_then(|name| name.to_str()) {
        Some("config") => parent
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(parent),
        Some("repositories") => parent
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or(parent),
        _ => parent,
    }
}
