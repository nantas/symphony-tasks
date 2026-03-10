use anyhow::{Context, Result, bail};
use std::path::Path;
use tokio::process::Command;

pub async fn run_hook_commands(workspace_path: &Path, hooks: &[String]) -> Result<()> {
    for hook in hooks {
        let output = Command::new("/bin/sh")
            .arg("-lc")
            .arg(hook)
            .current_dir(workspace_path)
            .output()
            .await
            .with_context(|| format!("failed to execute hook: {hook}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "hook failed: {hook}; stderr={}",
                stderr.trim()
            );
        }
    }

    Ok(())
}
