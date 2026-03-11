use std::fs;

use symphony_tasks::app::config::{OrchestratorConfig, validate_loaded_config_with};

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-validate-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_base_config(
    root: &std::path::Path,
    global_concurrency: usize,
    repo_path: &str,
    workflow_path: &str,
) {
    fs::create_dir_all(root.join("config/repositories")).unwrap();
    fs::write(
        root.join("config/orchestrator.toml"),
        format!(
            r#"
poll_interval_secs = 30
global_concurrency = {global_concurrency}
log_level = "info"
state_root = "var/state"
workspace_root = "var/workspaces"
lock_path = "var/locks/daemon.lock"
default_tracker_kind = "github"
github_token_env = "GITHUB_TOKEN"
default_runner = "process"
runner_program = "/bin/sh"
runner_args = ["-lc", "printf '{{\"status\":\"success\",\"summary\":\"ok\"}}'"]
repositories_dir = "config/repositories"
"#
        ),
    )
    .unwrap();
    fs::write(
        root.join("config/repositories/demo.toml"),
        format!(
            r#"
repo_id = "demo"
repo_path = "{repo_path}"
workflow_path = "{workflow_path}"
tracker_kind = "github"
tracker_project_ref = "acme/demo"
default_runner = "process"
enabled = true
max_concurrent_runs = 1
"#
        ),
    )
    .unwrap();
}

#[test]
fn rejects_invalid_repository_path() {
    let root = unique_temp_dir("bad-repo");
    write_base_config(&root, 1, "missing-repo", "WORKFLOW.md");
    let config = OrchestratorConfig::load_from_file(root.join("config/orchestrator.toml")).unwrap();

    let error = validate_loaded_config_with(&config, |_| Some("token".into()))
        .unwrap_err()
        .to_string();

    assert!(error.contains("repo_path"));
}

#[test]
fn rejects_missing_workflow_file() {
    let root = unique_temp_dir("missing-workflow");
    fs::create_dir_all(root.join("repo")).unwrap();
    write_base_config(&root, 1, "repo", "repo/WORKFLOW.md");
    let config = OrchestratorConfig::load_from_file(root.join("config/orchestrator.toml")).unwrap();

    let error = validate_loaded_config_with(&config, |_| Some("token".into()))
        .unwrap_err()
        .to_string();

    assert!(error.contains("workflow_path"));
}

#[test]
fn rejects_invalid_concurrency_value() {
    let root = unique_temp_dir("bad-concurrency");
    fs::create_dir_all(root.join("repo")).unwrap();
    fs::write(root.join("repo/WORKFLOW.md"), "---\n---\nbody").unwrap();
    write_base_config(&root, 0, "repo", "repo/WORKFLOW.md");

    let error = OrchestratorConfig::load_from_file(root.join("config/orchestrator.toml"))
        .unwrap_err()
        .to_string();

    assert!(error.contains("global_concurrency"));
}

#[test]
fn rejects_missing_github_token_environment_binding() {
    let root = unique_temp_dir("missing-token");
    fs::create_dir_all(root.join("repo")).unwrap();
    fs::write(root.join("repo/WORKFLOW.md"), "---\n---\nbody").unwrap();
    write_base_config(&root, 1, "repo", "repo/WORKFLOW.md");
    let config = OrchestratorConfig::load_from_file(root.join("config/orchestrator.toml")).unwrap();

    let error = validate_loaded_config_with(&config, |_| None)
        .unwrap_err()
        .to_string();

    assert!(error.contains("GITHUB_TOKEN"));
}
