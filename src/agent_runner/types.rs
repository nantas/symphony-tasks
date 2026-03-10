use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunResult {
    pub status: AgentRunStatus,
    pub summary: String,
    pub branch_name: Option<String>,
    pub commit_sha: Option<String>,
    pub requested_next_action: Option<String>,
}

#[derive(Debug)]
pub enum RunnerError {
    ProcessFailed {
        exit_code: Option<i32>,
        stderr: String,
    },
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProcessFailed { exit_code, stderr } => {
                write!(
                    f,
                    "process failed with exit code {:?}: {}",
                    exit_code, stderr
                )
            }
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Parse(error) => write!(f, "parse error: {error}"),
        }
    }
}

impl std::error::Error for RunnerError {}

impl From<std::io::Error> for RunnerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for RunnerError {
    fn from(value: serde_json::Error) -> Self {
        Self::Parse(value)
    }
}
