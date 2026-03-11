use std::path::PathBuf;

use symphony_tasks::models::repository::RepositoryProfile;
use symphony_tasks::tracker::Tracker;
use symphony_tasks::tracker::github::client::GitHubClient;
use symphony_tasks::tracker::types::{CommentRequest, CreatePrRequest};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn repo_profile() -> RepositoryProfile {
    RepositoryProfile {
        repo_id: "example".into(),
        repo_path: PathBuf::from("/tmp/example"),
        workflow_path: PathBuf::from("/tmp/example/WORKFLOW.md"),
        tracker_kind: "github".into(),
        tracker_project_ref: "acme/example".into(),
        default_runner: "process".into(),
        enabled: true,
        max_concurrent_runs: 1,
    }
}

#[tokio::test]
async fn fetches_candidate_issues_from_github() {
    let server = MockServer::start().await;
    let response = vec![
        serde_json::json!({
            "id": 100,
            "number": 42,
            "title": "Implement orchestrator",
            "body": "Build the GitHub adapter",
            "state": "open",
            "labels": [{"name": "todo"}, {"name": "backend"}],
            "html_url": "https://github.com/acme/example/issues/42",
            "pull_request": null,
            "created_at": "2026-03-10T12:00:00Z",
            "updated_at": "2026-03-10T12:05:00Z"
        }),
        serde_json::json!({
            "id": 101,
            "number": 43,
            "title": "Already a PR",
            "body": null,
            "state": "open",
            "labels": [{"name": "todo"}],
            "html_url": "https://github.com/acme/example/pull/43",
            "pull_request": {"url": "https://api.github.com/repos/acme/example/pulls/43"},
            "created_at": null,
            "updated_at": null
        }),
    ];

    Mock::given(method("GET"))
        .and(path("/repos/acme/example/issues"))
        .and(query_param("state", "open"))
        .and(header("authorization", "Bearer token"))
        .and(header("accept", "application/vnd.github+json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&server)
        .await;

    let client = GitHubClient::new(server.uri(), "token");
    let issues = client
        .fetch_candidate_issues(&repo_profile())
        .await
        .unwrap();

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].identifier, "example#42");
    assert_eq!(issues[0].state, "Todo");
}

#[tokio::test]
async fn fetches_single_github_issue() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/example/issues/42"))
        .and(header("authorization", "Bearer token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 100,
            "number": 42,
            "title": "Implement orchestrator",
            "body": null,
            "state": "open",
            "labels": [{"name": "todo"}],
            "html_url": null,
            "pull_request": null,
            "created_at": null,
            "updated_at": null
        })))
        .mount(&server)
        .await;

    let client = GitHubClient::new(server.uri(), "token");
    let issue = client.fetch_issue(&repo_profile(), "42").await.unwrap();

    assert_eq!(issue.id, "42");
    assert_eq!(issue.state, "Todo");
}

#[tokio::test]
async fn replaces_workflow_state_labels_and_posts_comment() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/example/issues/42/labels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![
            serde_json::json!({"name": "todo"}),
            serde_json::json!({"name": "backend"}),
        ]))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/repos/acme/example/issues/42/labels"))
        .and(body_json(
            serde_json::json!({"labels": ["backend", "human-review"]}),
        ))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/example/issues/42/comments"))
        .and(body_json(serde_json::json!({"body": "started"})))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let client = GitHubClient::new(server.uri(), "token");
    client
        .update_issue_state(&repo_profile(), "42", "Human Review")
        .await
        .unwrap();
    client
        .add_comment(
            &repo_profile(),
            CommentRequest {
                issue_id: "42".into(),
                body: "started".into(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn creates_pr_queries_status_merges_and_closes_issue() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/example/pulls"))
        .and(body_json(serde_json::json!({
            "title": "Implement orchestrator",
            "body": "details",
            "head": "feat/demo-42",
            "base": "main"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 9,
            "number": 9,
            "html_url": "https://github.com/acme/example/pull/9",
            "state": "open",
            "head": {"ref": "feat/demo-42"},
            "mergeable": true,
            "merged": false,
            "review_decision": null
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/example/pulls/9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 9,
            "number": 9,
            "html_url": "https://github.com/acme/example/pull/9",
            "state": "open",
            "head": {"ref": "feat/demo-42"},
            "mergeable": true,
            "merged": false,
            "review_decision": null
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/example/pulls/9/reviews"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(vec![serde_json::json!({"state": "APPROVED"})]),
        )
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/repos/acme/example/pulls/9/merge"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/repos/acme/example/issues/42"))
        .and(body_json(serde_json::json!({"state": "closed"})))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let client = GitHubClient::new(server.uri(), "token");
    let pr = client
        .create_or_update_pr(
            &repo_profile(),
            CreatePrRequest {
                issue_id: "100".into(),
                title: "Implement orchestrator".into(),
                body: "details".into(),
                head_branch: "feat/demo-42".into(),
                base_branch: "main".into(),
            },
        )
        .await
        .unwrap();
    let status = client.get_pr_status(&repo_profile(), "9").await.unwrap();
    client.merge_pr(&repo_profile(), "9").await.unwrap();
    client.close_issue(&repo_profile(), "42").await.unwrap();

    assert_eq!(pr.number, 9);
    assert_eq!(status.pr.number, 9);
}

#[tokio::test]
async fn returns_merged_status_for_closed_github_prs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/example/pulls/9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 19,
            "number": 9,
            "html_url": "https://github.com/acme/example/pull/9",
            "state": "closed",
            "head": {"ref": "feat/demo-42"},
            "mergeable": false,
            "merged": true,
            "review_decision": null
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/example/pulls/9/reviews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Vec::<serde_json::Value>::new()))
        .mount(&server)
        .await;

    let client = GitHubClient::new(server.uri(), "token");
    let status = client.get_pr_status(&repo_profile(), "9").await.unwrap();

    assert_eq!(status.pr.id, "9");
    assert_eq!(status.pr.state, "closed");
    assert_eq!(
        status.pr.merge_status,
        symphony_tasks::models::pr::MergeStatus::Merged
    );
}
