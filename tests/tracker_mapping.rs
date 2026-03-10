use symphony_tasks::tracker::gitcode::models::{GitCodeIssue, GitCodeLabel, GitCodePullRequest};

#[test]
fn maps_gitcode_issue_to_normalized_issue() {
    let issue = GitCodeIssue {
        id: 100,
        number: 42,
        title: "Implement orchestrator".into(),
        body: Some("Build the GitCode adapter".into()),
        state: "open".into(),
        issue_state: Some("Todo".into()),
        labels: vec![
            GitCodeLabel {
                name: "backend".into(),
            },
            GitCodeLabel {
                name: "automation".into(),
            },
        ],
        priority: Some(1),
        html_url: Some("https://gitcode.example/demo/issues/42".into()),
        created_at: Some("2026-03-10T12:00:00Z".into()),
        updated_at: Some("2026-03-10T12:05:00Z".into()),
    };

    let normalized = issue.to_normalized_issue("demo");

    assert_eq!(normalized.id, "100");
    assert_eq!(normalized.identifier, "demo#42");
    assert_eq!(normalized.state, "Todo");
    assert_eq!(normalized.labels, vec!["backend", "automation"]);
}

#[test]
fn falls_back_to_top_level_state_when_issue_state_missing() {
    let issue = GitCodeIssue {
        id: 101,
        number: 43,
        title: "Fallback state".into(),
        body: None,
        state: "closed".into(),
        issue_state: None,
        labels: vec![],
        priority: None,
        html_url: None,
        created_at: None,
        updated_at: None,
    };

    let normalized = issue.to_normalized_issue("demo");

    assert_eq!(normalized.state, "closed");
}

#[test]
fn maps_gitcode_pr_to_pull_request_ref() {
    let pr = GitCodePullRequest {
        id: 9,
        number: 9,
        html_url: "https://gitcode.example/demo/pulls/9".into(),
        state: "open".into(),
        head: "feat/demo-42".into(),
        merge_status: Some("can_be_merged".into()),
        review_status: Some("approved".into()),
    };

    let normalized = pr.to_pull_request_ref();

    assert_eq!(normalized.id, "9");
    assert_eq!(normalized.number, 9);
    assert_eq!(normalized.head_branch, "feat/demo-42");
}
