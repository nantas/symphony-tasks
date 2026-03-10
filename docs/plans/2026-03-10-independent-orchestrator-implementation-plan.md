# Independent Orchestrator Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust-based independent Symphony orchestrator that drives one registered local repository from GitCode issue pickup through PR creation, review waiting, auto-merge, and restart-safe recovery.

**Architecture:** Implement a single Rust binary with a shared `reconcile_once()` core used by both daemon and one-shot execution modes. Keep policy in target-repository `WORKFLOW.md`, hide GitCode specifics behind adapter traits, and persist runtime state in local files under `var/`.

**Tech Stack:** Rust, Cargo, Tokio, Reqwest, Serde, TOML, YAML frontmatter parsing, Markdown parsing, tracing, tempfile, assert_fs, wiremock or httpmock

---

### Task 1: Bootstrap Rust Workspace

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/cli/mod.rs`
- Create: `src/app/mod.rs`
- Create: `.gitignore`
- Test: `cargo test`

**Step 1: Write the minimal crate layout**

Create `Cargo.toml` with the package metadata and baseline dependencies:

```toml
[package]
name = "symphony-tasks"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "process", "signal", "time", "fs"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "json"] }

[dev-dependencies]
assert_cmd = "2"
```

**Step 2: Add the simplest executable and library**

Create `src/main.rs`:

```rust
fn main() {
    println!("symphony-tasks");
}
```

Create `src/lib.rs`:

```rust
pub mod app;
pub mod cli;
```

Create empty module files:

```rust
// src/cli/mod.rs
```

```rust
// src/app/mod.rs
```

**Step 3: Add `.gitignore`**

```gitignore
/target
/var
```

**Step 4: Run test/build verification**

Run: `cargo test`

Expected: build succeeds and test summary reports `0 failed`

**Step 5: Commit**

Run:

```bash
but status --json
but commit bootstrap-rust -c -m "chore: bootstrap rust orchestrator crate" --changes <ids> --json --status-after
```

### Task 2: Define CLI Surface and Global Config

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`
- Modify: `src/cli/mod.rs`
- Create: `src/cli/args.rs`
- Create: `src/app/config.rs`
- Create: `tests/cli_smoke.rs`

**Step 1: Write the failing CLI smoke test**

Create `tests/cli_smoke.rs`:

```rust
use assert_cmd::Command;

#[test]
fn prints_help() {
    Command::cargo_bin("symphony-tasks")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
}
```

**Step 2: Run test to verify it fails if CLI is not wired**

Run: `cargo test tests::cli_smoke -- --nocapture`

Expected: fail or not compile because CLI modules are not implemented yet

**Step 3: Implement minimal CLI**

Add Clap and app config parsing with commands:

- `daemon`
- `reconcile-once`
- `validate-config`

Example `src/cli/args.rs`:

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Cli {
    #[arg(long, default_value = "config/orchestrator.toml")]
    pub config: PathBuf,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Daemon,
    ReconcileOnce,
    ValidateConfig,
}
```

**Step 4: Run tests**

Run: `cargo test cli_smoke -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit cli-surface -c -m "feat: add cli and global config entrypoints" --changes <ids> --json --status-after
```

### Task 3: Add Core Domain Models and State Enums

**Files:**
- Modify: `src/lib.rs`
- Create: `src/models/mod.rs`
- Create: `src/models/repository.rs`
- Create: `src/models/workflow.rs`
- Create: `src/models/issue.rs`
- Create: `src/models/run_record.rs`
- Create: `src/models/pr.rs`
- Test: `tests/models_roundtrip.rs`

**Step 1: Write failing serialization tests**

Create `tests/models_roundtrip.rs` with JSON roundtrip coverage for:

- `RepositoryProfile`
- `NormalizedIssue`
- `RunRecord`
- `PullRequestRef`

**Step 2: Run the tests**

Run: `cargo test models_roundtrip -- --nocapture`

Expected: fail because the models do not exist

**Step 3: Implement the models**

Use `serde::{Serialize, Deserialize}` and explicit enums for:

- `RunStatus`
- `IssueLifecycleState`
- `ReviewStatus`
- `MergeStatus`

Keep fields aligned with the design doc and avoid optional fields unless they are truly absent in V1.

**Step 4: Run the tests again**

Run: `cargo test models_roundtrip -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit core-models -c -m "feat: add orchestrator domain models" --changes <ids> --json --status-after
```

### Task 4: Parse Repository Config Files

**Files:**
- Modify: `src/app/config.rs`
- Create: `src/registry/mod.rs`
- Create: `src/registry/load.rs`
- Create: `tests/registry_loading.rs`
- Create: `config/orchestrator.toml`
- Create: `config/repositories/example.toml`

**Step 1: Write failing registry loading tests**

Cover:

- loading global config from `config/orchestrator.toml`
- loading one enabled repository profile
- rejecting duplicate `repo_id`
- rejecting missing `repo_path`

**Step 2: Run tests**

Run: `cargo test registry_loading -- --nocapture`

Expected: fail

**Step 3: Implement config and registry loading**

Use `std::fs` plus Serde TOML parsing. Keep path normalization inside `src/app/config.rs` so downstream modules receive absolute paths.

**Step 4: Re-run tests**

Run: `cargo test registry_loading -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit registry-config -c -m "feat: load orchestrator and repository config" --changes <ids> --json --status-after
```

### Task 5: Parse `WORKFLOW.md`

**Files:**
- Modify: `Cargo.toml`
- Create: `src/workflow/mod.rs`
- Create: `src/workflow/parser.rs`
- Create: `tests/workflow_parser.rs`

**Step 1: Write failing workflow parser tests**

Cover:

- frontmatter plus markdown body parsing
- extracting active and terminal states
- extracting hook commands
- rejecting malformed YAML

**Step 2: Run tests**

Run: `cargo test workflow_parser -- --nocapture`

Expected: fail

**Step 3: Implement the parser**

Add dependencies as needed, for example:

```toml
serde_yaml = "0.9"
gray_matter = "0.2"
```

Return a typed `WorkflowDefinition` and preserve the markdown body as `prompt_template`.

**Step 4: Re-run tests**

Run: `cargo test workflow_parser -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit workflow-loader -c -m "feat: parse workflow policy from markdown" --changes <ids> --json --status-after
```

### Task 6: Implement File-Backed State Store

**Files:**
- Create: `src/state_store/mod.rs`
- Create: `src/state_store/layout.rs`
- Create: `src/state_store/files.rs`
- Create: `tests/state_store.rs`

**Step 1: Write failing state store tests**

Cover:

- saving and loading `RunRecord`
- saving and loading retry queue
- saving and loading PR watch state
- per-issue file layout under `var/runs/<repo-id>/<issue-id>.json`

**Step 2: Run tests**

Run: `cargo test state_store -- --nocapture`

Expected: fail

**Step 3: Implement minimal state store**

Use atomic write patterns where practical:

- write to temp file
- rename into place

Prefer line-oriented JSON only if it simplifies debugging; otherwise use pretty JSON documents.

**Step 4: Re-run tests**

Run: `cargo test state_store -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit state-store -c -m "feat: add file-backed runtime state store" --changes <ids> --json --status-after
```

### Task 7: Implement Workspace Management

**Files:**
- Create: `src/workspace/mod.rs`
- Create: `src/workspace/keys.rs`
- Create: `src/workspace/hooks.rs`
- Create: `tests/workspace_manager.rs`

**Step 1: Write failing workspace tests**

Cover:

- issue key sanitization
- workspace path creation
- `after_create` only runs on first creation
- `before_run` and `after_run` invocation

**Step 2: Run tests**

Run: `cargo test workspace_manager -- --nocapture`

Expected: fail

**Step 3: Implement workspace manager**

Use `tokio::process::Command` for hook execution and return structured hook failures with captured stderr snippets.

**Step 4: Re-run tests**

Run: `cargo test workspace_manager -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit workspace-manager -c -m "feat: add isolated workspace management" --changes <ids> --json --status-after
```

### Task 8: Define Tracker Traits and GitCode Models

**Files:**
- Modify: `src/lib.rs`
- Create: `src/tracker/mod.rs`
- Create: `src/tracker/types.rs`
- Create: `src/tracker/gitcode/mod.rs`
- Create: `src/tracker/gitcode/models.rs`
- Create: `tests/tracker_mapping.rs`

**Step 1: Write failing tracker mapping tests**

Cover GitCode payload normalization into:

- `NormalizedIssue`
- `PullRequestRef`

Include cases where `issue_state` is absent and top-level `state` must be used.

**Step 2: Run tests**

Run: `cargo test tracker_mapping -- --nocapture`

Expected: fail

**Step 3: Implement tracker traits and raw models**

Define traits such as:

```rust
#[async_trait::async_trait]
pub trait Tracker {
    async fn fetch_candidate_issues(&self, repo: &RepositoryProfile) -> anyhow::Result<Vec<NormalizedIssue>>;
    async fn fetch_issue(&self, repo: &RepositoryProfile, issue_id: &str) -> anyhow::Result<NormalizedIssue>;
    async fn update_issue_state(&self, repo: &RepositoryProfile, issue_id: &str, state: &str) -> anyhow::Result<()>;
}
```

Add more methods for comments, PR creation, PR queries, and merge before leaving the task.

**Step 4: Re-run tests**

Run: `cargo test tracker_mapping -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit tracker-traits -c -m "feat: add tracker traits and gitcode mapping" --changes <ids> --json --status-after
```

### Task 9: Implement GitCode HTTP Client

**Files:**
- Modify: `Cargo.toml`
- Create: `src/tracker/gitcode/client.rs`
- Modify: `src/tracker/gitcode/mod.rs`
- Create: `tests/gitcode_client.rs`

**Step 1: Write failing client tests**

Use `wiremock` or `httpmock` to cover:

- fetching candidate issues
- fetching a single issue
- updating issue state
- creating a comment
- querying PR review status
- merging a PR

**Step 2: Run tests**

Run: `cargo test gitcode_client -- --nocapture`

Expected: fail

**Step 3: Implement minimal GitCode REST client**

Add dependencies:

```toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
async-trait = "0.1"
```

Normalize auth and headers in one place. Return domain errors instead of leaking raw `reqwest` errors to the orchestrator.

**Step 4: Re-run tests**

Run: `cargo test gitcode_client -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit gitcode-client -c -m "feat: implement gitcode tracker adapter" --changes <ids> --json --status-after
```

### Task 10: Add Runner Trait and Stub Runner Implementation

**Files:**
- Create: `src/agent_runner/mod.rs`
- Create: `src/agent_runner/types.rs`
- Create: `src/agent_runner/process.rs`
- Create: `tests/runner_process.rs`

**Step 1: Write failing runner tests**

Cover:

- prompt rendering receives issue and workflow data
- runner executes a configured command in the workspace
- non-zero exit is surfaced as structured failure

**Step 2: Run tests**

Run: `cargo test runner_process -- --nocapture`

Expected: fail

**Step 3: Implement generic runner trait**

Use a small process-backed implementation that shells out to a configured executable and parses a JSON result from stdout.

**Step 4: Re-run tests**

Run: `cargo test runner_process -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit runner-trait -c -m "feat: add process-backed agent runner" --changes <ids> --json --status-after
```

### Task 11: Build Orchestrator Selection and Retry Logic

**Files:**
- Create: `src/orchestrator/mod.rs`
- Create: `src/orchestrator/reconcile.rs`
- Create: `src/orchestrator/retry.rs`
- Create: `tests/orchestrator_selection.rs`

**Step 1: Write failing orchestrator tests**

Cover:

- repository and global concurrency limits
- retry backoff exclusion
- skip already-claimed issues
- issue state eligibility

**Step 2: Run tests**

Run: `cargo test orchestrator_selection -- --nocapture`

Expected: fail

**Step 3: Implement candidate selection and retry logic**

Keep `reconcile_once()` thin at first:

- read candidate issues
- filter by eligibility
- return a dispatch plan

Do not run the full runner yet.

**Step 4: Re-run tests**

Run: `cargo test orchestrator_selection -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit orchestrator-core -c -m "feat: add orchestrator selection and retry logic" --changes <ids> --json --status-after
```

### Task 12: Wire End-to-End Dispatch Through Workspace and Runner

**Files:**
- Modify: `src/orchestrator/reconcile.rs`
- Modify: `src/workspace/mod.rs`
- Modify: `src/agent_runner/mod.rs`
- Create: `tests/reconcile_dispatch.rs`

**Step 1: Write failing dispatch integration tests**

Use fake tracker and fake runner implementations to cover:

- claim issue
- move issue to `In Progress`
- create workspace
- execute runner
- persist `RunRecord`

**Step 2: Run tests**

Run: `cargo test reconcile_dispatch -- --nocapture`

Expected: fail

**Step 3: Implement dispatch orchestration**

Translate one selected issue into:

- issue state update
- workspace preparation
- runner execution
- run record persistence

**Step 4: Re-run tests**

Run: `cargo test reconcile_dispatch -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit dispatch-flow -c -m "feat: wire dispatch through workspace and runner" --changes <ids> --json --status-after
```

### Task 13: Add PR Creation, Review Waiting, and Auto-Merge

**Files:**
- Modify: `src/orchestrator/reconcile.rs`
- Modify: `src/tracker/mod.rs`
- Modify: `src/tracker/gitcode/client.rs`
- Create: `tests/pr_reconcile.rs`

**Step 1: Write failing PR lifecycle tests**

Cover:

- create or update PR after successful run
- move issue to `Human Review`
- detect approved PR
- merge approved PR
- move issue to `Done`

**Step 2: Run tests**

Run: `cargo test pr_reconcile -- --nocapture`

Expected: fail

**Step 3: Implement PR lifecycle logic**

Persist PR watch state separately from run records so the reconcile loop can restart cleanly without rerunning the agent.

**Step 4: Re-run tests**

Run: `cargo test pr_reconcile -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit pr-lifecycle -c -m "feat: add pr review tracking and auto-merge" --changes <ids> --json --status-after
```

### Task 14: Implement Restart Recovery and Single-Instance Locking

**Files:**
- Modify: `src/app/mod.rs`
- Modify: `src/state_store/mod.rs`
- Create: `src/app/lock.rs`
- Create: `tests/restart_recovery.rs`

**Step 1: Write failing recovery tests**

Cover:

- rebuilding retry queue after restart
- recovering PR watch tasks after restart
- detecting interrupted runs without active process
- refusing a second daemon instance when the lock is held

**Step 2: Run tests**

Run: `cargo test restart_recovery -- --nocapture`

Expected: fail

**Step 3: Implement recovery and locking**

Use an OS-visible lock file or advisory lock that works on Linux and macOS. Keep the lock acquisition in `app`, not in `orchestrator`.

**Step 4: Re-run tests**

Run: `cargo test restart_recovery -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit restart-recovery -c -m "feat: add restart recovery and daemon locking" --changes <ids> --json --status-after
```

### Task 15: Add Logging, Config Validation, and Operator Diagnostics

**Files:**
- Create: `src/logging/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/app/config.rs`
- Create: `tests/validate_config.rs`

**Step 1: Write failing validation tests**

Cover:

- invalid repository path
- missing workflow file
- invalid concurrency value
- missing GitCode token environment binding

**Step 2: Run tests**

Run: `cargo test validate_config -- --nocapture`

Expected: fail

**Step 3: Implement diagnostics**

Expose clear validation errors and structured logs using `tracing` with JSON output in daemon mode.

**Step 4: Re-run tests**

Run: `cargo test validate_config -- --nocapture`

Expected: pass

**Step 5: Commit**

Run:

```bash
but status --json
but commit diagnostics -c -m "feat: add config validation and structured logging" --changes <ids> --json --status-after
```

### Task 16: Add End-to-End Fixture and Usage Documentation

**Files:**
- Create: `README.md`
- Create: `config/repositories/example-gitcode.toml`
- Create: `tests/e2e_fixture.rs`
- Modify: `docs/plans/2026-03-10-independent-orchestrator-design.md`

**Step 1: Write failing e2e fixture test**

Cover one fake repository and one fake GitCode issue that reaches:

- dispatch
- PR waiting
- approval
- merge
- completion

**Step 2: Run tests**

Run: `cargo test e2e_fixture -- --nocapture`

Expected: fail

**Step 3: Implement fixture and docs**

Document:

- required environment variables
- config layout
- daemon command
- one-shot command
- local development workflow

**Step 4: Re-run tests**

Run: `cargo test e2e_fixture -- --nocapture`

Expected: pass

**Step 5: Final verification and commit**

Run:

```bash
cargo test
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
but status --json
but commit docs-and-fixture -c -m "docs: add usage guide and end-to-end fixture" --changes <ids> --json --status-after
```

## Execution Notes

- Use `@superpowers/test-driven-development` before implementing each task.
- Use `@superpowers/systematic-debugging` immediately when a test or command fails unexpectedly.
- Use `@superpowers/verification-before-completion` before every completion claim or commit.
- Keep each commit scoped to one task.
- Do not implement clone support, multiple runners, or multi-repo active scheduling in this plan.
