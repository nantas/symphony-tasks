use crate::models::issue::NormalizedIssue;
use crate::models::pr::{MergeStatus, PullRequestRef, ReviewStatus};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

const TODO_LABEL: &str = "todo";
const IN_PROGRESS_LABEL: &str = "in-progress";
const HUMAN_REVIEW_LABEL: &str = "human-review";
const DONE_LABEL: &str = "done";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubLabel {
    pub name: String,
}

impl GitHubLabel {
    pub fn is_workflow_state(&self) -> bool {
        map_workflow_label(&self.name).is_some()
    }

    pub fn from_workflow_state(state: &str) -> Result<&'static str> {
        match state {
            "Todo" => Ok(TODO_LABEL),
            "In Progress" => Ok(IN_PROGRESS_LABEL),
            "Human Review" => Ok(HUMAN_REVIEW_LABEL),
            "Done" => Ok(DONE_LABEL),
            _ => bail!("unsupported GitHub workflow state {state}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubIssuePullRequestRef {
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    #[serde(default)]
    pub labels: Vec<GitHubLabel>,
    pub html_url: Option<String>,
    pub pull_request: Option<GitHubIssuePullRequestRef>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl GitHubIssue {
    pub fn to_normalized_issue(&self, repo_id: &str) -> NormalizedIssue {
        NormalizedIssue {
            // GitHub issue lifecycle endpoints use issue_number, not the internal database id.
            id: self.number.to_string(),
            identifier: format!("{repo_id}#{}", self.number),
            repo_id: repo_id.to_string(),
            title: self.title.clone(),
            description: self.body.clone(),
            state: map_issue_state(&self.state, &self.labels),
            priority: None,
            labels: self.labels.iter().map(|label| label.name.clone()).collect(),
            url: self.html_url.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubPullRequestHead {
    pub r#ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubPullRequest {
    pub id: u64,
    pub number: u64,
    pub html_url: String,
    pub state: String,
    pub head: GitHubPullRequestHead,
    pub mergeable: Option<bool>,
    pub merged: bool,
    pub review_decision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubPullRequestReview {
    pub state: String,
}

impl GitHubPullRequest {
    pub fn to_pull_request_ref(&self) -> PullRequestRef {
        PullRequestRef {
            // GitHub pull request read/merge endpoints use pull_number, not the internal id.
            id: self.number.to_string(),
            number: self.number,
            url: self.html_url.clone(),
            head_branch: self.head.r#ref.clone(),
            state: self.state.clone(),
            review_status: map_review_status(self.review_decision.as_deref()),
            merge_status: map_merge_status(self.mergeable, self.merged),
        }
    }
}

fn map_issue_state(default_state: &str, labels: &[GitHubLabel]) -> String {
    let workflow_labels = labels
        .iter()
        .filter_map(|label| map_workflow_label(&label.name))
        .collect::<Vec<_>>();

    match workflow_labels.as_slice() {
        [] => default_state.to_string(),
        [state] => (*state).to_string(),
        _ => "ambiguous".to_string(),
    }
}

fn map_workflow_label(label: &str) -> Option<&'static str> {
    match label {
        TODO_LABEL => Some("Todo"),
        IN_PROGRESS_LABEL => Some("In Progress"),
        HUMAN_REVIEW_LABEL => Some("Human Review"),
        DONE_LABEL => Some("Done"),
        _ => None,
    }
}

fn map_review_status(value: Option<&str>) -> ReviewStatus {
    match value {
        Some("APPROVED") => ReviewStatus::Approved,
        Some("CHANGES_REQUESTED") => ReviewStatus::ChangesRequested,
        _ => ReviewStatus::Pending,
    }
}

fn map_merge_status(mergeable: Option<bool>, merged: bool) -> MergeStatus {
    if merged {
        return MergeStatus::Merged;
    }

    match mergeable {
        Some(true) => MergeStatus::Mergeable,
        Some(false) => MergeStatus::Conflicting,
        None => MergeStatus::Unknown,
    }
}
