use crate::models::issue::NormalizedIssue;
use crate::models::repository::RepositoryProfile;
use crate::models::workflow::WorkflowDefinition;
use crate::orchestrator::retry::RetryBackoffEntry;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct SelectionContext {
    pub global_limit: usize,
    pub global_running: usize,
    pub repo_running: usize,
    pub claimed_issue_ids: HashSet<String>,
    pub retry_backoff: Vec<RetryBackoffEntry>,
    pub now_epoch_ms: u64,
}

pub fn select_dispatch_candidates(
    candidates: &[NormalizedIssue],
    repo: &RepositoryProfile,
    workflow: &WorkflowDefinition,
    context: &SelectionContext,
) -> Vec<NormalizedIssue> {
    let remaining_global = context.global_limit.saturating_sub(context.global_running);
    let remaining_repo = repo.max_concurrent_runs.saturating_sub(context.repo_running);
    let limit = remaining_global.min(remaining_repo);

    if limit == 0 {
        return Vec::new();
    }

    candidates
        .iter()
        .filter(|issue| workflow.active_states.iter().any(|state| state == &issue.state))
        .filter(|issue| !context.claimed_issue_ids.contains(&issue.id))
        .filter(|issue| !is_in_backoff(issue, &context.retry_backoff, context.now_epoch_ms))
        .take(limit)
        .cloned()
        .collect()
}

fn is_in_backoff(
    issue: &NormalizedIssue,
    retry_entries: &[RetryBackoffEntry],
    now_epoch_ms: u64,
) -> bool {
    retry_entries
        .iter()
        .any(|entry| entry.issue_id == issue.id && entry.due_at_epoch_ms > now_epoch_ms)
}
