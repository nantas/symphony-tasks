pub mod process;
pub mod types;

use crate::agent_runner::types::{AgentRunResult, RunnerError};
use crate::models::issue::NormalizedIssue;
use crate::models::workflow::WorkflowDefinition;
use std::path::Path;

#[async_trait::async_trait]
pub trait AgentRunner: Send + Sync {
    async fn run(
        &self,
        workspace_path: &Path,
        issue: &NormalizedIssue,
        workflow: &WorkflowDefinition,
    ) -> Result<AgentRunResult, RunnerError>;
}
