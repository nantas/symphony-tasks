# Production Reliability Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the GitHub-backed single-instance orchestrator so it can run as a production-ready V1 with real retry consumption, terminal-state convergence, cycle summaries, and deployable operator guidance.

**Architecture:** Keep the existing orchestrator core (`dispatch_issue`, `create_pr_for_run`, `reconcile_pr_watch`) intact and add a scheduling-control layer around `reconcile_once()`. Treat the orchestrator as the only authority over scheduling state, make retry queue entries participate in dispatch, add explicit terminal convergence and summary emission, and document a Linux/systemd deployment contract.

**Tech Stack:** Rust, `tokio`, file-backed state store, GitHub REST API, process runner, TOML config, markdown docs

---

### Task 1: Add Scheduling Summary and Retry-Aware Selection Tests

**Files:**
- Modify: `tests/orchestrator_selection.rs`
- Modify: `tests/restart_recovery.rs`
- Modify: `tests/app_runtime.rs`
- Inspect: `src/app/mod.rs`
- Inspect: `src/orchestrator/reconcile.rs`

**Step 1: Write the failing tests**

Add tests covering:

- retry entries that are not yet due still block dispatch
- retry entries that are due become dispatch-eligible
- `reconcile_once()` returns a richer summary with retry and convergence counters

Example assertion shape:

```rust
assert_eq!(summary.retries_requeued, 1);
assert_eq!(summary.skipped_due_to_backoff, 1);
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test orchestrator_selection -- --nocapture
cargo test --test app_runtime -- --nocapture
cargo test --test restart_recovery -- --nocapture
```

Expected:

- FAIL because retry queue is not yet consumed during reconcile
- FAIL because the summary only tracks dispatched runs and reconciled PRs

**Step 3: Write the minimal implementation**

Extend the in-memory summary and selection inputs so reconcile can distinguish:

- dispatched runs
- reconciled PRs
- due retry entries
- backoff-skipped entries
- terminal convergence actions

Do not redesign run status storage in this task.

**Step 4: Run tests to verify they pass**

Run the same commands again.

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/app/mod.rs src/orchestrator/reconcile.rs tests/orchestrator_selection.rs tests/restart_recovery.rs tests/app_runtime.rs
git commit -m "feat: add retry-aware reconcile summaries"
```

### Task 2: Consume Retry Queue During Reconcile

**Files:**
- Modify: `src/app/mod.rs`
- Modify: `src/state_store/mod.rs`
- Modify: `src/orchestrator/reconcile.rs`
- Modify: `tests/orchestrator_selection.rs`
- Modify: `tests/restart_recovery.rs`

**Step 1: Write the failing tests**

Add tests covering:

- due retry entries are selected before fresh candidate issues when capacity is limited
- retry queue entries are retained when still not due
- retry queue entries are removed or updated when re-dispatched

Example assertion shape:

```rust
assert_eq!(selected[0].id, "100");
assert!(remaining_retry.iter().all(|entry| entry.issue_id != "100"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test orchestrator_selection -- --nocapture
cargo test --test restart_recovery -- --nocapture
```

Expected:

- FAIL because retry queue is loaded but ignored in live dispatch logic

**Step 3: Write the minimal implementation**

Implement:

- due/not-due retry partitioning
- retry entries participating in dispatch candidate selection
- retry queue write-back after each reconcile cycle

Keep retry state file format unchanged unless tests force a targeted change.

**Step 4: Run tests to verify they pass**

Run the same commands again.

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/app/mod.rs src/state_store/mod.rs src/orchestrator/reconcile.rs tests/orchestrator_selection.rs tests/restart_recovery.rs
git commit -m "feat: consume retry queue during reconcile"
```

### Task 3: Converge Terminal and Externally Closed Issues

**Files:**
- Modify: `src/app/mod.rs`
- Modify: `src/orchestrator/reconcile.rs`
- Modify: `tests/pr_reconcile.rs`
- Modify: `tests/app_runtime.rs`
- Modify: `tests/e2e_fixture.rs`

**Step 1: Write the failing tests**

Cover:

- issue closed externally with no active PR watch is removed from active scheduling
- issue already in terminal workflow state is not re-dispatched
- PR closed but unmerged does not stay in an infinite watch loop

Example assertion shape:

```rust
assert_eq!(summary.terminal_converged, 1);
assert!(state_store.load_pr_watch_state().unwrap().is_empty());
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test pr_reconcile -- --nocapture
cargo test --test app_runtime -- --nocapture
cargo test --test e2e_fixture -- --nocapture
```

Expected:

- FAIL because external terminal states are not yet treated as explicit convergence paths

**Step 3: Write the minimal implementation**

Add a terminal-convergence pass that:

- checks watched and locally active issues against remote state
- removes obsolete watch/retry entries
- prevents re-dispatch of terminal issues
- records the convergence result in the reconcile summary

Do not add new workflow states in this task.

**Step 4: Run tests to verify they pass**

Run the same commands again.

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/app/mod.rs src/orchestrator/reconcile.rs tests/pr_reconcile.rs tests/app_runtime.rs tests/e2e_fixture.rs
git commit -m "feat: converge external terminal issue states"
```

### Task 4: Emit Structured Cycle Summary and Event Logs

**Files:**
- Modify: `src/app/mod.rs`
- Modify: `src/main.rs`
- Create or Modify: `src/logging/mod.rs`
- Modify: `tests/cli_smoke.rs`
- Modify: `tests/app_runtime.rs`

**Step 1: Write the failing tests**

Add tests covering:

- reconcile emits a summary event with counts
- daemon/reconcile-once paths include stable log fields for operators

Example assertion shape:

```rust
assert!(stderr.contains("event=reconcile_summary"));
assert!(stderr.contains("dispatched_runs=1"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test cli_smoke -- --nocapture
cargo test --test app_runtime -- --nocapture
```

Expected:

- FAIL because the current runtime has no structured summary emission contract

**Step 3: Write the minimal implementation**

Add:

- one structured summary event per reconcile cycle
- per-issue event logs for dispatch, retry scheduling, terminal convergence, and PR closeout

Keep logging implementation lightweight; do not add metrics infrastructure.

**Step 4: Run tests to verify they pass**

Run the same commands again.

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/app/mod.rs src/main.rs src/logging/ tests/cli_smoke.rs tests/app_runtime.rs
git commit -m "feat: add reconcile summary logging"
```

### Task 5: Define Linux Deployment and Operations Baseline

**Files:**
- Modify: `README.md`
- Create: `docs/operations/systemd.md`
- Create: `docs/operations/smoke-checklist.md`
- Create or Modify: `config/orchestrator.example.toml`

**Step 1: Write the documentation updates**

Document:

- required environment variables
- expected directory layout
- `systemd` service unit example
- log inspection commands
- minimal smoke and recovery checklist

Include concrete command examples for:

- `validate-config`
- `reconcile-once`
- daemon startup
- reading run/retry/pr-watch state files

**Step 2: Verify docs and examples stay coherent**

Run:

```bash
cargo test --test cli_smoke -- --nocapture
GITHUB_TOKEN=placeholder cargo run -- --config config/orchestrator.toml validate-config
```

Expected:

- PASS

**Step 3: Commit**

```bash
git add README.md docs/operations/systemd.md docs/operations/smoke-checklist.md config/orchestrator.example.toml
git commit -m "docs: add production deployment baseline"
```

### Task 6: Final Verification and Reliability Smoke

**Files:**
- Any updated docs from Task 5

**Step 1: Run full verification**

Run:

```bash
cargo test
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
git status --short
```

Expected:

- PASS
- only intentional changes remain

**Step 2: Run a targeted reliability smoke**

Run:

```bash
zsh -lic 'cargo run -- --config config/orchestrator.toml reconcile-once'
```

Then verify:

- summary logging is emitted
- retry/pr-watch state files remain internally consistent
- no terminal issue is re-dispatched

Expected:

- PASS or a clearly documented follow-up item

**Step 3: Commit any final doc-only follow-up**

```bash
git add README.md docs/operations/
git commit -m "docs: capture reliability smoke follow-up"
```

Only if there are actual doc changes.

Plan complete and saved to `docs/plans/2026-03-11-production-reliability-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
