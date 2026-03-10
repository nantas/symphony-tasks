use crate::models::pr::PullRequestRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePrRequest {
    pub issue_id: String,
    pub title: String,
    pub body: String,
    pub head_branch: String,
    pub base_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRequest {
    pub issue_id: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrStatus {
    pub pr: PullRequestRef,
}
