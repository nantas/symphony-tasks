# GitHub Tracker Design

## Context

As of Tuesday, March 11, 2026, the current `main` branch of `symphony-tasks` already ships a working GitCode-oriented V1 orchestrator:

- single Rust binary
- `daemon`, `reconcile-once`, `validate-config`
- local file state
- process-backed runner
- PR watch reconciliation
- restart recovery and daemon locking

That baseline is usable, but real-project integration has changed direction:

- GitCode is no longer the preferred live platform
- the next live integration target is `GitHub Issues + GitHub PR`
- workflow state on GitHub will be represented through issue labels
- the first pilot repository is `/Users/nantas-agent/projects/game-design-patterns`
- the execution engine inside the target workspace will use `codex CLI`

This document defines the design for the GitHub transition without discarding the current orchestrator core.

## Goals

- Reuse the existing orchestrator runtime, state store, workspace manager, and process runner.
- Add a GitHub tracker adapter for issues, labels, pull requests, merge, and issue close.
- Keep the orchestrator core platform-agnostic.
- Model workflow state using GitHub issue labels instead of platform-native workflow states.
- Support a pilot live integration for `game-design-patterns`.
- Preserve Linux-compatible deployment.

## Non-Goals

- No GitHub Projects integration in V1.
- No multi-platform production parity in this change.
- No codex app-server integration in this change.
- No web UI, dashboard, or external database.
- No generalized multi-runner framework beyond the current process runner.

## Recommended Approach

Implement a new `tracker::github` adapter and shift platform-specific orchestration concerns behind the existing `Tracker` trait.

Key principle:

- keep `orchestrator` working on normalized issue state names such as `Todo`, `In Progress`, `Human Review`, `Done`
- let the GitHub adapter translate between those internal states and GitHub issue labels such as `todo`, `in-progress`, `human-review`, `done`

This keeps the workflow contract stable while replacing the external platform.

## Options Considered

### Option 1: GitHub-only V1 adapter with label-driven state

Add a clean GitHub adapter, keep trait boundaries, and route real integration through GitHub labels.

Pros:

- aligns with current orchestrator shape
- lowest real-integration risk
- no dependency on GitHub Projects schemas
- straightforward API contract

Cons:

- requires explicit label discipline
- needs label conflict handling

### Option 2: Full multi-platform framework now

Make GitCode and GitHub first-class at the same time with complete config and documentation parity.

Pros:

- best long-term symmetry

Cons:

- increases scope immediately
- slows the pilot repository integration

### Option 3: Hard-replace GitCode logic with GitHub logic

Rewrite platform-specific code in place and stop carrying any GitCode assumptions.

Pros:

- simpler short-term code graph

Cons:

- throws away a working baseline
- reduces reuse value from the already-built GitCode V1

### Recommendation

Choose a hybrid of Option 1 and future extensibility:

- implement GitHub cleanly
- keep adapter boundaries reusable
- do not spend this iteration on full multi-platform completion

This matches the user decision:

- architecture should remain extensible
- this delivery only needs GitHub as the live integration target

## State Model

### Internal State Model

The orchestrator should continue to reason about normalized workflow states:

- `Todo`
- `In Progress`
- `Human Review`
- `Done`
- optional future states: `Blocked`, `Failed`

These are the values consumed by:

- workflow `active_states`
- workflow `terminal_states`
- dispatch selection
- PR lifecycle updates

### GitHub External State Model

GitHub will use issue labels as the workflow state carrier.

Recommended pilot labels:

- `todo`
- `in-progress`
- `human-review`
- `done`

Optional future labels:

- `blocked`
- `failed`

GitHub native `open` / `closed` remains useful, but only as a lifecycle envelope:

- `open` means the issue may still participate in orchestration
- `closed` is always terminal

### Label Mapping Rules

GitHub adapter responsibilities:

1. Read all labels from an issue.
2. Find the active workflow-state label.
3. Normalize that label into internal state text.
4. When updating state:
   - remove any old workflow-state labels
   - add the new workflow-state label
5. When completing an issue after merge:
   - set the `done` label
   - close the issue

### Label Conflict Handling

If an issue contains multiple workflow-state labels at once, the adapter should not guess.

Recommended V1 behavior:

- treat the issue as invalid for dispatch
- log a clear warning
- leave the issue untouched

This is safer than making hidden precedence rules.

## Configuration Contract

### Global Config

Current global config is GitCode-oriented. It should be generalized.

Recommended fields:

- `poll_interval_secs`
- `global_concurrency`
- `log_level`
- `state_root`
- `workspace_root`
- `lock_path`
- `default_runner`
- `runner_program`
- `runner_args`
- `repositories_dir`
- `default_tracker_kind`
- `github_token_env`

Notes:

- `gitcode_token_env` should be removed from the live path
- `default_tracker_kind = "github"` should drive the pilot configuration

### Repository Config

Current repository config includes `gitcode_project_ref`, which is too platform-specific.

Recommended repository fields:

- `repo_id`
- `repo_path`
- `workflow_path`
- `tracker_kind`
- `tracker_project_ref`
- `default_runner`
- `enabled`
- `max_concurrent_runs`

For GitHub, `tracker_project_ref` will hold `owner/repo`, for example:

- `nantas1/game-design-patterns`

### Workflow Contract

`WORKFLOW.md` should remain platform-neutral. It should continue to define:

- `active_states`
- `terminal_states`
- `state_mapping`
- hooks
- retry policy
- PR policy
- completion policy

For the pilot repository, workflow state names remain human-oriented:

- `Todo`
- `In Progress`
- `Human Review`
- `Done`

The GitHub adapter is responsible for mapping labels to those names.

## Module Design

### `tracker/github`

Add:

- `src/tracker/github/mod.rs`
- `src/tracker/github/models.rs`
- `src/tracker/github/client.rs`

Responsibilities:

- fetch candidate issues from GitHub
- fetch a single issue
- update issue workflow state via labels
- add issue comments
- create pull requests
- inspect pull request review and mergeability state
- merge pull requests
- close issues

### `tracker/mod.rs`

Keep the `Tracker` trait as the main platform boundary.

Add:

- `pub mod github;`

Potential trait expansion:

- current trait already covers most lifecycle calls
- issue close can either:
  - be added as a new trait method
  - or be encoded inside `update_issue_state`

Recommendation:

- add an explicit issue-close capability to the trait

Reason:

- GitHub `done` plus `close issue` is not the same thing as a workflow label update
- making close explicit is clearer and easier to test

### `app`

Current `app::reconcile_once()` directly builds `GitCodeClient`.

This must change to a tracker factory model:

- load config
- load repository profiles
- choose tracker kind for the active repository
- build the corresponding client

V1 live path:

- only GitHub will be instantiated in production

But the construction path should stop hardcoding GitCode.

### `models::repository`

Current shape leaks GitCode:

- `gitcode_project_ref`

Replace with platform-neutral metadata:

- `tracker_kind`
- `tracker_project_ref`

This reduces future migration cost.

## GitHub API Behavior

### Issues

The GitHub adapter should:

- list open issues
- ignore pull requests returned in issue listings
- normalize labels into workflow state
- update labels using issue-label APIs

### Pull Requests

The adapter should:

- create a PR against the configured base branch
- fetch PR details including mergeability and review outcome
- merge only when policy conditions are satisfied

### Completion

When a PR is merged and workflow policy says the issue is complete:

1. update workflow label to `done`
2. close the issue

This makes both the workflow state and the GitHub issue lifecycle visible to operators.

## Pilot Repository Integration

Pilot target:

- local path: `/Users/nantas-agent/projects/game-design-patterns`
- tracker ref: `nantas1/game-design-patterns`
- execution engine: `codex CLI`

Additional pilot requirements:

- create `WORKFLOW.md` in the target repository
- add a wrapper script such as `tools/run_symphony_agent.sh`
- configure runner to call that wrapper script

The wrapper script should:

- read `PROMPT`
- invoke `codex CLI`
- run `uv sync`
- run `uv run pytest tests -q`
- capture `branch_name`
- capture `commit_sha`
- print the JSON that `ProcessRunner` expects

## Testing Strategy

### Mapping Tests

Add tests for:

- label-to-state normalization
- conflicting state labels
- PR review status normalization
- merge status normalization

### Adapter Tests

Use `wiremock` to test:

- fetch issues
- fetch issue
- update issue labels
- add comment
- create PR
- read PR status
- merge PR
- close issue

### Integration Tests

Retain existing fake-tracker orchestrator tests where possible.

Add GitHub-focused runtime tests for:

- state progression through labels
- completion path that closes the issue after merge

### Live Smoke Tests

After implementation:

- run `reconcile-once` against the pilot GitHub repository
- validate one issue through dispatch
- validate one reviewed PR through merge and issue close

## Risks

### Label ambiguity

Multiple workflow-state labels can make issue state undecidable.

Mitigation:

- reject ambiguous issues for dispatch
- log explicit diagnostics

### PR mergeability timing

GitHub mergeability can be temporarily unknown after PR creation or update.

Mitigation:

- treat unknown mergeability as non-mergeable for the current reconcile pass
- retry on later passes

### Close semantics

If issue close is hidden behind state update logic, behavior becomes harder to reason about.

Mitigation:

- keep issue close as an explicit adapter action

### Runner push/auth

`codex CLI` may complete local changes without being able to push a branch.

Mitigation:

- verify GitHub auth and push capability during pilot setup
- keep runner diagnostics operator-visible

## Success Criteria

This design is successful when:

- the orchestrator no longer depends on GitCode-specific live integration
- a GitHub-labeled issue can be dispatched
- the runner can create a branch and PR
- a reviewed PR can be merged automatically
- the linked issue gets the `done` label and is closed
- the existing daemon and reconcile runtime remain intact

## Summary

The correct next step is not to rewrite the orchestrator. It is to preserve the working V1 core and replace the live integration layer:

- GitCode remains the current baseline
- GitHub becomes the live target platform
- labels become the workflow-state carrier
- `codex CLI` remains behind the current process-runner model

This keeps the architecture honest while making the first real integration materially easier.
