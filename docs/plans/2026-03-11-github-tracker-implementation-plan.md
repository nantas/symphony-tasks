# GitHub Tracker Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the live GitCode integration path with a GitHub Issues + GitHub PR adapter driven by issue labels, while preserving the existing orchestrator core and enabling pilot integration for `game-design-patterns`.

**Architecture:** Keep `orchestrator`, `workspace`, `state_store`, and the process-backed runner unchanged where possible. Introduce a new `tracker::github` adapter, generalize config and repository metadata away from GitCode-specific fields, and add a target-repository workflow plus codex runner wrapper for the pilot repository.

**Tech Stack:** Rust, `reqwest`, `wiremock`, `tokio`, GitHub REST API, shell runner wrapper, `codex CLI`, Python `uv`

---

### Task 1: Generalize Repository Metadata Away From GitCode

**Files:**
- Modify: `src/models/repository.rs`
- Modify: `src/registry/load.rs`
- Modify: `src/app/config.rs`
- Modify: `tests/models_roundtrip.rs`
- Modify: `tests/registry_loading.rs`
- Modify: `tests/validate_config.rs`
- Modify: `config/repositories/example.toml`
- Modify: `config/repositories/example-gitcode.toml`

**Step 1: Write the failing tests**

Update tests to expect:

- `tracker_kind`
- `tracker_project_ref`
- `default_tracker_kind`
- `github_token_env`

Example assertion shape:

```rust
assert_eq!(profile.tracker_kind, "github");
assert_eq!(profile.tracker_project_ref, "acme/example");
assert_eq!(config.default_tracker_kind, "github");
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test registry_loading -- --nocapture
cargo test --test validate_config -- --nocapture
cargo test --test models_roundtrip -- --nocapture
```

Expected:

- FAIL because the new fields do not exist yet

**Step 3: Write the minimal implementation**

Make these changes:

- replace `gitcode_project_ref` in `RepositoryProfile`
- update TOML loading in `src/registry/load.rs`
- add `default_tracker_kind` and `github_token_env` to `OrchestratorConfig`
- keep validation strict and explicit

**Step 4: Run tests to verify they pass**

Run the same commands again.

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/models/repository.rs src/registry/load.rs src/app/config.rs tests/models_roundtrip.rs tests/registry_loading.rs tests/validate_config.rs config/repositories/example.toml config/repositories/example-gitcode.toml
git commit -m "refactor: generalize tracker configuration"
```

### Task 2: Add GitHub Issue and PR Mapping Models

**Files:**
- Create: `src/tracker/github/mod.rs`
- Create: `src/tracker/github/models.rs`
- Modify: `src/tracker/mod.rs`
- Create: `tests/github_mapping.rs`

**Step 1: Write the failing tests**

Add tests covering:

- issue labels map to `Todo`
- issue with no state label falls back to `open`
- issue with multiple state labels is treated as ambiguous
- PR review and merge status map correctly

Example test shape:

```rust
assert_eq!(normalized.state, "Todo");
assert!(normalized.labels.contains(&"todo".to_string()));
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test github_mapping -- --nocapture
```

Expected:

- FAIL because `tracker::github` does not exist

**Step 3: Write minimal implementation**

Implement:

- GitHub issue structs
- label parsing
- state-label normalization
- PR normalization helpers

Use these workflow labels in V1:

- `todo`
- `in-progress`
- `human-review`
- `done`

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test github_mapping -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/tracker/github/mod.rs src/tracker/github/models.rs src/tracker/mod.rs tests/github_mapping.rs
git commit -m "feat: add github tracker models"
```

### Task 3: Add Explicit Issue Close Capability to the Tracker Trait

**Files:**
- Modify: `src/tracker/mod.rs`
- Modify: `src/tracker/gitcode/client.rs`
- Modify: `tests/pr_reconcile.rs`
- Modify: `tests/e2e_fixture.rs`
- Modify: `tests/reconcile_dispatch.rs`
- Modify: `tests/app_runtime.rs`

**Step 1: Write the failing test**

Update fake trackers to implement an explicit close operation.

Add a PR lifecycle assertion that completion uses issue close as a distinct action.

Example shape:

```rust
assert_eq!(tracker.closed_issues.lock().unwrap().as_slice(), &["100".to_string()]);
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test pr_reconcile -- --nocapture
cargo test --test e2e_fixture -- --nocapture
```

Expected:

- FAIL because the trait method does not exist

**Step 3: Write minimal implementation**

Add to `Tracker`:

```rust
async fn close_issue(&self, repo: &RepositoryProfile, issue_id: &str) -> Result<()>;
```

For the existing GitCode client:

- implement the method as a no-op compatibility path or by mapping to the platform close endpoint if available

**Step 4: Run tests to verify they pass**

Run the same commands again.

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/tracker/mod.rs src/tracker/gitcode/client.rs tests/pr_reconcile.rs tests/e2e_fixture.rs tests/reconcile_dispatch.rs tests/app_runtime.rs
git commit -m "refactor: add explicit issue close capability"
```

### Task 4: Implement the GitHub Client

**Files:**
- Create: `src/tracker/github/client.rs`
- Modify: `src/tracker/github/mod.rs`
- Create: `tests/github_client.rs`

**Step 1: Write the failing tests**

Cover with `wiremock`:

- fetch candidate issues from GitHub
- fetch a single issue
- replace workflow state labels
- add issue comment
- create PR
- fetch PR status
- merge PR
- close issue

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test github_client -- --nocapture
```

Expected:

- FAIL because the client does not exist

**Step 3: Write minimal implementation**

Implement a `GitHubClient` that:

- authenticates with `Authorization: Bearer ...`
- reads issues from `/repos/{owner}/{repo}/issues`
- ignores issue-list entries that are actually pull requests
- uses labels APIs to update workflow state
- uses pull request APIs for PR lifecycle
- uses issue PATCH close for issue completion

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test github_client -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/tracker/github/client.rs src/tracker/github/mod.rs tests/github_client.rs
git commit -m "feat: add github tracker client"
```

### Task 5: Replace GitCode Hardcoding in the App Runtime

**Files:**
- Modify: `src/app/mod.rs`
- Modify: `tests/app_runtime.rs`
- Modify: `README.md`

**Step 1: Write the failing test**

Update app runtime tests so the runtime builds a GitHub tracker path from config.

Add assertions that:

- GitHub configuration is loaded
- tracker selection is based on config, not hardcoded GitCode

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test app_runtime -- --nocapture
```

Expected:

- FAIL because `app` still hardcodes `GitCodeClient`

**Step 3: Write minimal implementation**

Refactor `app::reconcile_once()` so it:

- reads `default_tracker_kind`
- loads token env accordingly
- constructs `GitHubClient` for the live path
- preserves `reconcile_once_with()` for fake-tracker testing

Do not redesign orchestrator flow.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test app_runtime -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/app/mod.rs tests/app_runtime.rs README.md
git commit -m "feat: wire github tracker into app runtime"
```

### Task 6: Update PR Lifecycle to Close GitHub Issues on Completion

**Files:**
- Modify: `src/orchestrator/reconcile.rs`
- Modify: `tests/pr_reconcile.rs`
- Modify: `tests/e2e_fixture.rs`

**Step 1: Write the failing tests**

Add assertions that when a PR is approved and mergeable:

- PR is merged
- issue is closed explicitly
- run record becomes `Completed`

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test pr_reconcile -- --nocapture
cargo test --test e2e_fixture -- --nocapture
```

Expected:

- FAIL because completion does not call `close_issue`

**Step 3: Write minimal implementation**

In `reconcile_pr_watch()`:

- keep the existing merge path
- when `close_issue_on_merge` is true:
  - update workflow state to `Done`
  - call `close_issue`

Keep PR watch remove semantics unchanged.

**Step 4: Run tests to verify they pass**

Run the same commands again.

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/orchestrator/reconcile.rs tests/pr_reconcile.rs tests/e2e_fixture.rs
git commit -m "feat: close completed issues after github merge"
```

### Task 7: Add GitHub Example Configuration and Pilot Repository Registration

**Files:**
- Modify: `config/orchestrator.toml`
- Modify: `config/repositories/example.toml`
- Modify: `README.md`
- Create: `config/repositories/example-github.toml`

**Step 1: Write the failing documentation/config test**

Extend config and smoke coverage to expect:

- GitHub token env examples
- GitHub repo reference
- GitHub tracker kind examples

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test cli_smoke -- --nocapture
cargo test --test registry_loading -- --nocapture
```

Expected:

- FAIL if example config and docs are stale

**Step 3: Write minimal implementation**

Update shipped examples so they demonstrate GitHub as the live path.

Document:

- `GITHUB_TOKEN`
- `tracker_kind = "github"`
- `tracker_project_ref = "owner/repo"`

**Step 4: Run test to verify it passes**

Run the same commands again.

Expected:

- PASS

**Step 5: Commit**

```bash
git add config/orchestrator.toml config/repositories/example.toml config/repositories/example-github.toml README.md
git commit -m "docs: add github configuration examples"
```

### Task 8: Add `WORKFLOW.md` to the Pilot Repository

**Files:**
- Create: `/Users/nantas-agent/projects/game-design-patterns/WORKFLOW.md`
- Reference: `/Users/nantas-agent/projects/game-design-patterns/AGENTS.md`
- Reference: `/Users/nantas-agent/projects/game-design-patterns/README.md`

**Step 1: Write the workflow content**

Include:

- `active_states`
- `terminal_states`
- hooks
- retry policy
- PR policy
- completion policy
- prompt body that points the agent at `AGENTS.md`

Suggested frontmatter values:

```yaml
active_states:
  - Todo
terminal_states:
  - Done
hooks:
  after_create: []
  before_run:
    - uv sync
  after_run:
    - uv run pytest tests -q
retry_policy:
  max_attempts: 3
  backoff_seconds: 60
pr_policy:
  require_pr: true
completion_policy:
  close_issue_on_merge: true
```

**Step 2: Validate the workflow parses**

Run:

```bash
cargo test --test workflow_parser -- --nocapture
```

Expected:

- PASS after any necessary parser-compatible formatting adjustments

**Step 3: Commit**

```bash
git -C /Users/nantas-agent/projects/game-design-patterns add WORKFLOW.md
git -C /Users/nantas-agent/projects/game-design-patterns commit -m "docs: add symphony workflow contract"
```

### Task 9: Add a Codex Runner Wrapper to the Pilot Repository

**Files:**
- Create: `/Users/nantas-agent/projects/game-design-patterns/tools/run_symphony_agent.sh`
- Optionally Modify: `/Users/nantas-agent/projects/game-design-patterns/README.md`

**Step 1: Write the wrapper script**

The script must:

- run in the current workspace
- read `PROMPT`
- invoke `codex CLI`
- run `uv sync`
- run `uv run pytest tests -q`
- print JSON like:

```json
{"status":"success","summary":"implemented","branch_name":"feat/...","commit_sha":"...","requested_next_action":null}
```

**Step 2: Verify it is executable**

Run:

```bash
chmod +x /Users/nantas-agent/projects/game-design-patterns/tools/run_symphony_agent.sh
```

**Step 3: Smoke test the script manually**

Run:

```bash
PROMPT="test prompt" /Users/nantas-agent/projects/game-design-patterns/tools/run_symphony_agent.sh
```

Expected:

- script emits valid JSON

**Step 4: Commit**

```bash
git -C /Users/nantas-agent/projects/game-design-patterns add tools/run_symphony_agent.sh README.md
git -C /Users/nantas-agent/projects/game-design-patterns commit -m "feat: add codex runner wrapper"
```

### Task 10: Register the Pilot Repository for GitHub Integration

**Files:**
- Modify: `config/repositories/example.toml` or create a pilot-specific config under `config/repositories/`
- Modify: `config/orchestrator.toml`

**Step 1: Create pilot config**

Include:

- `repo_id = "game-design-patterns"`
- `repo_path = "/Users/nantas-agent/projects/game-design-patterns"`
- `workflow_path = "/Users/nantas-agent/projects/game-design-patterns/WORKFLOW.md"`
- `tracker_kind = "github"`
- `tracker_project_ref = "nantas1/game-design-patterns"`
- `default_runner = "process"`
- `enabled = true`
- `max_concurrent_runs = 1`

Set global runner values to call the wrapper:

- `runner_program = "/bin/sh"`
- `runner_args = ["-lc", "./tools/run_symphony_agent.sh"]`

**Step 2: Validate config**

Run:

```bash
GITHUB_TOKEN=placeholder cargo run -- --config config/orchestrator.toml validate-config
```

Expected:

- PASS if all paths resolve and configuration loads

**Step 3: Commit**

```bash
git add config/orchestrator.toml config/repositories/
git commit -m "chore: register github pilot repository"
```

### Task 11: Run Live Smoke Tests Against GitHub

**Files:**
- No code files required
- Operator checklist output should be recorded in notes or docs

**Step 1: Prepare environment**

Verify:

```bash
echo "$GITHUB_TOKEN"
codex --help
uv --version
git -C /Users/nantas-agent/projects/game-design-patterns remote -v
```

Expected:

- token present
- codex available
- uv available
- git remote points to GitHub

**Step 2: Run one reconciliation pass**

Run:

```bash
cargo run -- --config config/orchestrator.toml reconcile-once
```

Expected:

- one `todo` issue moves to `in-progress`
- runner executes
- PR is created
- issue moves to `human-review`

**Step 3: Approve the PR manually**

Use GitHub UI or CLI to approve the created PR.

**Step 4: Run another reconciliation pass**

Run:

```bash
cargo run -- --config config/orchestrator.toml reconcile-once
```

Expected:

- PR merges
- issue gets `done`
- issue closes

**Step 5: Record residual issues**

Document:

- auth problems
- label conflicts
- mergeability delays
- runner failures

No commit required for this task unless docs are updated.

### Task 12: Final Verification and Cleanup

**Files:**
- Any updated docs from smoke-test findings

**Step 1: Run full verification**

Run:

```bash
cargo test
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected:

- PASS

**Step 2: Check git status**

Run:

```bash
git status --short
git -C /Users/nantas-agent/projects/game-design-patterns status --short
```

Expected:

- no unexpected leftovers

**Step 3: Commit any final doc fixes**

```bash
git add README.md docs/plans/
git commit -m "docs: record github integration follow-up"
```

Only if there are actual doc changes.

Plan complete and saved to `docs/plans/2026-03-11-github-tracker-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
