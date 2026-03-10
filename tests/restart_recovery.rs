use std::path::PathBuf;

use symphony_tasks::app::lock::DaemonLock;
use symphony_tasks::app::recover_runtime_state;
use symphony_tasks::models::run_record::{RunRecord, RunStatus};
use symphony_tasks::state_store::{PrWatchEntry, RetryEntry, StateStore};

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-recovery-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_record(root: &std::path::Path, issue_id: &str, status: RunStatus) -> RunRecord {
    RunRecord {
        issue_id: issue_id.into(),
        repo_id: "demo".into(),
        attempt: 1,
        workspace_path: root.join(format!("var/workspaces/demo/{issue_id}")),
        status,
        branch_name: Some(format!("feat/demo-{issue_id}")),
        commit_sha: None,
        pr_ref: None,
        started_at: "2026-03-10T12:00:00Z".into(),
        updated_at: "2026-03-10T12:05:00Z".into(),
        last_error: None,
        next_retry_at: None,
    }
}

#[test]
fn rebuilds_retry_queue_after_restart() {
    let root = unique_temp_dir("retry");
    let store = StateStore::new(root.join("var"));
    let expected = vec![RetryEntry {
        issue_id: "100".into(),
        identifier: "demo#100".into(),
        attempt: 2,
        due_at: "2026-03-10T12:10:00Z".into(),
        error: Some("transient".into()),
    }];
    store.save_retry_queue(&expected).unwrap();

    let recovered = recover_runtime_state(&store).unwrap();

    assert_eq!(recovered.retry_queue, expected);
}

#[test]
fn recovers_pr_watch_tasks_after_restart() {
    let root = unique_temp_dir("pr-watch");
    let store = StateStore::new(root.join("var"));
    let expected = vec![PrWatchEntry {
        issue_id: "100".into(),
        repo_id: "demo".into(),
        pr_ref: "9".into(),
        status: "awaiting_human_review".into(),
    }];
    store.save_pr_watch_state(&expected).unwrap();

    let recovered = recover_runtime_state(&store).unwrap();

    assert_eq!(recovered.pr_watch_entries, expected);
}

#[test]
fn detects_interrupted_runs_without_active_process() {
    let root = unique_temp_dir("interrupted");
    let store = StateStore::new(root.join("var"));
    store
        .save_run_record(&run_record(&root, "100", RunStatus::RunningAgent))
        .unwrap();
    store
        .save_run_record(&run_record(&root, "101", RunStatus::AwaitingHumanReview))
        .unwrap();
    store
        .save_run_record(&run_record(&root, "102", RunStatus::PreparingWorkspace))
        .unwrap();

    let recovered = recover_runtime_state(&store).unwrap();

    assert_eq!(
        recovered.interrupted_issue_ids,
        vec!["100".to_string(), "102".to_string()]
    );
}

#[test]
fn refuses_second_daemon_instance_when_lock_is_held() {
    let root = unique_temp_dir("lock");
    let lock_path = root.join("var/locks/daemon.lock");

    let _first = DaemonLock::acquire(&lock_path).unwrap();
    let second = DaemonLock::acquire(&lock_path);

    assert!(second.is_err());
}
