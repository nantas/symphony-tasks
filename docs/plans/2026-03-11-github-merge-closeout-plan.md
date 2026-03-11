# GitHub Merge Closeout Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let GitHub-backed runs finish correctly whether the orchestrator merges an approved PR or a human merges the PR manually in GitHub.

**Architecture:** Keep the current PR creation and watch flow. Expand PR reconciliation so completion is triggered by the observed merged state, not only by a successful local `merge_pr` call. Preserve `completion_policy.close_issue_on_merge = true`, and document the dual completion path in the pilot repository workflow.

**Tech Stack:** Rust, `tokio`, `reqwest`, `wiremock`, GitHub REST API, existing workflow parser/state store

---

### Task 1: Document the Dual Merge Workflow Contract

**Files:**
- Modify: `/Users/nantas-agent/projects/game-design-patterns/WORKFLOW.md`
- Modify: `docs/plans/2026-03-11-github-tracker-implementation-plan.md`

**Step 1: Write the documentation change**

Update the pilot `WORKFLOW.md` prose to say:

- PRs may be merged by the orchestrator after approval
- or merged manually by a user in GitHub
- after either merge path, the orchestrator is responsible for moving the issue to `Done` and closing it

Add an addendum to the existing implementation plan so Task 11 and Task 12 accept either merge path as a valid live-smoke completion route.

**Step 2: Validate the workflow still parses**

Run:

```bash
cargo test --test workflow_parser -- --nocapture
GITHUB_TOKEN=placeholder cargo run -- --config config/orchestrator.toml validate-config
```

Expected:

- PASS

**Step 3: Commit**

```bash
git -C /Users/nantas-agent/projects/game-design-patterns add WORKFLOW.md
git add docs/plans/2026-03-11-github-tracker-implementation-plan.md
git commit -m "docs: describe dual github merge closeout"
```

### Task 2: Add a Regression Test for Human-Merged PR Closeout

**Files:**
- Modify: `tests/pr_reconcile.rs`
- Modify: `tests/e2e_fixture.rs`

**Step 1: Write the failing tests**

Add tests covering:

- a watched PR with `merge_status = MergeStatus::Merged`
- no call to `merge_pr`
- issue state updated to `Done`
- issue explicitly closed
- run record persisted as `Completed`
- PR watch entry removed

Example assertion shape:

```rust
assert!(tracker.merged_prs.lock().unwrap().is_empty());
assert_eq!(updated.status, RunStatus::Completed);
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test pr_reconcile -- --nocapture
cargo test --test e2e_fixture -- --nocapture
```

Expected:

- FAIL because externally merged PRs still stay in `AwaitingHumanReview`

**Step 3: Commit the failing expectation only after implementation passes**

Do not commit in the red state.

### Task 3: Finalize Runs When a PR Is Already Merged

**Files:**
- Modify: `src/orchestrator/reconcile.rs`
- Modify: `tests/pr_reconcile.rs`
- Modify: `tests/e2e_fixture.rs`

**Step 1: Write the minimal implementation**

Refactor `reconcile_pr_watch()` so it uses one shared completion branch:

- if `status.pr.merge_status == MergeStatus::Merged`, skip `merge_pr` and finalize immediately
- else if `review_status == Approved` and `merge_status == Mergeable`, call `merge_pr` and then finalize
- else keep the run in `AwaitingHumanReview`

The finalize branch must:

- set run status to `Completed`
- update issue state to `Done` when `close_issue_on_merge` is enabled
- call `close_issue`
- remove the PR watch entry

**Step 2: Run tests to verify they pass**

Run:

```bash
cargo test --test pr_reconcile -- --nocapture
cargo test --test e2e_fixture -- --nocapture
```

Expected:

- PASS

**Step 3: Commit**

```bash
git add src/orchestrator/reconcile.rs tests/pr_reconcile.rs tests/e2e_fixture.rs
git commit -m "feat: finalize manually merged github prs"
```

### Task 4: Lock In GitHub Merged-PR Status Mapping

**Files:**
- Modify: `tests/github_mapping.rs`
- Modify: `tests/github_client.rs`
- Inspect: `src/tracker/github/models.rs`
- Inspect: `src/tracker/github/client.rs`

**Step 1: Write the failing tests**

Add coverage proving:

- a GitHub PR payload with `"merged": true` maps to `MergeStatus::Merged`
- `get_pr_status()` returns a merged PR that can drive the new reconciliation branch

Example shape:

```rust
assert_eq!(normalized.merge_status, MergeStatus::Merged);
```

**Step 2: Run tests to verify they fail or expose any stale assumptions**

Run:

```bash
cargo test --test github_mapping -- --nocapture
cargo test --test github_client -- --nocapture
```

Expected:

- FAIL if merged PR mapping or status fixtures do not match the new reconciliation behavior

**Step 3: Write the minimal implementation if needed**

If the tests fail:

- keep GitHub PR normalization based on `merged`
- adjust fixture payloads or client decoding only as needed

**Step 4: Run tests to verify they pass**

Run the same commands again.

Expected:

- PASS

**Step 5: Commit**

```bash
git add tests/github_mapping.rs tests/github_client.rs src/tracker/github/models.rs src/tracker/github/client.rs
git commit -m "test: cover github merged pr reconciliation"
```

### Task 5: Re-Run Live Smoke With Dual Completion Acceptance

**Files:**
- No code files required

**Step 1: Verify the current PR state**

Run:

```bash
gh pr view 2 --repo nantas/game-design-database --json number,state,reviewDecision,mergeStateStatus,isDraft,url
gh issue view 1 --repo nantas/game-design-database --json number,state,labels,url
```

Expected:

- current PR and issue state are visible

**Step 2: Exercise one valid completion path**

Path A:

```bash
zsh -lic 'cargo run -- --config config/orchestrator.toml reconcile-once'
```

Expected:

- if the PR is approved and still open, the orchestrator merges it and closes the issue

Path B:

- merge the PR manually in GitHub
- then run:

```bash
zsh -lic 'cargo run -- --config config/orchestrator.toml reconcile-once'
```

Expected:

- the orchestrator detects the PR is already merged
- the issue gets `done`
- the issue closes

**Step 3: Record residual issues**

Document:

- approval timing delays
- mergeability refresh delays
- manual merge detection timing
- any issue label drift

No commit required unless docs change.

### Task 6: Final Verification and Cleanup

**Files:**
- Any updated docs from Task 1 or Task 5

**Step 1: Run full verification**

Run:

```bash
cargo test
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
git status --short
git -C /Users/nantas-agent/projects/game-design-patterns status --short
```

Expected:

- PASS
- no unexpected leftovers

**Step 2: Commit any final doc-only follow-up**

```bash
git add README.md docs/plans/
git commit -m "docs: record github dual-merge smoke follow-up"
```

Only if there are actual doc changes.

Plan complete and saved to `docs/plans/2026-03-11-github-merge-closeout-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
