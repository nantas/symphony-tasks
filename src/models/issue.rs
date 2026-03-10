use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueLifecycleState {
    Todo,
    InProgress,
    HumanReview,
    Done,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedIssue {
    pub id: String,
    pub identifier: String,
    pub repo_id: String,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub priority: Option<u8>,
    pub labels: Vec<String>,
    pub url: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
