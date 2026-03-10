use std::fs;

use symphony_tasks::models::run_record::{RunRecord, RunStatus};
use symphony_tasks::state_store::{PrWatchEntry, RetryEntry, StateStore};

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-state-store-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn saves_and_loads_run_records_in_per_issue_layout() {
    let root = unique_temp_dir("run-record");
    let store = StateStore::new(root.join("var"));
    let record = RunRecord {
        issue_id: "100".into(),
        repo_id: "demo".into(),
        attempt: 1,
        workspace_path: "/tmp/demo/100".into(),
        status: RunStatus::RunningAgent,
        branch_name: Some("feat/demo-100".into()),
        commit_sha: Some("abc123".into()),
        pr_ref: None,
        started_at: "2026-03-10T12:00:00Z".into(),
        updated_at: "2026-03-10T12:02:00Z".into(),
        last_error: None,
        next_retry_at: None,
    };

    store.save_run_record(&record).unwrap();

    let expected_path = root.join("var/runs/demo/100.json");
    assert!(expected_path.exists());

    let loaded = store.load_run_record("demo", "100").unwrap();
    assert_eq!(loaded, record);
}

#[test]
fn saves_and_loads_retry_queue() {
    let root = unique_temp_dir("retry-queue");
    let store = StateStore::new(root.join("var"));
    let entries = vec![
        RetryEntry {
            issue_id: "100".into(),
            identifier: "demo#100".into(),
            attempt: 2,
            due_at: "2026-03-10T12:10:00Z".into(),
            error: Some("transient".into()),
        },
        RetryEntry {
            issue_id: "101".into(),
            identifier: "demo#101".into(),
            attempt: 1,
            due_at: "2026-03-10T12:11:00Z".into(),
            error: None,
        },
    ];

    store.save_retry_queue(&entries).unwrap();
    let loaded = store.load_retry_queue().unwrap();

    assert_eq!(loaded, entries);
}

#[test]
fn saves_and_loads_pr_watch_state() {
    let root = unique_temp_dir("pr-watch");
    let store = StateStore::new(root.join("var"));
    let entries = vec![PrWatchEntry {
        issue_id: "100".into(),
        repo_id: "demo".into(),
        pr_ref: "pr-9".into(),
        status: "awaiting_human_review".into(),
    }];

    store.save_pr_watch_state(&entries).unwrap();
    let loaded = store.load_pr_watch_state().unwrap();

    assert_eq!(loaded, entries);
}
