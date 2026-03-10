use crate::models::issue::NormalizedIssue;
use crate::models::pr::{MergeStatus, PullRequestRef, ReviewStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCodeLabel {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCodeIssue {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub issue_state: Option<String>,
    #[serde(default)]
    pub labels: Vec<GitCodeLabel>,
    pub priority: Option<u8>,
    pub html_url: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl GitCodeIssue {
    pub fn to_normalized_issue(&self, repo_id: &str) -> NormalizedIssue {
        NormalizedIssue {
            id: self.id.to_string(),
            identifier: format!("{repo_id}#{}", self.number),
            repo_id: repo_id.to_string(),
            title: self.title.clone(),
            description: self.body.clone(),
            state: self.issue_state.clone().unwrap_or_else(|| self.state.clone()),
            priority: self.priority,
            labels: self.labels.iter().map(|label| label.name.clone()).collect(),
            url: self.html_url.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCodePullRequest {
    pub id: u64,
    pub number: u64,
    pub html_url: String,
    pub state: String,
    pub head: String,
    pub merge_status: Option<String>,
    pub review_status: Option<String>,
}

impl GitCodePullRequest {
    pub fn to_pull_request_ref(&self) -> PullRequestRef {
        PullRequestRef {
            id: self.id.to_string(),
            number: self.number,
            url: self.html_url.clone(),
            head_branch: self.head.clone(),
            state: self.state.clone(),
            review_status: map_review_status(self.review_status.as_deref()),
            merge_status: map_merge_status(self.merge_status.as_deref()),
        }
    }
}

fn map_review_status(value: Option<&str>) -> ReviewStatus {
    match value {
        Some("approved") => ReviewStatus::Approved,
        Some("changes_requested") => ReviewStatus::ChangesRequested,
        _ => ReviewStatus::Pending,
    }
}

fn map_merge_status(value: Option<&str>) -> MergeStatus {
    match value {
        Some("can_be_merged") => MergeStatus::Mergeable,
        Some("cannot_be_merged") => MergeStatus::Conflicting,
        Some("merged") => MergeStatus::Merged,
        _ => MergeStatus::Unknown,
    }
}
