mod hooks;
pub mod keys;

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct WorkspaceManager {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct WorkspaceRequest {
    pub repo_id: String,
    pub issue_identifier: String,
    pub source_repo_path: PathBuf,
    pub after_create: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub repo_id: String,
    pub issue_identifier: String,
    pub key: String,
    pub path: PathBuf,
    pub source_repo_path: PathBuf,
    pub created_now: bool,
    pub after_create: Vec<String>,
}

impl WorkspaceManager {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn workspace_key(&self, issue_identifier: &str) -> String {
        keys::sanitize_issue_key(issue_identifier)
    }

    pub fn prepare_workspace(&self, request: &WorkspaceRequest) -> Result<Workspace> {
        let key = self.workspace_key(&request.issue_identifier);
        let repo_root = self.root.join(&request.repo_id);
        let path = repo_root.join(&key);
        let created_now = !path.exists();

        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create workspace {}", path.display()))?;

        Ok(Workspace {
            repo_id: request.repo_id.clone(),
            issue_identifier: request.issue_identifier.clone(),
            key,
            path,
            source_repo_path: request.source_repo_path.clone(),
            created_now,
            after_create: request.after_create.clone(),
        })
    }

    pub async fn run_after_create_hooks(&self, workspace: &Workspace) -> Result<()> {
        if workspace.created_now {
            hooks::run_hook_commands(&workspace.path, &workspace.after_create).await?;
        }

        Ok(())
    }

    pub async fn run_hooks(
        &self,
        workspace: &Workspace,
        before_run: &[String],
        after_run: &[String],
    ) -> Result<()> {
        hooks::run_hook_commands(&workspace.path, before_run).await?;
        hooks::run_hook_commands(&workspace.path, after_run).await?;
        Ok(())
    }
}
