use std::fs;
use std::process::Command;

use symphony_tasks::workspace::{WorkspaceManager, WorkspaceRequest};

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-workspace-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn init_source_repo(path: &std::path::Path) {
    fs::create_dir_all(path).unwrap();
    fs::write(path.join("pyproject.toml"), "[project]\nname = \"demo\"\n").unwrap();
    fs::write(path.join("README.md"), "demo\n").unwrap();

    let status = Command::new("git")
        .args(["init"])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new("git")
        .args(["config", "user.name", "Symphony Tests"])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new("git")
        .args(["config", "user.email", "tests@example.com"])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new("git")
        .args(["remote", "add", "origin", "git@github.com:acme/demo.git"])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn sanitizes_issue_key_for_workspace_paths() {
    let root = unique_temp_dir("sanitize");
    let manager = WorkspaceManager::new(root.join("var/workspaces"));

    let key = manager.workspace_key("demo/issue #42: fix?");

    assert_eq!(key, "demo-issue-42-fix");
}

#[test]
fn creates_workspace_path_from_repo_and_issue() {
    let root = unique_temp_dir("path");
    let manager = WorkspaceManager::new(root.join("var/workspaces"));
    let source_repo_path = root.join("repo");
    init_source_repo(&source_repo_path);

    let workspace = manager
        .prepare_workspace(&WorkspaceRequest {
            repo_id: "demo".into(),
            issue_identifier: "demo#42".into(),
            source_repo_path,
            after_create: vec![],
        })
        .unwrap();

    assert!(workspace.path.exists());
    assert_eq!(workspace.path, root.join("var/workspaces/demo/demo-42"));
    assert!(workspace.created_now);
    assert!(workspace.path.join("pyproject.toml").exists());

    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&workspace.path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "git@github.com:acme/demo.git"
    );
}

#[tokio::test]
async fn runs_after_create_only_on_first_creation() {
    let root = unique_temp_dir("after-create");
    let manager = WorkspaceManager::new(root.join("var/workspaces"));
    let source_repo_path = root.join("repo");
    init_source_repo(&source_repo_path);

    let marker = root.join("after-create.txt");
    let hook = format!("printf first-run >> {}", marker.display());

    let first = manager
        .prepare_workspace(&WorkspaceRequest {
            repo_id: "demo".into(),
            issue_identifier: "demo#42".into(),
            source_repo_path: source_repo_path.clone(),
            after_create: vec![hook.clone()],
        })
        .unwrap();
    manager.run_after_create_hooks(&first).await.unwrap();

    let second = manager
        .prepare_workspace(&WorkspaceRequest {
            repo_id: "demo".into(),
            issue_identifier: "demo#42".into(),
            source_repo_path,
            after_create: vec![hook],
        })
        .unwrap();
    manager.run_after_create_hooks(&second).await.unwrap();

    let contents = fs::read_to_string(marker).unwrap();
    assert_eq!(contents, "first-run");
    assert!(!second.created_now);
}

#[tokio::test]
async fn runs_before_and_after_run_hooks() {
    let root = unique_temp_dir("run-hooks");
    let manager = WorkspaceManager::new(root.join("var/workspaces"));
    let source_repo_path = root.join("repo");
    init_source_repo(&source_repo_path);

    let workspace = manager
        .prepare_workspace(&WorkspaceRequest {
            repo_id: "demo".into(),
            issue_identifier: "demo#42".into(),
            source_repo_path,
            after_create: vec![],
        })
        .unwrap();

    let before_marker = root.join("before.txt");
    let after_marker = root.join("after.txt");
    manager
        .run_hooks(
            &workspace,
            &[format!("printf before > {}", before_marker.display())],
            &[format!("printf after > {}", after_marker.display())],
        )
        .await
        .unwrap();

    assert_eq!(fs::read_to_string(before_marker).unwrap(), "before");
    assert_eq!(fs::read_to_string(after_marker).unwrap(), "after");
}
