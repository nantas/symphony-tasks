use crate::models::workflow::{
    CompletionPolicy, PrPolicy, RetryPolicy, WorkflowDefinition, WorkflowHooks,
};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct WorkflowFrontmatter {
    #[serde(default)]
    active_states: Vec<String>,
    #[serde(default)]
    terminal_states: Vec<String>,
    #[serde(default)]
    state_mapping: BTreeMap<String, String>,
    #[serde(default)]
    hooks: WorkflowHooks,
    #[serde(default)]
    retry_policy: RetryPolicy,
    #[serde(default)]
    pr_policy: PrPolicy,
    #[serde(default)]
    completion_policy: CompletionPolicy,
}

pub fn load_workflow_definition(path: impl AsRef<Path>) -> Result<WorkflowDefinition> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read workflow file {}", path.display()))?;
    let (frontmatter, body) = split_frontmatter(&contents)?;
    let parsed: WorkflowFrontmatter = serde_yaml::from_str(frontmatter)
        .context("failed to parse workflow frontmatter")?;

    Ok(WorkflowDefinition {
        active_states: parsed.active_states,
        terminal_states: parsed.terminal_states,
        prompt_template: body.trim().to_string(),
        state_mapping: parsed.state_mapping,
        hooks: parsed.hooks,
        retry_policy: parsed.retry_policy,
        pr_policy: parsed.pr_policy,
        completion_policy: parsed.completion_policy,
    })
}

fn split_frontmatter(contents: &str) -> Result<(&str, &str)> {
    let rest = contents
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow::anyhow!("workflow frontmatter must start with ---"))?;
    let Some((frontmatter, body)) = rest.split_once("\n---\n") else {
        bail!("workflow frontmatter must end with ---");
    };

    Ok((frontmatter, body))
}
