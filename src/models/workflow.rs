use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub active_states: Vec<String>,
    pub terminal_states: Vec<String>,
    pub prompt_template: String,
    #[serde(default)]
    pub state_mapping: BTreeMap<String, String>,
    pub hooks: WorkflowHooks,
    pub retry_policy: RetryPolicy,
    pub pr_policy: PrPolicy,
    pub completion_policy: CompletionPolicy,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowHooks {
    pub after_create: Vec<String>,
    pub before_run: Vec<String>,
    pub after_run: Vec<String>,
    pub before_remove: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_seconds: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            backoff_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrPolicy {
    pub require_pr: bool,
}

impl Default for PrPolicy {
    fn default() -> Self {
        Self { require_pr: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionPolicy {
    pub close_issue_on_merge: bool,
}

impl Default for CompletionPolicy {
    fn default() -> Self {
        Self {
            close_issue_on_merge: true,
        }
    }
}
