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
