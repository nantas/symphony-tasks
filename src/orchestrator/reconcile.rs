use crate::agent_runner::AgentRunner;
use crate::models::issue::NormalizedIssue;
use crate::models::pr::{MergeStatus, ReviewStatus};
use crate::models::repository::RepositoryProfile;
use crate::models::run_record::{RunRecord, RunStatus};
use crate::models::workflow::WorkflowDefinition;
use crate::orchestrator::retry::RetryBackoffEntry;
use crate::state_store::{PrWatchEntry, StateStore};
use crate::tracker::Tracker;
use crate::tracker::types::CreatePrRequest;
use crate::workspace::{WorkspaceManager, WorkspaceRequest};
use anyhow::Result;
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

pub struct DispatchRequest<'a> {
    pub repo: &'a RepositoryProfile,
    pub issue: &'a NormalizedIssue,
    pub workflow: &'a WorkflowDefinition,
    pub started_at: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchResult {
    pub claimed_issue_id: String,
    pub run_record: RunRecord,
}

pub struct PrLifecycleRequest<'a> {
    pub repo: &'a RepositoryProfile,
    pub issue: &'a NormalizedIssue,
    pub workflow: &'a WorkflowDefinition,
    pub run_record: RunRecord,
    pub base_branch: &'a str,
    pub updated_at: &'a str,
}

pub struct WatchPrRequest<'a> {
    pub repo: &'a RepositoryProfile,
    pub issue: &'a NormalizedIssue,
    pub workflow: &'a WorkflowDefinition,
    pub run_record: RunRecord,
    pub updated_at: &'a str,
}

pub fn select_dispatch_candidates(
    candidates: &[NormalizedIssue],
    repo: &RepositoryProfile,
    workflow: &WorkflowDefinition,
    context: &SelectionContext,
) -> Vec<NormalizedIssue> {
    let remaining_global = context.global_limit.saturating_sub(context.global_running);
    let remaining_repo = repo
        .max_concurrent_runs
        .saturating_sub(context.repo_running);
    let limit = remaining_global.min(remaining_repo);

    if limit == 0 {
        return Vec::new();
    }

    candidates
        .iter()
        .filter(|issue| {
            workflow
                .active_states
                .iter()
                .any(|state| state == &issue.state)
        })
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

pub async fn dispatch_issue<T: Tracker, R: AgentRunner>(
    tracker: &T,
    runner: &R,
    workspace_manager: &WorkspaceManager,
    state_store: &StateStore,
    request: DispatchRequest<'_>,
) -> Result<DispatchResult> {
    tracker
        .update_issue_state(request.repo, &request.issue.id, "In Progress")
        .await?;

    let workspace = workspace_manager.prepare_workspace(&WorkspaceRequest {
        repo_id: request.repo.repo_id.clone(),
        issue_identifier: request.issue.identifier.clone(),
        source_repo_path: request.repo.repo_path.clone(),
        after_create: request.workflow.hooks.after_create.clone(),
    })?;
    workspace_manager.run_after_create_hooks(&workspace).await?;
    workspace_manager
        .run_before_run_hooks(&workspace, &request.workflow.hooks.before_run)
        .await?;
    let result = runner
        .run(&workspace.path, request.issue, request.workflow)
        .await
        .map_err(anyhow::Error::new)?;
    workspace_manager
        .run_after_run_hooks(&workspace, &request.workflow.hooks.after_run)
        .await?;

    let run_record = RunRecord {
        issue_id: request.issue.id.clone(),
        repo_id: request.repo.repo_id.clone(),
        attempt: 1,
        workspace_path: workspace.path.clone(),
        status: match result.status {
            crate::agent_runner::types::AgentRunStatus::Success => RunStatus::AwaitingPrCreation,
            crate::agent_runner::types::AgentRunStatus::Failed => RunStatus::Failed,
        },
        branch_name: result.branch_name.clone(),
        commit_sha: result.commit_sha.clone(),
        pr_ref: None,
        started_at: request.started_at.to_string(),
        updated_at: request.started_at.to_string(),
        last_error: None,
        next_retry_at: None,
    };
    state_store.save_run_record(&run_record)?;

    Ok(DispatchResult {
        claimed_issue_id: request.issue.id.clone(),
        run_record,
    })
}

pub async fn create_pr_for_run<T: Tracker>(
    tracker: &T,
    state_store: &StateStore,
    request: PrLifecycleRequest<'_>,
) -> Result<RunRecord> {
    let pr = tracker
        .create_or_update_pr(
            request.repo,
            CreatePrRequest {
                issue_id: request.issue.id.clone(),
                title: request.issue.title.clone(),
                body: request.issue.description.clone().unwrap_or_default(),
                head_branch: request
                    .run_record
                    .branch_name
                    .clone()
                    .unwrap_or_else(|| request.issue.identifier.clone()),
                base_branch: request.base_branch.to_string(),
            },
        )
        .await?;

    tracker
        .update_issue_state(request.repo, &request.issue.id, "Human Review")
        .await?;

    let updated = RunRecord {
        pr_ref: Some(pr.id.clone()),
        status: RunStatus::AwaitingHumanReview,
        updated_at: request.updated_at.to_string(),
        ..request.run_record
    };
    state_store.save_run_record(&updated)?;
    state_store.upsert_pr_watch_entry(PrWatchEntry {
        issue_id: updated.issue_id.clone(),
        repo_id: updated.repo_id.clone(),
        pr_ref: pr.id,
        status: "awaiting_human_review".to_string(),
    })?;

    Ok(updated)
}

pub async fn reconcile_pr_watch<T: Tracker>(
    tracker: &T,
    state_store: &StateStore,
    request: WatchPrRequest<'_>,
) -> Result<RunRecord> {
    let pr_ref = request
        .run_record
        .pr_ref
        .clone()
        .ok_or_else(|| anyhow::anyhow!("run record missing pr_ref"))?;
    let status = tracker.get_pr_status(request.repo, &pr_ref).await?;

    let mut updated = RunRecord {
        updated_at: request.updated_at.to_string(),
        ..request.run_record
    };

    if status.pr.review_status == ReviewStatus::Approved
        && status.pr.merge_status == MergeStatus::Mergeable
    {
        tracker.merge_pr(request.repo, &pr_ref).await?;
        updated.status = RunStatus::Completed;
        if request.workflow.completion_policy.close_issue_on_merge {
            tracker
                .update_issue_state(request.repo, &request.issue.id, "Done")
                .await?;
        }
        state_store.remove_pr_watch_entry(&updated.repo_id, &updated.issue_id)?;
    } else {
        updated.status = RunStatus::AwaitingHumanReview;
        state_store.upsert_pr_watch_entry(PrWatchEntry {
            issue_id: updated.issue_id.clone(),
            repo_id: updated.repo_id.clone(),
            pr_ref: pr_ref.clone(),
            status: "awaiting_human_review".to_string(),
        })?;
    }

    state_store.save_run_record(&updated)?;
    Ok(updated)
}
