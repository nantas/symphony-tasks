use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryProfile {
    pub repo_id: String,
    pub repo_path: PathBuf,
    pub workflow_path: PathBuf,
    pub gitcode_project_ref: String,
    pub default_runner: String,
    pub enabled: bool,
    pub max_concurrent_runs: usize,
}
