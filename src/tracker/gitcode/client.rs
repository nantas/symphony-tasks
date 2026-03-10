use crate::models::issue::NormalizedIssue;
use crate::models::pr::PullRequestRef;
use crate::models::repository::RepositoryProfile;
use crate::tracker::gitcode::models::{GitCodeIssue, GitCodePullRequest};
use crate::tracker::types::{CommentRequest, CreatePrRequest, PrStatus};
use crate::tracker::Tracker;
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct GitCodeClient {
    base_url: String,
    token: String,
    http: Client,
}

impl GitCodeClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
            http: Client::new(),
        }
    }

    fn endpoint(&self, repo: &RepositoryProfile, suffix: &str) -> String {
        format!("{}/api/v5/repos/{}{}", self.base_url, repo.gitcode_project_ref, suffix)
    }

    fn request(&self, method: reqwest::Method, url: String) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header("private-token", &self.token)
            .header("accept", "application/json")
    }

    async fn parse_json<T: serde::de::DeserializeOwned>(
        response: reqwest::Response,
        context: &str,
    ) -> Result<T> {
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("{context} failed with status {status}: {body}");
        }

        response
            .json::<T>()
            .await
            .with_context(|| format!("failed to decode {context} response"))
    }

    async fn expect_success(response: reqwest::Response, context: &str) -> Result<()> {
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("{context} failed with status {status}: {body}");
        }

        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct UpdateIssueStateBody<'a> {
    state: &'a str,
}

#[derive(Debug, Serialize)]
struct CommentBody<'a> {
    body: &'a str,
}

#[derive(Debug, Serialize)]
struct CreatePrBody<'a> {
    title: &'a str,
    body: &'a str,
    head: &'a str,
    base: &'a str,
}

#[async_trait]
impl Tracker for GitCodeClient {
    async fn fetch_candidate_issues(&self, repo: &RepositoryProfile) -> Result<Vec<NormalizedIssue>> {
        let response = self
            .request(reqwest::Method::GET, self.endpoint(repo, "/issues"))
            .query(&[("state", "open")])
            .send()
            .await
            .context("failed to fetch candidate issues")?;
        let issues: Vec<GitCodeIssue> = Self::parse_json(response, "fetch candidate issues").await?;
        Ok(issues
            .iter()
            .map(|issue| issue.to_normalized_issue(&repo.repo_id))
            .collect())
    }

    async fn fetch_issue(&self, repo: &RepositoryProfile, issue_id: &str) -> Result<NormalizedIssue> {
        let response = self
            .request(
                reqwest::Method::GET,
                self.endpoint(repo, &format!("/issues/{issue_id}")),
            )
            .send()
            .await
            .context("failed to fetch issue")?;
        let issue: GitCodeIssue = Self::parse_json(response, "fetch issue").await?;
        Ok(issue.to_normalized_issue(&repo.repo_id))
    }

    async fn update_issue_state(
        &self,
        repo: &RepositoryProfile,
        issue_id: &str,
        state: &str,
    ) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::PATCH,
                self.endpoint(repo, &format!("/issues/{issue_id}")),
            )
            .json(&UpdateIssueStateBody { state })
            .send()
            .await
            .context("failed to update issue state")?;
        Self::expect_success(response, "update issue state").await
    }

    async fn add_comment(&self, repo: &RepositoryProfile, request: CommentRequest) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                self.endpoint(repo, &format!("/issues/{}/comments", request.issue_id)),
            )
            .json(&CommentBody {
                body: &request.body,
            })
            .send()
            .await
            .context("failed to create issue comment")?;
        Self::expect_success(response, "create issue comment").await
    }

    async fn create_or_update_pr(
        &self,
        repo: &RepositoryProfile,
        request: CreatePrRequest,
    ) -> Result<PullRequestRef> {
        let response = self
            .request(reqwest::Method::POST, self.endpoint(repo, "/pulls"))
            .json(&CreatePrBody {
                title: &request.title,
                body: &request.body,
                head: &request.head_branch,
                base: &request.base_branch,
            })
            .send()
            .await
            .context("failed to create pull request")?;
        let pr: GitCodePullRequest = Self::parse_json(response, "create pull request").await?;
        Ok(pr.to_pull_request_ref())
    }

    async fn get_pr_status(&self, repo: &RepositoryProfile, pr_ref: &str) -> Result<PrStatus> {
        let response = self
            .request(
                reqwest::Method::GET,
                self.endpoint(repo, &format!("/pulls/{pr_ref}")),
            )
            .send()
            .await
            .context("failed to fetch pull request status")?;
        let pr: GitCodePullRequest = Self::parse_json(response, "fetch pull request status").await?;
        Ok(PrStatus {
            pr: pr.to_pull_request_ref(),
        })
    }

    async fn merge_pr(&self, repo: &RepositoryProfile, pr_ref: &str) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::PUT,
                self.endpoint(repo, &format!("/pulls/{pr_ref}/merge")),
            )
            .send()
            .await
            .context("failed to merge pull request")?;
        Self::expect_success(response, "merge pull request").await
    }
}
