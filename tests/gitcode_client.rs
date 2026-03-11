use std::path::PathBuf;

use symphony_tasks::models::repository::RepositoryProfile;
use symphony_tasks::tracker::Tracker;
use symphony_tasks::tracker::gitcode::client::GitCodeClient;
use symphony_tasks::tracker::types::{CommentRequest, CreatePrRequest};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn repo_profile() -> RepositoryProfile {
    RepositoryProfile {
        repo_id: "example".into(),
        repo_path: PathBuf::from("/tmp/example"),
        workflow_path: PathBuf::from("/tmp/example/WORKFLOW.md"),
        tracker_kind: "gitcode".into(),
        tracker_project_ref: "acme/example".into(),
        default_runner: "process".into(),
        enabled: true,
        max_concurrent_runs: 1,
    }
}

#[tokio::test]
async fn fetches_candidate_issues() {
    let server = MockServer::start().await;
    let response = vec![serde_json::json!({
        "id": 100,
        "number": 42,
        "title": "Implement orchestrator",
        "body": "Build the GitCode adapter",
        "state": "open",
        "issue_state": "Todo",
        "labels": [{"name": "backend"}],
        "priority": 1,
        "html_url": "https://gitcode.example/demo/issues/42",
        "created_at": "2026-03-10T12:00:00Z",
        "updated_at": "2026-03-10T12:05:00Z"
    })];

    Mock::given(method("GET"))
        .and(path("/api/v5/repos/acme/example/issues"))
        .and(query_param("state", "open"))
        .and(header("private-token", "token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&server)
        .await;

    let client = GitCodeClient::new(server.uri(), "token");
    let issues = client
        .fetch_candidate_issues(&repo_profile())
        .await
        .unwrap();

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].identifier, "example#42");
}

#[tokio::test]
async fn fetches_single_issue() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/acme/example/issues/100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 100,
            "number": 42,
            "title": "Implement orchestrator",
            "body": null,
            "state": "open",
            "issue_state": "Todo",
            "labels": [],
            "priority": null,
            "html_url": null,
            "created_at": null,
            "updated_at": null
        })))
        .mount(&server)
        .await;

    let client = GitCodeClient::new(server.uri(), "token");
    let issue = client.fetch_issue(&repo_profile(), "100").await.unwrap();

    assert_eq!(issue.id, "100");
}

#[tokio::test]
async fn updates_issue_state_and_posts_comment() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/v5/repos/acme/example/issues/100"))
        .and(body_json(serde_json::json!({"state": "In Progress"})))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/acme/example/issues/100/comments"))
        .and(body_json(serde_json::json!({"body": "started"})))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let client = GitCodeClient::new(server.uri(), "token");
    client
        .update_issue_state(&repo_profile(), "100", "In Progress")
        .await
        .unwrap();
    client
        .add_comment(
            &repo_profile(),
            CommentRequest {
                issue_id: "100".into(),
                body: "started".into(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn creates_pr_queries_status_and_merges() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/acme/example/pulls"))
        .and(body_json(serde_json::json!({
            "title": "Implement orchestrator",
            "body": "details",
            "head": "feat/demo-42",
            "base": "main"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 9,
            "number": 9,
            "html_url": "https://gitcode.example/demo/pulls/9",
            "state": "open",
            "head": "feat/demo-42",
            "merge_status": "can_be_merged",
            "review_status": "approved"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/acme/example/pulls/9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 9,
            "number": 9,
            "html_url": "https://gitcode.example/demo/pulls/9",
            "state": "open",
            "head": "feat/demo-42",
            "merge_status": "can_be_merged",
            "review_status": "approved"
        })))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v5/repos/acme/example/pulls/9/merge"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let client = GitCodeClient::new(server.uri(), "token");
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

    assert_eq!(pr.number, 9);
    assert_eq!(status.pr.number, 9);
}
