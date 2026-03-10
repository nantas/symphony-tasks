use crate::agent_runner::types::{AgentRunResult, RunnerError};
use crate::models::issue::NormalizedIssue;
use crate::models::workflow::WorkflowDefinition;
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRunnerConfig {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProcessRunner {
    config: ProcessRunnerConfig,
}

impl ProcessRunner {
    pub fn new(config: ProcessRunnerConfig) -> Self {
        Self { config }
    }

    pub(crate) fn render_prompt(
        &self,
        issue: &NormalizedIssue,
        workflow: &WorkflowDefinition,
    ) -> String {
        workflow
            .prompt_template
            .replace("{{issue_title}}", &issue.title)
            .replace(
                "{{issue_description}}",
                issue.description.as_deref().unwrap_or_default(),
            )
            .replace("{{issue_identifier}}", &issue.identifier)
            .replace("{{issue_state}}", &issue.state)
    }
}

#[async_trait::async_trait]
impl super::AgentRunner for ProcessRunner {
    async fn run(
        &self,
        workspace_path: &Path,
        issue: &NormalizedIssue,
        workflow: &WorkflowDefinition,
    ) -> Result<AgentRunResult, RunnerError> {
        let prompt = self.render_prompt(issue, workflow);
        let output = Command::new(&self.config.program)
            .args(&self.config.args)
            .current_dir(workspace_path)
            .env("PROMPT", prompt)
            .output()
            .await?;

        if !output.status.success() {
            return Err(RunnerError::ProcessFailed {
                exit_code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(serde_json::from_slice(&output.stdout)?)
    }
}
