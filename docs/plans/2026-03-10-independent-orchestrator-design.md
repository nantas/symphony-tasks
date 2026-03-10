# Independent Symphony Orchestrator Design

## Context

This repository will host an independent Symphony-compatible orchestrator rather than embedding orchestration runtime code into a target product repository. The design follows the layered boundaries from `/Volumes/Shuttle/projects/agentic/symphony/SPEC.md` and the multi-repo split described in `/Users/nantas-agent/Projects/obsidian-mind/30_研究/Symphony/Symphony 协调器多仓库方案设计.md`.

V1 scope is intentionally narrow:

- GitCode Issues as the task source
- GitCode PR as the review and merge artifact
- single target repository runtime loop
- multi-repository architecture from day one
- one built-in agent runner implementation behind a generic runner trait
- local file persistence only
- both daemon mode and one-shot reconcile mode
- Linux-compatible deployment from the start

## Goals

- Build an independent orchestrator runtime that can later serve multiple repositories.
- Keep workflow policy inside the target repository via `WORKFLOW.md`.
- Drive issues from active state to PR, human review, merge, and final completion.
- Recover after process restart without introducing an external database.
- Avoid coupling the orchestrator core to GitCode API details or a specific CLI agent.

## Non-Goals

- No multi-tenant control plane or web UI in V1.
- No repository cloning or repo bootstrap in V1; registered repositories already exist locally.
- No generalized dependency graph or blocker engine in V1.
- No multiple runner implementations in V1.
- No external database, queue, or message bus.

## Recommended Approach

Use a single Rust binary with trait-based internal module boundaries and file-backed runtime state.

Why this approach:

- It keeps the runtime small enough for V1 while preserving long-term architecture.
- It supports both `daemon` and `reconcile-once` without duplicating logic.
- It matches Symphony's configuration and coordination split.
- It avoids the rewrite cost of a throwaway linear PoC.

## System Architecture

The binary exposes two execution modes:

- `daemon`: loop on a fixed poll interval and repeatedly call `reconcile_once()`
- `reconcile-once`: execute one full coordination pass and exit

Both modes share the same application services and domain model.

### Module Boundaries

#### `app`

Bootstraps the process:

- CLI argument parsing
- global config loading
- logger setup
- single-instance lock acquisition
- service graph construction

#### `registry`

Owns repository registration metadata:

- local repository path
- `WORKFLOW.md` path
- GitCode project reference
- runner selection
- enabled flag
- repository-level concurrency settings

V1 runs one repository end to end, but the model supports multiple repositories.

#### `workflow`

Loads and parses the target repository's `WORKFLOW.md`:

- frontmatter configuration
- prompt template body
- state mapping
- retry policy
- workspace hook commands
- PR, merge, and close rules

This is the only repo-defined policy entry point.

#### `tracker`

Defines traits for normalized task and PR operations. V1 includes a `gitcode` adapter implementation.

Expected capabilities:

- fetch candidate issues
- fetch issue by id
- refresh issue states
- create progress comments
- update issue state
- create or update PR
- query PR review and merge state
- merge PR
- close or complete issue

The orchestrator depends on these capabilities, not on raw REST payloads.

#### `workspace`

Manages per-issue isolated work directories derived from the registered local repository path:

- create workspace root
- create or reuse issue workspace
- run `after_create`, `before_run`, `after_run`, `before_remove` hooks
- provide normalized paths to the runner
- clean terminal workspaces when eligible

V1 does not clone repositories.

#### `agent_runner`

Defines a generic runner trait and one built-in implementation:

- render final prompt from issue data plus workflow template
- launch agent process in the workspace
- collect structured result
- return branch, commit, summary, artifacts, and next action hints

The runner does not own long-lived review waiting logic.

#### `orchestrator`

Single authoritative coordinator for runtime behavior:

- poll active work
- decide eligibility
- claim and release tasks
- enforce concurrency limits
- dispatch agent runs
- reconcile PR waiting tasks
- retry transient failures
- converge external and internal state

Its core unit is `reconcile_once()`.

#### `state_store`

Persists the minimum runtime state required for restart recovery:

- repository snapshots
- run records
- retry queue
- PR watch state
- daemon lock metadata

No external database is used.

## Core Data Model

### `RepositoryProfile`

- `repo_id`
- `repo_path`
- `workflow_path`
- `gitcode_project_ref`
- `default_runner`
- `enabled`
- `max_concurrent_runs`

### `WorkflowDefinition`

- `config`
- `prompt_template`
- `state_mapping`
- `hooks`
- `retry_policy`
- `pr_policy`
- `completion_policy`

### `NormalizedIssue`

- `id`
- `identifier`
- `repo_id`
- `title`
- `description`
- `state`
- `priority`
- `labels`
- `url`
- `created_at`
- `updated_at`

### `RunRecord`

- `issue_id`
- `repo_id`
- `attempt`
- `workspace_path`
- `status`
- `branch_name`
- `commit_sha`
- `pr_ref`
- `started_at`
- `updated_at`
- `last_error`
- `next_retry_at`

### `PullRequestRef`

- `id`
- `number`
- `url`
- `head_branch`
- `state`
- `review_status`
- `merge_status`

### `AgentRunResult`

- `status`
- `branch_name`
- `commit_sha`
- `summary`
- `artifacts`
- `requested_next_action`
- `pr_payload`

## State Machine

### External Issue States

Workflow policy maps GitCode issue states into this conceptual set:

- `Todo`
- `In Progress`
- `Human Review`
- `Done`
- `Blocked`
- `Failed`

### Internal Run States

- `queued`
- `claiming`
- `preparing_workspace`
- `running_agent`
- `awaiting_pr_creation`
- `awaiting_human_review`
- `approved_for_merge`
- `merging`
- `completed`
- `retry_backoff`
- `blocked`
- `failed`

### Main Lifecycle

1. Eligible issue appears in an active workflow state.
2. Orchestrator claims it and moves external state to `In Progress`.
3. Workspace is prepared and hooks run.
4. Agent runner executes in the issue workspace.
5. If code is produced successfully, the orchestrator creates or updates a PR.
6. Issue moves to `Human Review`.
7. Reconcile loop watches PR approval state.
8. After approval and mergeability checks pass, orchestrator merges the PR.
9. Successful merge advances the issue to `Done`.

Important invariant:

- agent completion is not task completion

Because GitCode blocker and dependency support is incomplete, V1 does not build blockers into dispatch eligibility beyond optional lightweight workflow rules such as labels.

## Reconcile Flow

`reconcile_once()` executes this sequence:

1. Load enabled repository profiles from `registry`.
2. Load each repository's `WORKFLOW.md` and derive typed runtime config.
3. Ask the tracker adapter for candidate issues in active states.
4. Rebuild active runtime view from persisted state.
5. Reconcile existing waiting runs:
   - detect merged PRs
   - detect approved PRs
   - detect canceled or externally closed issues
   - release or complete runs as needed
6. Select new dispatch candidates respecting:
   - global concurrency limit
   - repository concurrency limit
   - active claims
   - retry backoff
   - workflow eligibility
7. For each selected issue:
   - claim the issue
   - update issue state to `In Progress`
   - prepare workspace
   - execute runner
   - persist result
   - create or update PR if applicable
   - move issue to `Human Review`
8. Persist updated runtime state and exit or sleep until next poll.

### Interaction Rules

- `orchestrator` depends on traits only.
- `workflow` provides policy and templates, not side effects.
- `workspace` owns filesystem lifecycle only.
- `agent_runner` owns one execution attempt only.
- `tracker` hides GitCode-specific transport and payload details.

## Configuration Contract

Configuration is split across three layers.

### Global Config

File: `config/orchestrator.toml`

Suggested fields:

- poll interval
- execution mode defaults
- global concurrency
- log level
- state root
- workspace root
- lock path
- GitCode token environment variable name
- default runner kind

### Repository Config

Files: `config/repositories/<repo-id>.toml`

Suggested fields:

- `repo_id`
- `repo_path`
- `workflow_path`
- GitCode project reference
- repository concurrency limit
- enabled flag
- optional runner override

### Repo-Defined Workflow

File inside target repository: `WORKFLOW.md`

Frontmatter defines:

- active states
- terminal states
- state mapping
- retry policy
- workspace hooks
- review policy
- merge policy
- completion policy

Body defines the prompt template rendered for the runner.

## Persistence Layout

Use file-backed state under `var/`:

```text
var/
  state/
    registry.snapshot.json
    runtime.json
    retry_queue.json
    pr_watch.json
  runs/
    <repo-id>/
      <issue-id>.json
  logs/
  locks/
    daemon.lock
  workspaces/
    <repo-id>/
      <issue-key>/
```

Design choices:

- split run records by issue for inspectability and partial recovery
- avoid one giant mutable state file
- keep lock management separate from state documents
- allow human debugging with ordinary filesystem tools

## Error Handling and Recovery

### Failure Categories

#### 1. Transient platform failures

Examples:

- GitCode API timeout
- temporary 5xx
- rate limiting

Response:

- record error
- move to `retry_backoff`
- retry with exponential backoff

#### 2. Environment failures

Examples:

- repository path missing
- invalid runner executable
- hook command failure

Response:

- mark the run as failed
- retry only when policy explicitly allows it

#### 3. Agent execution failures

Examples:

- non-zero exit
- missing branch or commit output
- incomplete PR prerequisites

Response:

- persist structured error details
- retry up to workflow-defined limit
- never advance to `Human Review` unless PR prerequisites are satisfied

#### 4. Reconciliation inconsistencies

Examples:

- issue manually closed while a run exists
- PR already merged while local state still waits
- local process crashed mid-run

Response:

- treat GitCode as the source of truth
- use persisted run records as restart hints
- reconcile to the externally observed final state

### Restart Recovery

On startup:

1. Load persisted state files.
2. Reconstruct retry queue.
3. Reconstruct PR watch set.
4. Detect interrupted runs with no active process.
5. Reconcile each interrupted run:
   - if issue is inactive, release it
   - if PR exists, move to review-waiting
   - if PR is merged, complete it
   - otherwise requeue it

Daemon mode must enforce a single-instance lock to prevent duplicate dispatch.

## Testing Strategy

### Unit Tests

- workflow frontmatter parsing
- state mapping logic
- retry and backoff calculation
- issue and PR normalization
- workspace key and path generation
- state machine transition rules

### Integration Tests

- file persistence and restart recovery
- `reconcile_once()` against fake tracker and fake runner
- PR approval to merge convergence
- issue cancellation during run

### Adapter Tests

- GitCode request construction
- response decoding
- error mapping
- pagination behavior
- state update payload construction

Prefer mock and fixture-driven tests first, with a small set of real-environment smoke tests later.

### Manual Acceptance Scenarios

- issue moves from `Todo` to `Human Review` with a created PR
- approved PR is auto-merged and issue moves to `Done`
- process restarts and resumes consistent state
- issue is manually canceled or closed and the run is released
- token, path, or hook failures surface with clear diagnostics

## Success Criteria for V1

- The orchestrator loads one registered local repository and its `WORKFLOW.md`.
- It fetches eligible GitCode issues and dispatches an isolated workspace run.
- It executes one built-in agent runner through a stable runner interface.
- It creates or links a GitCode PR and advances the issue to `Human Review`.
- It watches PR approval state, auto-merges when policy allows, and advances the issue to `Done`.
- It restarts cleanly using only local file persistence.
- The same core logic supports both daemon mode and one-shot reconcile mode.

## Deferred Work

These items are explicitly left for later phases:

- true multi-repository active scheduling
- repository auto-clone and sync
- multiple runner implementations
- richer observability surface such as HTTP or TUI
- dependency and blocker graph semantics
- stronger distributed locking
- SQLite or event-log persistence

## Current Implementation Snapshot

The current implementation already includes:

- config and repository registration loading
- `WORKFLOW.md` parsing
- file-backed state persistence
- workspace lifecycle hooks
- process-backed agent runner
- GitCode tracker and PR client
- dispatch selection, run dispatch, PR creation, PR watch, and auto-merge
- restart recovery and daemon lock handling

Local operator commands:

- `cargo run -- --config config/orchestrator.toml validate-config`
- `cargo run -- --config config/orchestrator.toml reconcile-once`
- `cargo run -- --config config/orchestrator.toml daemon`

## Initial Repository Layout

```text
symphony-tasks/
  Cargo.toml
  src/
    main.rs
    cli/
    app/
    orchestrator/
    registry/
    workflow/
    tracker/
      mod.rs
      gitcode/
    workspace/
    agent_runner/
    state_store/
    models/
    logging/
  config/
    orchestrator.toml
    repositories/
  docs/
    plans/
  var/
    state/
    runs/
    logs/
    locks/
    workspaces/
```
