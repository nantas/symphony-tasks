use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use symphony_tasks::models::issue::NormalizedIssue;
use symphony_tasks::models::pr::{MergeStatus, PullRequestRef, ReviewStatus};
use symphony_tasks::models::repository::RepositoryProfile;
use symphony_tasks::models::run_record::{RunRecord, RunStatus};
use symphony_tasks::models::workflow::{
    CompletionPolicy, PrPolicy, RetryPolicy, WorkflowDefinition, WorkflowHooks,
};
use symphony_tasks::orchestrator::reconcile::{
    PrLifecycleRequest, WatchPrRequest, create_pr_for_run, reconcile_pr_watch,
};
use symphony_tasks::state_store::StateStore;
use symphony_tasks::tracker::Tracker;
use symphony_tasks::tracker::types::{CommentRequest, CreatePrRequest, PrStatus};

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-pr-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn repo_profile(root: &std::path::Path) -> RepositoryProfile {
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

fn issue() -> NormalizedIssue {
    NormalizedIssue {
        id: "100".into(),
        identifier: "demo#42".into(),
        repo_id: "demo".into(),
        title: "Implement orchestrator".into(),
        description: Some("Build the review loop".into()),
        state: "Todo".into(),
        priority: Some(1),
        labels: vec![],
        url: None,
        created_at: None,
        updated_at: None,
    }
}

fn awaiting_pr_record(root: &std::path::Path) -> RunRecord {
    RunRecord {
        issue_id: "100".into(),
        repo_id: "demo".into(),
        attempt: 1,
        workspace_path: root.join("var/workspaces/demo/demo-42"),
        status: RunStatus::AwaitingPrCreation,
        branch_name: Some("feat/demo-42".into()),
        commit_sha: Some("abc123".into()),
        pr_ref: None,
        started_at: "2026-03-10T12:00:00Z".into(),
        updated_at: "2026-03-10T12:00:00Z".into(),
        last_error: None,
        next_retry_at: None,
    }
}

#[derive(Clone)]
struct FakeTracker {
    created_pr: PullRequestRef,
    watched_pr: PullRequestRef,
    updated_states: Arc<Mutex<Vec<(String, String)>>>,
    merged_prs: Arc<Mutex<Vec<String>>>,
}

impl FakeTracker {
    fn new(created_pr: PullRequestRef, watched_pr: PullRequestRef) -> Self {
        Self {
            created_pr,
            watched_pr,
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
        Ok(self.created_pr.clone())
    }

    async fn get_pr_status(
        &self,
        _repo: &RepositoryProfile,
        _pr_ref: &str,
    ) -> anyhow::Result<PrStatus> {
        Ok(PrStatus {
            pr: self.watched_pr.clone(),
        })
    }

    async fn merge_pr(&self, _repo: &RepositoryProfile, pr_ref: &str) -> anyhow::Result<()> {
        self.merged_prs.lock().unwrap().push(pr_ref.to_string());
        Ok(())
    }
}

#[tokio::test]
async fn creates_pr_and_moves_issue_to_human_review() {
    let root = unique_temp_dir("create-pr");
    let repo = repo_profile(&root);
    let state_store = StateStore::new(root.join("var"));
    let run_record = awaiting_pr_record(&root);
    state_store.save_run_record(&run_record).unwrap();

    let tracker = FakeTracker::new(
        PullRequestRef {
            id: "9".into(),
            number: 9,
            url: "https://gitcode.example/demo/pulls/9".into(),
            head_branch: "feat/demo-42".into(),
            state: "open".into(),
            review_status: ReviewStatus::Pending,
            merge_status: MergeStatus::Mergeable,
        },
        PullRequestRef {
            id: "9".into(),
            number: 9,
            url: "https://gitcode.example/demo/pulls/9".into(),
            head_branch: "feat/demo-42".into(),
            state: "open".into(),
            review_status: ReviewStatus::Pending,
            merge_status: MergeStatus::Mergeable,
        },
    );

    let updated = create_pr_for_run(
        &tracker,
        &state_store,
        PrLifecycleRequest {
            repo: &repo,
            issue: &issue(),
            workflow: &workflow(),
            run_record,
            base_branch: "main",
            updated_at: "2026-03-10T12:05:00Z",
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.status, RunStatus::AwaitingHumanReview);
    assert_eq!(updated.pr_ref.as_deref(), Some("9"));
    assert_eq!(
        tracker.updated_states.lock().unwrap().as_slice(),
        &[("100".to_string(), "Human Review".to_string())]
    );
    let pr_watch = state_store.load_pr_watch_state().unwrap();
    assert_eq!(pr_watch.len(), 1);
    assert_eq!(pr_watch[0].pr_ref, "9");
}

#[tokio::test]
async fn merges_approved_pr_and_moves_issue_to_done() {
    let root = unique_temp_dir("merge-pr");
    let repo = repo_profile(&root);
    let state_store = StateStore::new(root.join("var"));
    let run_record = RunRecord {
        pr_ref: Some("9".into()),
        status: RunStatus::AwaitingHumanReview,
        updated_at: "2026-03-10T12:05:00Z".into(),
        ..awaiting_pr_record(&root)
    };
    state_store.save_run_record(&run_record).unwrap();
    state_store
        .save_pr_watch_state(&[symphony_tasks::state_store::PrWatchEntry {
            issue_id: "100".into(),
            repo_id: "demo".into(),
            pr_ref: "9".into(),
            status: "awaiting_human_review".into(),
        }])
        .unwrap();

    let tracker = FakeTracker::new(
        PullRequestRef {
            id: "9".into(),
            number: 9,
            url: "https://gitcode.example/demo/pulls/9".into(),
            head_branch: "feat/demo-42".into(),
            state: "open".into(),
            review_status: ReviewStatus::Approved,
            merge_status: MergeStatus::Mergeable,
        },
        PullRequestRef {
            id: "9".into(),
            number: 9,
            url: "https://gitcode.example/demo/pulls/9".into(),
            head_branch: "feat/demo-42".into(),
            state: "open".into(),
            review_status: ReviewStatus::Approved,
            merge_status: MergeStatus::Mergeable,
        },
    );

    let updated = reconcile_pr_watch(
        &tracker,
        &state_store,
        WatchPrRequest {
            repo: &repo,
            issue: &issue(),
            workflow: &workflow(),
            run_record,
            updated_at: "2026-03-10T12:10:00Z",
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.status, RunStatus::Completed);
    assert_eq!(
        tracker.updated_states.lock().unwrap().as_slice(),
        &[("100".to_string(), "Done".to_string())]
    );
    assert_eq!(
        tracker.merged_prs.lock().unwrap().as_slice(),
        &["9".to_string()]
    );
    assert!(state_store.load_pr_watch_state().unwrap().is_empty());
}
