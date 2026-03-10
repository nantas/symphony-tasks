use std::fs;

use symphony_tasks::agent_runner::process::{ProcessRunner, ProcessRunnerConfig};
use symphony_tasks::agent_runner::types::{AgentRunStatus, RunnerError};
use symphony_tasks::agent_runner::AgentRunner;
use symphony_tasks::models::issue::NormalizedIssue;
use symphony_tasks::models::workflow::{
    CompletionPolicy, PrPolicy, RetryPolicy, WorkflowDefinition, WorkflowHooks,
};

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "symphony-runner-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn test_issue() -> NormalizedIssue {
    NormalizedIssue {
        id: "100".into(),
        identifier: "demo#42".into(),
        repo_id: "demo".into(),
        title: "Implement orchestrator".into(),
        description: Some("Build the first slice".into()),
        state: "Todo".into(),
        priority: Some(1),
        labels: vec!["backend".into()],
        url: None,
        created_at: None,
        updated_at: None,
    }
}

fn test_workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        active_states: vec!["Todo".into()],
        terminal_states: vec!["Done".into()],
        prompt_template: "Title: {{issue_title}}\nDescription: {{issue_description}}".into(),
        state_mapping: Default::default(),
        hooks: WorkflowHooks::default(),
        retry_policy: RetryPolicy {
            max_attempts: 1,
            backoff_seconds: 60,
        },
        pr_policy: PrPolicy { require_pr: true },
        completion_policy: CompletionPolicy {
            close_issue_on_merge: true,
        },
    }
}

#[tokio::test]
async fn renders_prompt_with_issue_and_workflow_data() {
    let workspace = unique_temp_dir("prompt-render");
    let runner = ProcessRunner::new(ProcessRunnerConfig {
        program: "/bin/sh".into(),
        args: vec![
            "-lc".into(),
            "printf '%s' \"$PROMPT\" > prompt.txt; printf '{\"status\":\"success\",\"summary\":\"ok\"}'"
                .into(),
        ],
    });

    let result = runner
        .run(&workspace, &test_issue(), &test_workflow())
        .await
        .unwrap();

    assert_eq!(result.status, AgentRunStatus::Success);
    let prompt = fs::read_to_string(workspace.join("prompt.txt")).unwrap();
    assert!(prompt.contains("Implement orchestrator"));
    assert!(prompt.contains("Build the first slice"));
}

#[tokio::test]
async fn executes_runner_command_in_workspace() {
    let workspace = unique_temp_dir("workspace-exec");
    let runner = ProcessRunner::new(ProcessRunnerConfig {
        program: "/bin/sh".into(),
        args: vec![
            "-lc".into(),
            "printf workspace > marker.txt; printf '{\"status\":\"success\",\"summary\":\"ok\",\"branch_name\":\"feat/demo-42\"}'"
                .into(),
        ],
    });

    let result = runner
        .run(&workspace, &test_issue(), &test_workflow())
        .await
        .unwrap();

    assert_eq!(fs::read_to_string(workspace.join("marker.txt")).unwrap(), "workspace");
    assert_eq!(result.branch_name.as_deref(), Some("feat/demo-42"));
}

#[tokio::test]
async fn surfaces_non_zero_exit_as_structured_failure() {
    let workspace = unique_temp_dir("runner-failure");
    let runner = ProcessRunner::new(ProcessRunnerConfig {
        program: "/bin/sh".into(),
        args: vec!["-lc".into(), "echo broken 1>&2; exit 7".into()],
    });

    let error = runner
        .run(&workspace, &test_issue(), &test_workflow())
        .await
        .unwrap_err();

    match error {
        RunnerError::ProcessFailed { exit_code, stderr } => {
            assert_eq!(exit_code, Some(7));
            assert!(stderr.contains("broken"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
