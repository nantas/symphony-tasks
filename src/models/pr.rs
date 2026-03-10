use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewStatus {
    Pending,
    Approved,
    ChangesRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStatus {
    Unknown,
    Mergeable,
    Conflicting,
    Merged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestRef {
    pub id: String,
    pub number: u64,
    pub url: String,
    pub head_branch: String,
    pub state: String,
    pub review_status: ReviewStatus,
    pub merge_status: MergeStatus,
}
