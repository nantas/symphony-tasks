use crate::app::config::{OrchestratorConfig, config_root, resolve_path};
use crate::models::repository::RepositoryProfile;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct RawRepositoryProfile {
    repo_id: String,
    repo_path: PathBuf,
    workflow_path: PathBuf,
    gitcode_project_ref: String,
    default_runner: String,
    enabled: bool,
    max_concurrent_runs: usize,
}

pub fn load_repository_profiles(config: &OrchestratorConfig) -> Result<Vec<RepositoryProfile>> {
    let mut files = fs::read_dir(&config.repositories_dir)
        .with_context(|| {
            format!(
                "failed to read repositories dir {}",
                config.repositories_dir.display()
            )
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to list repository configs")?;
    files.sort_by_key(|entry| entry.path());

    let mut seen_repo_ids = HashSet::new();
    let mut profiles = Vec::new();

    for entry in files {
        let path = entry.path();
        if !is_toml_file(&path) {
            continue;
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read repository config {}", path.display()))?;
        let raw: RawRepositoryProfile = toml::from_str(&contents)
            .with_context(|| format!("failed to parse repository config {}", path.display()))?;

        if !seen_repo_ids.insert(raw.repo_id.clone()) {
            bail!("duplicate repo_id: {}", raw.repo_id);
        }
        if !raw.enabled {
            continue;
        }
        if raw.max_concurrent_runs == 0 {
            bail!(
                "max_concurrent_runs must be greater than zero for {}",
                raw.repo_id
            );
        }

        let base_dir = config_root(&path);
        let repo_path = resolve_path(&base_dir, &raw.repo_path);
        if !repo_path.exists() {
            bail!(
                "repo_path does not exist for {}: {}",
                raw.repo_id,
                repo_path.display()
            );
        }

        profiles.push(RepositoryProfile {
            repo_id: raw.repo_id,
            repo_path,
            workflow_path: resolve_path(&base_dir, &raw.workflow_path),
            gitcode_project_ref: raw.gitcode_project_ref,
            default_runner: raw.default_runner,
            enabled: raw.enabled,
            max_concurrent_runs: raw.max_concurrent_runs,
        });
    }

    Ok(profiles)
}

fn is_toml_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("toml")
}
