use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use symphony_tasks::agent_runner::AgentRunner;
use symphony_tasks::agent_runner::types::{AgentRunResult, AgentRunStatus, RunnerError};
use symphony_tasks::models::issue::NormalizedIssue;
use symphony_tasks::models::pr::PullRequestRef;
use symphony_tasks::models::repository::RepositoryProfile;
use symphony_tasks::models::run_record::RunStatus;
use symphony_tasks::models::workflow::{
    CompletionPolicy, PrPolicy, RetryPolicy, WorkflowDefinition, WorkflowHooks,
};
use symphony_tasks::orchestrator::reconcile::{DispatchRequest, dispatch_issue};
use symphony_tasks::state_store::StateStore;
use symphony_tasks::tracker::Tracker;
use symphony_tasks::tracker::types::{CommentRequest, CreatePrRequest, PrStatus};
use symphony_tasks::workspace::WorkspaceManager;

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-dispatch-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn repo_profile(root: &Path) -> RepositoryProfile {
    RepositoryProfile {
        repo_id: "demo".into(),
        repo_path: root.join("repo"),
        workflow_path: root.join("repo/WORKFLOW.md"),
        gitcode_project_ref: "acme/demo".into(),
        default_runner: "process".into(),
        enabled: true,
        max_concurrent_runs: 1,
    }
}

fn workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        active_states: vec!["Todo".into()],
        terminal_states: vec!["Done".into()],
        prompt_template: "Implement {{issue_title}}".into(),
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

fn issue() -> NormalizedIssue {
    NormalizedIssue {
        id: "100".into(),
        identifier: "demo#42".into(),
        repo_id: "demo".into(),
        title: "Implement orchestrator".into(),
        description: Some("Build the dispatch loop".into()),
        state: "Todo".into(),
        priority: Some(1),
        labels: vec![],
        url: None,
        created_at: None,
        updated_at: None,
    }
}

#[derive(Default, Clone)]
struct FakeTracker {
    updated_states: Arc<Mutex<Vec<(String, String)>>>,
}

#[async_trait]
impl Tracker for FakeTracker {
    async fn fetch_candidate_issues(
        &self,
        _repo: &RepositoryProfile,
    ) -> anyhow::Result<Vec<NormalizedIssue>> {
        unreachable!()
    }

    async fn fetch_issue(
        &self,
        _repo: &RepositoryProfile,
        _issue_id: &str,
    ) -> anyhow::Result<NormalizedIssue> {
        unreachable!()
    }

    async fn update_issue_state(
        &self,
        _repo: &RepositoryProfile,
        issue_id: &str,
        state: &str,
    ) -> anyhow::Result<()> {
        self.updated_states
            .lock()
            .unwrap()
            .push((issue_id.to_string(), state.to_string()));
        Ok(())
    }

    async fn add_comment(
        &self,
        _repo: &RepositoryProfile,
        _request: CommentRequest,
    ) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn create_or_update_pr(
        &self,
        _repo: &RepositoryProfile,
        _request: CreatePrRequest,
    ) -> anyhow::Result<PullRequestRef> {
        unreachable!()
    }

    async fn get_pr_status(
        &self,
        _repo: &RepositoryProfile,
        _pr_ref: &str,
    ) -> anyhow::Result<PrStatus> {
        unreachable!()
    }

    async fn merge_pr(&self, _repo: &RepositoryProfile, _pr_ref: &str) -> anyhow::Result<()> {
        unreachable!()
    }
}

#[derive(Default, Clone)]
struct FakeRunner {
    workspaces: Arc<Mutex<Vec<PathBuf>>>,
}

#[async_trait]
impl AgentRunner for FakeRunner {
    async fn run(
        &self,
        workspace_path: &Path,
        _issue: &NormalizedIssue,
        _workflow: &WorkflowDefinition,
    ) -> Result<AgentRunResult, RunnerError> {
        self.workspaces
            .lock()
            .unwrap()
            .push(workspace_path.to_path_buf());
        Ok(AgentRunResult {
            status: AgentRunStatus::Success,
            summary: "implemented".into(),
            branch_name: Some("feat/demo-42".into()),
            commit_sha: Some("abc123".into()),
            requested_next_action: None,
        })
    }
}

#[tokio::test]
async fn dispatch_claims_issue_updates_state_and_persists_run_record() {
    let root = unique_temp_dir("dispatch");
    std::fs::create_dir_all(root.join("repo")).unwrap();
    let repo = repo_profile(&root);
    let tracker = FakeTracker::default();
    let runner = FakeRunner::default();
    let workspace_manager = WorkspaceManager::new(root.join("var/workspaces"));
    let state_store = StateStore::new(root.join("var"));

    let result = dispatch_issue(
        &tracker,
        &runner,
        &workspace_manager,
        &state_store,
        DispatchRequest {
            repo: &repo,
            issue: &issue(),
            workflow: &workflow(),
            started_at: "2026-03-10T12:00:00Z",
        },
    )
    .await
    .unwrap();

    assert_eq!(result.claimed_issue_id, "100");
    assert_eq!(
        tracker.updated_states.lock().unwrap().as_slice(),
        &[("100".to_string(), "In Progress".to_string())]
    );

    let workspace_paths = runner.workspaces.lock().unwrap().clone();
    assert_eq!(workspace_paths.len(), 1);
    assert!(workspace_paths[0].exists());

    let stored = state_store.load_run_record("demo", "100").unwrap();
    assert_eq!(stored.status, RunStatus::AwaitingPrCreation);
    assert_eq!(stored.branch_name.as_deref(), Some("feat/demo-42"));
    assert_eq!(stored.commit_sha.as_deref(), Some("abc123"));
}
