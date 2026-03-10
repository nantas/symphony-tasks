use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use symphony_tasks::agent_runner::AgentRunner;
use symphony_tasks::agent_runner::types::{AgentRunResult, AgentRunStatus, RunnerError};
use symphony_tasks::models::issue::NormalizedIssue;
use symphony_tasks::models::pr::{MergeStatus, PullRequestRef, ReviewStatus};
use symphony_tasks::models::repository::RepositoryProfile;
use symphony_tasks::models::workflow::{
    CompletionPolicy, PrPolicy, RetryPolicy, WorkflowDefinition, WorkflowHooks,
};
use symphony_tasks::orchestrator::reconcile::{
    DispatchRequest, PrLifecycleRequest, WatchPrRequest, create_pr_for_run, dispatch_issue,
    reconcile_pr_watch,
};
use symphony_tasks::state_store::StateStore;
use symphony_tasks::tracker::Tracker;
use symphony_tasks::tracker::types::{CommentRequest, CreatePrRequest, PrStatus};
use symphony_tasks::workspace::WorkspaceManager;

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-e2e-{name}-{}-{}",
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
        description: Some("Build full lifecycle fixture".into()),
        state: "Todo".into(),
        priority: Some(1),
        labels: vec!["backend".into()],
        url: None,
        created_at: None,
        updated_at: None,
    }
}

#[derive(Clone)]
struct FakeTracker {
    updated_states: Arc<Mutex<Vec<String>>>,
    merged_prs: Arc<Mutex<Vec<String>>>,
}

impl FakeTracker {
    fn new() -> Self {
        Self {
            updated_states: Arc::new(Mutex::new(Vec::new())),
            merged_prs: Arc::new(Mutex::new(Vec::new())),
        }
    }
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
        _issue_id: &str,
        state: &str,
    ) -> anyhow::Result<()> {
        self.updated_states.lock().unwrap().push(state.to_string());
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
                review_status: ReviewStatus::Approved,
                merge_status: MergeStatus::Mergeable,
            },
        })
    }

    async fn merge_pr(&self, _repo: &RepositoryProfile, pr_ref: &str) -> anyhow::Result<()> {
        self.merged_prs.lock().unwrap().push(pr_ref.to_string());
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

#[tokio::test]
async fn drives_one_issue_through_dispatch_pr_and_merge() {
    let root = unique_temp_dir("lifecycle");
    std::fs::create_dir_all(root.join("repo")).unwrap();
    let repo = repo_profile(&root);
    let workflow = workflow();
    let issue = issue();
    let tracker = FakeTracker::new();
    let runner = FakeRunner;
    let workspace_manager = WorkspaceManager::new(root.join("var/workspaces"));
    let state_store = StateStore::new(root.join("var"));

    let dispatched = dispatch_issue(
        &tracker,
        &runner,
        &workspace_manager,
        &state_store,
        DispatchRequest {
            repo: &repo,
            issue: &issue,
            workflow: &workflow,
            started_at: "2026-03-10T12:00:00Z",
        },
    )
    .await
    .unwrap();

    let with_pr = create_pr_for_run(
        &tracker,
        &state_store,
        PrLifecycleRequest {
            repo: &repo,
            issue: &issue,
            workflow: &workflow,
            run_record: dispatched.run_record,
            base_branch: "main",
            updated_at: "2026-03-10T12:05:00Z",
        },
    )
    .await
    .unwrap();

    let completed = reconcile_pr_watch(
        &tracker,
        &state_store,
        WatchPrRequest {
            repo: &repo,
            issue: &issue,
            workflow: &workflow,
            run_record: with_pr,
            updated_at: "2026-03-10T12:10:00Z",
        },
    )
    .await
    .unwrap();

    assert_eq!(
        completed.status,
        symphony_tasks::models::run_record::RunStatus::Completed
    );
    assert_eq!(
        tracker.updated_states.lock().unwrap().as_slice(),
        &[
            "In Progress".to_string(),
            "Human Review".to_string(),
            "Done".to_string()
        ]
    );
    assert_eq!(
        tracker.merged_prs.lock().unwrap().as_slice(),
        &["9".to_string()]
    );
    assert!(state_store.load_pr_watch_state().unwrap().is_empty());
}

#[test]
fn ships_readme_and_example_gitcode_config() {
    let readme = std::fs::read_to_string("README.md").unwrap();
    assert!(readme.contains("reconcile-once"));
    assert!(readme.contains("validate-config"));
    assert!(std::path::Path::new("config/repositories/example-gitcode.toml").exists());
}
