# AGENTS.md

Guidelines for AI coding agents working on symphony-tasks.

## Project Overview

Symphony Tasks is a Rust single-binary orchestrator for GitHub Issues and PR workflows. It manages issue-to-workspace lifecycle, PR creation, and state tracking via a local file-based runtime.

## Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo run -- --config config/orchestrator.toml validate-config
cargo run -- --config config/orchestrator.toml reconcile-once
cargo run -- --config config/orchestrator.toml daemon
```

## Test Commands

```bash
cargo test                                       # Run all tests
cargo test --test orchestrator_selection         # Run single test file
cargo test respects_repository_and_global_concurrency_limits   # Run single test by name
cargo test -- --nocapture                        # Show test output
cargo test -- --test-threads=1                   # Run tests sequentially
```

## Lint and Format

```bash
cargo clippy -- -D warnings   # Linter, treat warnings as errors
cargo fmt -- --check          # Check formatting
```

## Code Style

### Comments

**Do not add comments.** Code should be self-documenting through clear naming and structure.

### Imports

Group imports with blank lines between: std, external crates, internal modules.

```rust
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::issue::NormalizedIssue;
use crate::tracker::Tracker;
```

### Naming Conventions

- **Types**: PascalCase (`WorkflowDefinition`, `SelectionContext`)
- **Functions/methods/variables**: snake_case (`select_dispatch_candidates`, `run_record`)
- **Constants**: SCREAMING_SNAKE_CASE (`API_VERSION`)
- **Module/file names**: snake_case (`state_store`, `github_client.rs`)

### Types and Structs

- Derive: `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, `Deserialize` as appropriate
- Use `#[serde(default)]` for optional struct fields
- Implement `Default` trait explicitly when needed

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_seconds: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 1, backoff_seconds: 60 }
    }
}
```

### Error Handling

- Use `anyhow::Result<T>` for fallible functions
- Use `anyhow::Context` to add context to errors
- Use `anyhow::bail!` for early returns with errors

```rust
pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let raw: RawConfig = toml::from_str(&contents).context("failed to parse config")?;
    Ok(raw.into())
}
```

### Async Patterns

- Use `#[async_trait]` for async traits
- Use `tokio` as the async runtime

```rust
#[async_trait]
pub trait Tracker: Send + Sync {
    async fn fetch_candidate_issues(&self, repo: &RepositoryProfile) -> Result<Vec<NormalizedIssue>>;
}
```

### Struct Updates

Use struct update syntax:

```rust
let updated = RunRecord {
    pr_ref: Some(pr.id.clone()),
    status: RunStatus::AwaitingHumanReview,
    updated_at: request.updated_at.to_string(),
    ..request.run_record
};
```

### Testing

- Integration tests in `tests/`, unit tests in `#[cfg(test)]` modules
- Use helper functions for test fixtures
- Use `wiremock` for HTTP mocking
- Use `#[tokio::test]` for async tests

```rust
fn repo_profile() -> RepositoryProfile {
    RepositoryProfile {
        repo_id: "demo".into(),
        repo_path: "/tmp/demo".into(),
        workflow_path: "/tmp/demo/WORKFLOW.md".into(),
        tracker_kind: "github".into(),
        tracker_project_ref: "acme/demo".into(),
        default_runner: "process".into(),
        enabled: true,
        max_concurrent_runs: 2,
    }
}

#[test]
fn respects_repository_and_global_concurrency_limits() {
    let selected = select_dispatch_candidates(&candidates, &repo_profile(), &workflow(), &context);
    assert_eq!(selected.len(), 1);
}
```

### Path Handling

- Use `std::path::{Path, PathBuf}` for file paths
- Accept `impl AsRef<Path>` for flexibility

## Project Structure

```
src/
  main.rs              # CLI entrypoint
  lib.rs               # Library root
  app/                 # Application runtime and config
  agent_runner/        # Process-based agent execution
  models/              # Domain types (issue, pr, workflow, run_record)
  orchestrator/        # Dispatch and reconciliation logic
  registry/            # Repository registration loading
  state_store/         # File-based state persistence
  tracker/             # GitHub/GitCode API clients
  workflow/            # WORKFLOW.md parsing
  workspace/           # Workspace management and hooks
tests/                 # Integration tests
```

## Key Dependencies

`anyhow` `async-trait` `clap` `reqwest` `serde` `serde_json` `serde_yaml` `toml` `tokio` `tracing`

Dev: `wiremock` `assert_cmd`

## Pre-commit Checks

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```
