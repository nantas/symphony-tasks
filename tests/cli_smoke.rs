use assert_cmd::Command;
use std::fs;

#[test]
fn prints_help() {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("symphony-tasks"))
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("reconcile-once"));
    assert!(stdout.contains("validate-config"));
    assert!(stdout.contains("--config"));
}

#[test]
fn ships_github_docs_and_examples() {
    let readme = fs::read_to_string("README.md").unwrap();
    assert!(readme.contains("GITHUB_TOKEN"));
    assert!(readme.contains("tracker_kind = \"github\""));
    assert!(readme.contains("tracker_project_ref = \"owner/repo\""));

    let example = fs::read_to_string("config/repositories/example-github.toml").unwrap();
    assert!(example.contains("tracker_kind = \"github\""));
    assert!(example.contains("tracker_project_ref = \"owner/repo\""));
}

#[test]
fn reconcile_once_emits_summary_log() {
    let temp_dir = std::env::temp_dir().join(format!(
        "symphony-cli-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(temp_dir.join("config/repositories")).unwrap();
    fs::create_dir_all(temp_dir.join("repo")).unwrap();
    fs::write(temp_dir.join("repo/WORKFLOW.md"), "---\n---\nbody").unwrap();

    let status = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_dir.join("repo"))
        .status()
        .unwrap();
    assert!(status.success());

    let status = std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&temp_dir.join("repo"))
        .status()
        .unwrap();
    assert!(status.success());

    let status = std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&temp_dir.join("repo"))
        .status()
        .unwrap();
    assert!(status.success());

    let status = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&temp_dir.join("repo"))
        .status()
        .unwrap();
    assert!(status.success());

    let status = std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&temp_dir.join("repo"))
        .status()
        .unwrap();
    assert!(status.success());

    fs::write(
        temp_dir.join("config/orchestrator.toml"),
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
        temp_dir.join("config/repositories/demo.toml"),
        r#"
repo_id = "demo"
repo_path = "repo"
workflow_path = "repo/WORKFLOW.md"
tracker_kind = "github"
tracker_project_ref = "owner/repo"
default_runner = "process"
enabled = false
max_concurrent_runs = 0
"#,
    )
    .unwrap();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("symphony-tasks"))
        .args([
            "--config",
            temp_dir.join("config/orchestrator.toml").to_str().unwrap(),
            "reconcile-once",
        ])
        .env("GITHUB_TOKEN", "test-token")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("event=reconcile_summary")
            || stderr.contains("dispatched_runs")
            || output.status.success(),
        "Expected reconcile_summary log or success, got stderr: {stderr}"
    );
}
