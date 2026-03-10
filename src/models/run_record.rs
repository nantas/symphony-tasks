use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    Queued,
    Claiming,
    PreparingWorkspace,
    RunningAgent,
    AwaitingPrCreation,
    AwaitingHumanReview,
    ApprovedForMerge,
    Merging,
    Completed,
    RetryBackoff,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub issue_id: String,
    pub repo_id: String,
    pub attempt: u32,
    pub workspace_path: PathBuf,
    pub status: RunStatus,
    pub branch_name: Option<String>,
    pub commit_sha: Option<String>,
    pub pr_ref: Option<String>,
    pub started_at: String,
    pub updated_at: String,
    pub last_error: Option<String>,
    pub next_retry_at: Option<String>,
}
