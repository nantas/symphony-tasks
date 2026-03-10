use std::fs;

use symphony_tasks::workflow::parser::load_workflow_definition;

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-workflow-{name}-{}-{}",
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
fn parses_frontmatter_and_prompt_body() {
    let root = unique_temp_dir("valid");
    let workflow_path = root.join("WORKFLOW.md");

    fs::write(
        &workflow_path,
        r#"---
active_states:
  - Todo
  - In Progress
terminal_states:
  - Done
  - Failed
retry_policy:
  max_attempts: 3
  backoff_seconds: 60
hooks:
  after_create:
    - setup
  before_run:
    - lint
  after_run:
    - report
  before_remove:
    - cleanup
pr_policy:
  require_pr: true
completion_policy:
  close_issue_on_merge: true
---
# System Prompt

Implement the requested issue safely.
"#,
    )
    .unwrap();

    let workflow = load_workflow_definition(&workflow_path).unwrap();

    assert_eq!(workflow.active_states, vec!["Todo", "In Progress"]);
    assert_eq!(workflow.terminal_states, vec!["Done", "Failed"]);
    assert_eq!(workflow.retry_policy.max_attempts, 3);
    assert_eq!(workflow.hooks.after_create, vec!["setup"]);
    assert_eq!(workflow.hooks.before_run, vec!["lint"]);
    assert!(
        workflow
            .prompt_template
            .contains("Implement the requested issue safely.")
    );
}

#[test]
fn rejects_malformed_yaml() {
    let root = unique_temp_dir("invalid");
    let workflow_path = root.join("WORKFLOW.md");

    fs::write(
        &workflow_path,
        r#"---
active_states:
  - Todo
hooks: [broken
---
body
"#,
    )
    .unwrap();

    let error = load_workflow_definition(&workflow_path)
        .unwrap_err()
        .to_string();

    assert!(error.contains("workflow frontmatter"));
}
