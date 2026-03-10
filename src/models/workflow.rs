use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub config: serde_json::Value,
    pub prompt_template: String,
    pub state_mapping: serde_json::Value,
    pub hooks: WorkflowHooks,
    pub retry_policy: RetryPolicy,
    pub pr_policy: serde_json::Value,
    pub completion_policy: serde_json::Value,
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
