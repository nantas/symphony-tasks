mod hooks;
pub mod keys;

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
        let valid_workspace = path.join(".git").exists();
        let created_now = !valid_workspace;

        fs::create_dir_all(&repo_root)
            .with_context(|| format!("failed to create workspace root {}", repo_root.display()))?;

        if created_now {
            if path.exists() {
                fs::remove_dir_all(&path).with_context(|| {
                    format!("failed to remove invalid workspace {}", path.display())
                })?;
            }

            let source_repo = request
                .source_repo_path
                .to_str()
                .context("workspace source repo path is not valid UTF-8")?;
            let workspace_path = path.to_str().context("workspace path is not valid UTF-8")?;

            let status = Command::new("git")
                .args(["clone", "--quiet", source_repo, workspace_path])
                .status()
                .with_context(|| {
                    format!(
                        "failed to clone source repo {} into workspace {}",
                        request.source_repo_path.display(),
                        path.display()
                    )
                })?;

            if !status.success() {
                anyhow::bail!(
                    "git clone failed for workspace {} from {}",
                    path.display(),
                    request.source_repo_path.display()
                );
            }

            let remote_output = Command::new("git")
                .args(["remote", "get-url", "origin"])
                .current_dir(&request.source_repo_path)
                .output()
                .with_context(|| {
                    format!(
                        "failed to read origin remote from source repo {}",
                        request.source_repo_path.display()
                    )
                })?;

            if remote_output.status.success() {
                let remote_url = String::from_utf8_lossy(&remote_output.stdout)
                    .trim()
                    .to_string();

                if !remote_url.is_empty() {
                    let status = Command::new("git")
                        .args(["remote", "set-url", "origin", &remote_url])
                        .current_dir(&path)
                        .status()
                        .with_context(|| {
                            format!(
                                "failed to set origin remote for workspace {}",
                                path.display()
                            )
                        })?;

                    if !status.success() {
                        anyhow::bail!("git remote set-url failed for workspace {}", path.display());
                    }
                }
            }
        }

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
        self.run_before_run_hooks(workspace, before_run).await?;
        self.run_after_run_hooks(workspace, after_run).await?;
        Ok(())
    }

    pub async fn run_before_run_hooks(
        &self,
        workspace: &Workspace,
        before_run: &[String],
    ) -> Result<()> {
        hooks::run_hook_commands(&workspace.path, before_run).await?;
        Ok(())
    }

    pub async fn run_after_run_hooks(
        &self,
        workspace: &Workspace,
        after_run: &[String],
    ) -> Result<()> {
        hooks::run_hook_commands(&workspace.path, after_run).await?;
        Ok(())
    }
}
