use std::collections::HashSet;

use symphony_tasks::models::issue::NormalizedIssue;
use symphony_tasks::models::repository::RepositoryProfile;
use symphony_tasks::models::workflow::{
    CompletionPolicy, PrPolicy, RetryPolicy, WorkflowDefinition, WorkflowHooks,
};
use symphony_tasks::orchestrator::reconcile::{select_dispatch_candidates, SelectionContext};
use symphony_tasks::orchestrator::retry::RetryBackoffEntry;

fn repo_profile() -> RepositoryProfile {
    RepositoryProfile {
        repo_id: "demo".into(),
        repo_path: "/tmp/demo".into(),
        workflow_path: "/tmp/demo/WORKFLOW.md".into(),
        tracker_kind: "gitcode".into(),
        tracker_project_ref: "acme/demo".into(),
        default_runner: "process".into(),
        enabled: true,
        max_concurrent_runs: 2,
    }
}

fn workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        active_states: vec!["Todo".into(), "In Progress".into()],
        terminal_states: vec!["Done".into()],
        prompt_template: String::new(),
        state_mapping: Default::default(),
        hooks: WorkflowHooks::default(),
        retry_policy: RetryPolicy {
            max_attempts: 3,
            backoff_seconds: 60,
        },
        pr_policy: PrPolicy { require_pr: true },
        completion_policy: CompletionPolicy {
            close_issue_on_merge: true,
        },
    }
}

fn issue(id: &str, state: &str) -> NormalizedIssue {
    NormalizedIssue {
        id: id.into(),
        identifier: format!("demo#{id}"),
        repo_id: "demo".into(),
        title: format!("Issue {id}"),
        description: None,
        state: state.into(),
        priority: Some(1),
        labels: vec![],
        url: None,
        created_at: None,
        updated_at: None,
    }
}

#[test]
fn respects_repository_and_global_concurrency_limits() {
    let candidates = vec![issue("1", "Todo"), issue("2", "Todo"), issue("3", "Todo")];

    let selected = select_dispatch_candidates(
        &candidates,
        &repo_profile(),
        &workflow(),
        &SelectionContext {
            global_limit: 2,
            global_running: 0,
            repo_running: 1,
            claimed_issue_ids: HashSet::new(),
            retry_backoff: vec![],
            now_epoch_ms: 1_000,
        },
    );

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "1");
}

#[test]
fn excludes_issues_still_in_retry_backoff() {
    let candidates = vec![issue("1", "Todo"), issue("2", "Todo")];
    let retry_backoff = vec![RetryBackoffEntry {
        issue_id: "1".into(),
        due_at_epoch_ms: 2_000,
    }];

    let selected = select_dispatch_candidates(
        &candidates,
        &repo_profile(),
        &workflow(),
        &SelectionContext {
            global_limit: 2,
            global_running: 0,
            repo_running: 0,
            claimed_issue_ids: HashSet::new(),
            retry_backoff,
            now_epoch_ms: 1_000,
        },
    );

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "2");
}

#[test]
fn skips_already_claimed_issues() {
    let candidates = vec![issue("1", "Todo"), issue("2", "Todo")];
    let claimed_issue_ids = HashSet::from([String::from("1")]);

    let selected = select_dispatch_candidates(
        &candidates,
        &repo_profile(),
        &workflow(),
        &SelectionContext {
            global_limit: 2,
            global_running: 0,
            repo_running: 0,
            claimed_issue_ids,
            retry_backoff: vec![],
            now_epoch_ms: 1_000,
        },
    );

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "2");
}

#[test]
fn filters_out_non_active_issue_states() {
    let candidates = vec![issue("1", "Todo"), issue("2", "Done"), issue("3", "Failed")];

    let selected = select_dispatch_candidates(
        &candidates,
        &repo_profile(),
        &workflow(),
        &SelectionContext {
            global_limit: 2,
            global_running: 0,
            repo_running: 0,
            claimed_issue_ids: HashSet::new(),
            retry_backoff: vec![],
            now_epoch_ms: 1_000,
        },
    );

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "1");
}

#[test]
fn retry_entry_not_yet_due_blocks_dispatch() {
    let candidates = vec![issue("1", "Todo"), issue("2", "Todo")];
    let retry_backoff = vec![RetryBackoffEntry {
        issue_id: "1".into(),
        due_at_epoch_ms: 10_000,
    }];

    let selected = select_dispatch_candidates(
        &candidates,
        &repo_profile(),
        &workflow(),
        &SelectionContext {
            global_limit: 2,
            global_running: 0,
            repo_running: 0,
            claimed_issue_ids: HashSet::new(),
            retry_backoff,
            now_epoch_ms: 5_000,
        },
    );

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "2");
}

#[test]
fn retry_entry_that_is_due_becomes_eligible() {
    let candidates = vec![issue("1", "Todo"), issue("2", "Todo")];
    let retry_backoff = vec![RetryBackoffEntry {
        issue_id: "1".into(),
        due_at_epoch_ms: 5_000,
    }];

    let selected = select_dispatch_candidates(
        &candidates,
        &repo_profile(),
        &workflow(),
        &SelectionContext {
            global_limit: 2,
            global_running: 0,
            repo_running: 0,
            claimed_issue_ids: HashSet::new(),
            retry_backoff,
            now_epoch_ms: 10_000,
        },
    );

    assert_eq!(selected.len(), 2);
}
