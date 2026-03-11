use crate::models::issue::NormalizedIssue;
use crate::models::pr::ReviewStatus;
use crate::models::repository::RepositoryProfile;
use crate::tracker::Tracker;
use crate::tracker::github::models::{
    GitHubIssue, GitHubLabel, GitHubPullRequest, GitHubPullRequestReview,
};
use crate::tracker::types::{CommentRequest, CreatePrRequest, PrStatus};
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

const API_VERSION: &str = "2022-11-28";

#[derive(Debug, Clone)]
pub struct GitHubClient {
    base_url: String,
    token: String,
    http: Client,
}

impl GitHubClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
            http: Client::new(),
        }
    }

    fn repo_parts<'a>(&self, repo: &'a RepositoryProfile) -> Result<(&'a str, &'a str)> {
        repo.tracker_project_ref
            .split_once('/')
            .ok_or_else(|| anyhow!("invalid GitHub repo ref {}", repo.tracker_project_ref))
    }

    fn endpoint(&self, repo: &RepositoryProfile, suffix: &str) -> Result<String> {
        let (owner, name) = self.repo_parts(repo)?;
        Ok(format!("{}/repos/{owner}/{name}{suffix}", self.base_url))
    }

    fn request(&self, method: reqwest::Method, url: String) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header("authorization", format!("Bearer {}", self.token))
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", API_VERSION)
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

    async fn fetch_issue_labels(
        &self,
        repo: &RepositoryProfile,
        issue_id: &str,
    ) -> Result<Vec<GitHubLabel>> {
        let response = self
            .request(
                reqwest::Method::GET,
                self.endpoint(repo, &format!("/issues/{issue_id}/labels"))?,
            )
            .send()
            .await
            .context("failed to fetch issue labels")?;
        Self::parse_json(response, "fetch issue labels").await
    }

    async fn fetch_review_status(
        &self,
        repo: &RepositoryProfile,
        pr_ref: &str,
    ) -> Result<ReviewStatus> {
        let response = self
            .request(
                reqwest::Method::GET,
                self.endpoint(repo, &format!("/pulls/{pr_ref}/reviews"))?,
            )
            .send()
            .await
            .context("failed to fetch pull request reviews")?;
        let reviews: Vec<GitHubPullRequestReview> =
            Self::parse_json(response, "fetch pull request reviews").await?;

        if reviews.iter().any(|review| review.state == "CHANGES_REQUESTED") {
            return Ok(ReviewStatus::ChangesRequested);
        }
        if reviews.iter().any(|review| review.state == "APPROVED") {
            return Ok(ReviewStatus::Approved);
        }

        Ok(ReviewStatus::Pending)
    }
}

#[derive(Debug, Serialize)]
struct ReplaceLabelsBody<'a> {
    labels: &'a [String],
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

#[derive(Debug, Serialize)]
struct CloseIssueBody<'a> {
    state: &'a str,
}

#[async_trait]
impl Tracker for GitHubClient {
    async fn fetch_candidate_issues(
        &self,
        repo: &RepositoryProfile,
    ) -> Result<Vec<NormalizedIssue>> {
        let response = self
            .request(reqwest::Method::GET, self.endpoint(repo, "/issues")?)
            .query(&[("state", "open")])
            .send()
            .await
            .context("failed to fetch candidate issues")?;
        let issues: Vec<GitHubIssue> =
            Self::parse_json(response, "fetch candidate issues").await?;

        Ok(issues
            .into_iter()
            .filter(|issue| issue.pull_request.is_none())
            .map(|issue| issue.to_normalized_issue(&repo.repo_id))
            .collect())
    }

    async fn fetch_issue(
        &self,
        repo: &RepositoryProfile,
        issue_id: &str,
    ) -> Result<NormalizedIssue> {
        let response = self
            .request(
                reqwest::Method::GET,
                self.endpoint(repo, &format!("/issues/{issue_id}"))?,
            )
            .send()
            .await
            .context("failed to fetch issue")?;
        let issue: GitHubIssue = Self::parse_json(response, "fetch issue").await?;
        Ok(issue.to_normalized_issue(&repo.repo_id))
    }

    async fn update_issue_state(
        &self,
        repo: &RepositoryProfile,
        issue_id: &str,
        state: &str,
    ) -> Result<()> {
        let mut labels = self.fetch_issue_labels(repo, issue_id).await?;
        labels.retain(|label| !label.is_workflow_state());
        labels.push(GitHubLabel {
            name: GitHubLabel::from_workflow_state(state)?.to_string(),
        });

        let label_names = labels.into_iter().map(|label| label.name).collect::<Vec<_>>();
        let response = self
            .request(
                reqwest::Method::PUT,
                self.endpoint(repo, &format!("/issues/{issue_id}/labels"))?,
            )
            .json(&ReplaceLabelsBody {
                labels: &label_names,
            })
            .send()
            .await
            .context("failed to replace issue labels")?;
        Self::expect_success(response, "replace issue labels").await
    }

    async fn add_comment(&self, repo: &RepositoryProfile, request: CommentRequest) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                self.endpoint(repo, &format!("/issues/{}/comments", request.issue_id))?,
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
    ) -> Result<crate::models::pr::PullRequestRef> {
        let response = self
            .request(reqwest::Method::POST, self.endpoint(repo, "/pulls")?)
            .json(&CreatePrBody {
                title: &request.title,
                body: &request.body,
                head: &request.head_branch,
                base: &request.base_branch,
            })
            .send()
            .await
            .context("failed to create pull request")?;
        let pr: GitHubPullRequest = Self::parse_json(response, "create pull request").await?;
        Ok(pr.to_pull_request_ref())
    }

    async fn get_pr_status(&self, repo: &RepositoryProfile, pr_ref: &str) -> Result<PrStatus> {
        let response = self
            .request(
                reqwest::Method::GET,
                self.endpoint(repo, &format!("/pulls/{pr_ref}"))?,
            )
            .send()
            .await
            .context("failed to fetch pull request status")?;
        let mut pr: GitHubPullRequest =
            Self::parse_json(response, "fetch pull request status").await?;
        pr.review_decision = Some(
            match self.fetch_review_status(repo, pr_ref).await? {
                ReviewStatus::Approved => "APPROVED",
                ReviewStatus::ChangesRequested => "CHANGES_REQUESTED",
                ReviewStatus::Pending => "PENDING",
            }
            .to_string(),
        );

        Ok(PrStatus {
            pr: pr.to_pull_request_ref(),
        })
    }

    async fn merge_pr(&self, repo: &RepositoryProfile, pr_ref: &str) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::PUT,
                self.endpoint(repo, &format!("/pulls/{pr_ref}/merge"))?,
            )
            .send()
            .await
            .context("failed to merge pull request")?;
        Self::expect_success(response, "merge pull request").await
    }

    async fn close_issue(&self, repo: &RepositoryProfile, issue_id: &str) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::PATCH,
                self.endpoint(repo, &format!("/issues/{issue_id}"))?,
            )
            .json(&CloseIssueBody { state: "closed" })
            .send()
            .await
            .context("failed to close issue")?;
        Self::expect_success(response, "close issue").await
    }
}
