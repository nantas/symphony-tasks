use symphony_tasks::models::issue::NormalizedIssue;
use symphony_tasks::models::pr::{MergeStatus, PullRequestRef, ReviewStatus};
use symphony_tasks::models::repository::RepositoryProfile;
use symphony_tasks::models::run_record::{RunRecord, RunStatus};

#[test]
fn repository_profile_roundtrips() {
    let profile = RepositoryProfile {
        repo_id: "demo".into(),
        repo_path: "/tmp/demo".into(),
        workflow_path: "/tmp/demo/WORKFLOW.md".into(),
        gitcode_project_ref: "acme/demo".into(),
        default_runner: "process".into(),
        enabled: true,
        max_concurrent_runs: 1,
    };

    let json = serde_json::to_string(&profile).unwrap();
    let decoded: RepositoryProfile = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, profile);
}

#[test]
fn normalized_issue_roundtrips() {
    let issue = NormalizedIssue {
        id: "100".into(),
        identifier: "demo#42".into(),
        repo_id: "demo".into(),
        title: "Implement orchestrator".into(),
        description: Some("Build the first slice".into()),
        state: "Todo".into(),
        priority: Some(1),
        labels: vec!["backend".into(), "automation".into()],
        url: Some("https://gitcode.example/demo/issues/42".into()),
        created_at: Some("2026-03-10T12:00:00Z".into()),
        updated_at: Some("2026-03-10T12:05:00Z".into()),
    };

    let json = serde_json::to_string(&issue).unwrap();
    let decoded: NormalizedIssue = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, issue);
}

#[test]
fn run_record_roundtrips() {
    let record = RunRecord {
        issue_id: "100".into(),
        repo_id: "demo".into(),
        attempt: 1,
        workspace_path: "/tmp/workspaces/demo-42".into(),
        status: RunStatus::AwaitingHumanReview,
        branch_name: Some("feat/demo-42".into()),
        commit_sha: Some("abc123".into()),
        pr_ref: Some("pr-9".into()),
        started_at: "2026-03-10T12:00:00Z".into(),
        updated_at: "2026-03-10T12:10:00Z".into(),
        last_error: None,
        next_retry_at: None,
    };

    let json = serde_json::to_string(&record).unwrap();
    let decoded: RunRecord = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, record);
}

#[test]
fn pull_request_ref_roundtrips() {
    let pr = PullRequestRef {
        id: "pr-9".into(),
        number: 9,
        url: "https://gitcode.example/demo/pulls/9".into(),
        head_branch: "feat/demo-42".into(),
        state: "open".into(),
        review_status: ReviewStatus::Approved,
        merge_status: MergeStatus::Mergeable,
    };

    let json = serde_json::to_string(&pr).unwrap();
    let decoded: PullRequestRef = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, pr);
}
