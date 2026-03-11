use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use symphony_tasks::agent_runner::AgentRunner;
use symphony_tasks::agent_runner::types::{AgentRunResult, AgentRunStatus, RunnerError};
use symphony_tasks::app::config::OrchestratorConfig;
use symphony_tasks::app::reconcile_once_with;
use symphony_tasks::models::issue::NormalizedIssue;
use symphony_tasks::models::pr::{MergeStatus, PullRequestRef, ReviewStatus};
use symphony_tasks::models::repository::RepositoryProfile;
use symphony_tasks::models::run_record::RunStatus;
use symphony_tasks::models::workflow::WorkflowDefinition;
use symphony_tasks::state_store::StateStore;
use symphony_tasks::tracker::Tracker;
use symphony_tasks::tracker::types::{CommentRequest, CreatePrRequest, PrStatus};

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-app-runtime-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
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

#[derive(Clone)]
struct FakeTracker {
    issues: Arc<Mutex<Vec<NormalizedIssue>>>,
    updated_states: Arc<Mutex<Vec<(String, String)>>>,
    closed_issues: Arc<Mutex<Vec<String>>>,
}

impl FakeTracker {
    fn new(issues: Vec<NormalizedIssue>) -> Self {
        Self {
            issues: Arc::new(Mutex::new(issues)),
            updated_states: Arc::new(Mutex::new(Vec::new())),
            closed_issues: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Tracker for FakeTracker {
    async fn fetch_candidate_issues(
        &self,
        _repo: &RepositoryProfile,
    ) -> anyhow::Result<Vec<NormalizedIssue>> {
        Ok(self.issues.lock().unwrap().clone())
    }

    async fn fetch_issue(
        &self,
        _repo: &RepositoryProfile,
        issue_id: &str,
    ) -> anyhow::Result<NormalizedIssue> {
        self.issues
            .lock()
            .unwrap()
            .iter()
            .find(|issue| issue.id == issue_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing issue {issue_id}"))
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
        Ok(())
    }

    async fn create_or_update_pr(
        &self,
        _repo: &RepositoryProfile,
        _request: CreatePrRequest,
    ) -> anyhow::Result<PullRequestRef> {
        Ok(PullRequestRef {
            id: "9".into(),
            number: 9,
            url: "https://gitcode.example/demo/pulls/9".into(),
            head_branch: "feat/demo-42".into(),
            state: "open".into(),
            review_status: ReviewStatus::Pending,
            merge_status: MergeStatus::Mergeable,
        })
    }

    async fn get_pr_status(
        &self,
        _repo: &RepositoryProfile,
        _pr_ref: &str,
    ) -> anyhow::Result<PrStatus> {
        Ok(PrStatus {
            pr: PullRequestRef {
                id: "9".into(),
                number: 9,
                url: "https://gitcode.example/demo/pulls/9".into(),
                head_branch: "feat/demo-42".into(),
                state: "open".into(),
                review_status: ReviewStatus::Pending,
                merge_status: MergeStatus::Mergeable,
            },
        })
    }

    async fn merge_pr(&self, _repo: &RepositoryProfile, _pr_ref: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn close_issue(&self, _repo: &RepositoryProfile, issue_id: &str) -> anyhow::Result<()> {
        self.closed_issues
            .lock()
            .unwrap()
            .push(issue_id.to_string());
        Ok(())
    }
}

#[derive(Clone)]
struct FakeRunner;

#[async_trait]
impl AgentRunner for FakeRunner {
    async fn run(
        &self,
        _workspace_path: &Path,
        _issue: &NormalizedIssue,
        _workflow: &WorkflowDefinition,
    ) -> Result<AgentRunResult, RunnerError> {
        Ok(AgentRunResult {
            status: AgentRunStatus::Success,
            summary: "implemented".into(),
            branch_name: Some("feat/demo-42".into()),
            commit_sha: Some("abc123".into()),
            requested_next_action: None,
        })
    }
}

fn write_runtime_fixture(root: &Path) -> OrchestratorConfig {
    fs::create_dir_all(root.join("config/repositories")).unwrap();
    fs::create_dir_all(root.join("repo")).unwrap();
    fs::write(
        root.join("repo/WORKFLOW.md"),
        r#"---
active_states:
  - Todo
terminal_states:
  - Done
retry_policy:
  max_attempts: 3
  backoff_seconds: 60
pr_policy:
  require_pr: true
completion_policy:
  close_issue_on_merge: true
---
Implement {{issue_title}}
"#,
    )
    .unwrap();
    fs::write(
        root.join("config/orchestrator.toml"),
        r#"
poll_interval_secs = 30
global_concurrency = 1
log_level = "info"
state_root = "var/state"
workspace_root = "var/workspaces"
lock_path = "var/locks/daemon.lock"
default_tracker_kind = "github"
github_token_env = "GITHUB_TOKEN"
default_runner = "process"
repositories_dir = "config/repositories"
runner_program = "/bin/sh"
runner_args = ["-lc", "printf '{\"status\":\"success\",\"summary\":\"ok\"}'"]
"#,
    )
    .unwrap();
    fs::write(
        root.join("config/repositories/demo.toml"),
        r#"
repo_id = "demo"
repo_path = "repo"
workflow_path = "repo/WORKFLOW.md"
tracker_kind = "gitcode"
tracker_project_ref = "acme/demo"
default_runner = "process"
enabled = true
max_concurrent_runs = 1
"#,
    )
    .unwrap();

    OrchestratorConfig::load_from_file(root.join("config/orchestrator.toml")).unwrap()
}

#[tokio::test]
async fn reconcile_once_dispatches_issue_and_creates_pr_watch() {
    let root = unique_temp_dir("reconcile-once");
    let config = write_runtime_fixture(&root);
    let tracker = FakeTracker::new(vec![issue()]);
    let runner = FakeRunner;

    let summary = reconcile_once_with(&config, &tracker, &runner)
        .await
        .unwrap();

    assert_eq!(summary.dispatched_runs, 1);
    assert_eq!(summary.reconciled_prs, 0);
    assert_eq!(
        tracker.updated_states.lock().unwrap().as_slice(),
        &[
            ("100".to_string(), "In Progress".to_string()),
            ("100".to_string(), "Human Review".to_string()),
        ]
    );

    let state_root = config.state_root.parent().unwrap();
    let state_store = StateStore::new(state_root);
    let run_record = state_store.load_run_record("demo", "100").unwrap();
    assert_eq!(run_record.status, RunStatus::AwaitingHumanReview);
    assert_eq!(run_record.pr_ref.as_deref(), Some("9"));

    let pr_watch = state_store.load_pr_watch_state().unwrap();
    assert_eq!(pr_watch.len(), 1);
    assert_eq!(pr_watch[0].issue_id, "100");
    assert_eq!(pr_watch[0].pr_ref, "9");
}
