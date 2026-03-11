use symphony_tasks::models::pr::{MergeStatus, ReviewStatus};
use symphony_tasks::tracker::github::models::{
    GitHubIssue, GitHubLabel, GitHubPullRequest, GitHubPullRequestHead,
};

#[test]
fn maps_github_issue_state_label_to_normalized_state() {
    let issue = GitHubIssue {
        id: 100,
        number: 42,
        title: "Implement orchestrator".into(),
        body: Some("Build the GitHub adapter".into()),
        state: "open".into(),
        labels: vec![
            GitHubLabel {
                name: "todo".into(),
            },
            GitHubLabel {
                name: "backend".into(),
            },
        ],
        html_url: Some("https://github.com/acme/example/issues/42".into()),
        pull_request: None,
        created_at: Some("2026-03-10T12:00:00Z".into()),
        updated_at: Some("2026-03-10T12:05:00Z".into()),
    };

    let normalized = issue.to_normalized_issue("example");

    assert_eq!(normalized.id, "42");
    assert_eq!(normalized.state, "Todo");
    assert!(normalized.labels.contains(&"todo".to_string()));
}

#[test]
fn falls_back_to_open_when_no_workflow_label_exists() {
    let issue = GitHubIssue {
        id: 101,
        number: 43,
        title: "Fallback state".into(),
        body: None,
        state: "open".into(),
        labels: vec![GitHubLabel {
            name: "backend".into(),
        }],
        html_url: None,
        pull_request: None,
        created_at: None,
        updated_at: None,
    };

    let normalized = issue.to_normalized_issue("example");

    assert_eq!(normalized.state, "open");
}

#[test]
fn treats_multiple_workflow_labels_as_ambiguous() {
    let issue = GitHubIssue {
        id: 102,
        number: 44,
        title: "Ambiguous labels".into(),
        body: None,
        state: "open".into(),
        labels: vec![
            GitHubLabel {
                name: "todo".into(),
            },
            GitHubLabel {
                name: "in-progress".into(),
            },
        ],
        html_url: None,
        pull_request: None,
        created_at: None,
        updated_at: None,
    };

    let normalized = issue.to_normalized_issue("example");

    assert_eq!(normalized.state, "ambiguous");
}

#[test]
fn maps_github_pr_review_and_merge_status() {
    let pr = GitHubPullRequest {
        id: 338,
        number: 9,
        html_url: "https://github.com/acme/example/pull/9".into(),
        state: "open".into(),
        head: GitHubPullRequestHead {
            r#ref: "feat/demo-42".into(),
        },
        mergeable: Some(true),
        merged: false,
        review_decision: Some("APPROVED".into()),
    };

    let normalized = pr.to_pull_request_ref();

    assert_eq!(normalized.id, "9");
    assert_eq!(normalized.review_status, ReviewStatus::Approved);
    assert_eq!(normalized.merge_status, MergeStatus::Mergeable);
}
