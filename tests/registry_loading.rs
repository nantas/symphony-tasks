use std::fs;

use symphony_tasks::app::config::OrchestratorConfig;
use symphony_tasks::registry::load::load_repository_profiles;

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-tasks-{name}-{}-{}",
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
fn loads_global_config_from_repo_file() {
    let path = std::env::current_dir()
        .unwrap()
        .join("config/orchestrator.toml");

    let config = OrchestratorConfig::load_from_file(&path).unwrap();

    assert_eq!(config.poll_interval_secs, 30);
    assert_eq!(config.global_concurrency, 2);
    assert_eq!(config.default_tracker_kind, "github");
    assert_eq!(config.github_token_env, "GITHUB_TOKEN");
    assert_eq!(config.default_runner, "process");
    assert!(config.state_root.is_absolute());
    assert!(config.workspace_root.is_absolute());
    assert!(config.repositories_dir.is_absolute());
}

#[test]
fn loads_one_enabled_repository_profile() {
    let config = OrchestratorConfig::load_from_file("config/orchestrator.toml").unwrap();

    let profiles = load_repository_profiles(&config).unwrap();

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].repo_id, "game-design-patterns");
    assert_eq!(profiles[0].tracker_kind, "github");
    assert_eq!(
        profiles[0].tracker_project_ref,
        "nantas/game-design-database"
    );
    assert!(profiles[0].repo_path.is_absolute());
    assert!(profiles[0].workflow_path.is_absolute());
}

#[test]
fn rejects_duplicate_repo_ids() {
    let root = unique_temp_dir("duplicate-repo-ids");
    let repositories_dir = root.join("config/repositories");
    fs::create_dir_all(&repositories_dir).unwrap();

    fs::write(
        root.join("config/orchestrator.toml"),
        r#"
poll_interval_secs = 15
global_concurrency = 1
log_level = "info"
state_root = "var/state"
workspace_root = "var/workspaces"
lock_path = "var/locks/daemon.lock"
default_tracker_kind = "github"
github_token_env = "GITHUB_TOKEN"
default_runner = "process"
repositories_dir = "config/repositories"
"#,
    )
    .unwrap();

    fs::write(
        repositories_dir.join("one.toml"),
        r#"
repo_id = "dup"
repo_path = "."
workflow_path = "WORKFLOW.md"
tracker_kind = "github"
tracker_project_ref = "demo/one"
default_runner = "process"
enabled = true
max_concurrent_runs = 1
"#,
    )
    .unwrap();

    fs::write(
        repositories_dir.join("two.toml"),
        r#"
repo_id = "dup"
repo_path = "."
workflow_path = "WORKFLOW.md"
tracker_kind = "github"
tracker_project_ref = "demo/two"
default_runner = "process"
enabled = true
max_concurrent_runs = 1
"#,
    )
    .unwrap();

    let config = OrchestratorConfig::load_from_file(root.join("config/orchestrator.toml")).unwrap();
    let error = load_repository_profiles(&config).unwrap_err().to_string();

    assert!(error.contains("duplicate repo_id"));
}

#[test]
fn rejects_missing_repo_path() {
    let root = unique_temp_dir("missing-repo-path");
    let repositories_dir = root.join("config/repositories");
    fs::create_dir_all(&repositories_dir).unwrap();

    fs::write(
        root.join("config/orchestrator.toml"),
        r#"
poll_interval_secs = 15
global_concurrency = 1
log_level = "info"
state_root = "var/state"
workspace_root = "var/workspaces"
lock_path = "var/locks/daemon.lock"
default_tracker_kind = "github"
github_token_env = "GITHUB_TOKEN"
default_runner = "process"
repositories_dir = "config/repositories"
"#,
    )
    .unwrap();

    fs::write(
        repositories_dir.join("missing.toml"),
        r#"
repo_id = "missing"
repo_path = "repos/does-not-exist"
workflow_path = "WORKFLOW.md"
tracker_kind = "github"
tracker_project_ref = "demo/missing"
default_runner = "process"
enabled = true
max_concurrent_runs = 1
"#,
    )
    .unwrap();

    let config = OrchestratorConfig::load_from_file(root.join("config/orchestrator.toml")).unwrap();
    let error = load_repository_profiles(&config).unwrap_err().to_string();

    assert!(error.contains("repo_path"));
}
