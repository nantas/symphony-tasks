use assert_cmd::Command;

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
