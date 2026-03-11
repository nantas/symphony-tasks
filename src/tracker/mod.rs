pub mod github;
pub mod gitcode;
pub mod types;

use crate::models::issue::NormalizedIssue;
use crate::models::pr::PullRequestRef;
use crate::models::repository::RepositoryProfile;
use anyhow::Result;
use async_trait::async_trait;
use types::{CommentRequest, CreatePrRequest, PrStatus};

#[async_trait]
pub trait Tracker: Send + Sync {
    async fn fetch_candidate_issues(
        &self,
        repo: &RepositoryProfile,
    ) -> Result<Vec<NormalizedIssue>>;
    async fn fetch_issue(
        &self,
        repo: &RepositoryProfile,
        issue_id: &str,
    ) -> Result<NormalizedIssue>;
    async fn update_issue_state(
        &self,
        repo: &RepositoryProfile,
        issue_id: &str,
        state: &str,
    ) -> Result<()>;
    async fn add_comment(&self, repo: &RepositoryProfile, request: CommentRequest) -> Result<()>;
    async fn create_or_update_pr(
        &self,
        repo: &RepositoryProfile,
        request: CreatePrRequest,
    ) -> Result<PullRequestRef>;
    async fn get_pr_status(&self, repo: &RepositoryProfile, pr_ref: &str) -> Result<PrStatus>;
    async fn merge_pr(&self, repo: &RepositoryProfile, pr_ref: &str) -> Result<()>;
    async fn close_issue(&self, repo: &RepositoryProfile, issue_id: &str) -> Result<()>;
}
