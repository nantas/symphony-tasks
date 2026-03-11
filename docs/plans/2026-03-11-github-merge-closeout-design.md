# GitHub Merge Closeout Design

**Problem:** The current GitHub tracker flow only completes when the orchestrator itself merges the PR after approval. That blocks real usage when a human merges the PR directly in GitHub.

**Decision:** Support two completion paths with one shared closeout routine.

1. Auto-merge path
   - PR is approved and mergeable.
   - The orchestrator calls `merge_pr`.
   - After merge succeeds, the orchestrator marks the issue `Done`, closes it, marks the run `Completed`, and removes the PR watch entry.

2. Manual-merge path
   - A human merges the PR in GitHub before the orchestrator does.
   - On the next reconciliation pass, the orchestrator sees the PR is already merged.
   - It skips `merge_pr` and runs the same closeout routine: `Done`, `close_issue`, `Completed`, remove PR watch entry.

**Workflow contract:** Keep `completion_policy.close_issue_on_merge = true`. Its meaning becomes "when the system observes that the PR has been merged, finalize the linked issue." No new workflow flag is needed yet. The pilot repository `WORKFLOW.md` should be updated to document that PRs may be merged either by the orchestrator after approval or manually by a user.

**State model:** `AwaitingHumanReview` remains the watch state for open PRs. Completion is keyed off the observed PR merge result, not off who performed the merge.

**Code impact:**
- `src/orchestrator/reconcile.rs`
  - Treat `MergeStatus::Merged` as a completed PR that should be finalized without calling `merge_pr`.
- `tests/pr_reconcile.rs`
  - Add a regression test proving externally merged PRs complete without a merge call.
- `tests/e2e_fixture.rs`
  - Cover the end-to-end closeout path for an already-merged PR.
- `tests/github_mapping.rs` and `tests/github_client.rs`
  - Lock in GitHub merged-PR mapping and status fetch expectations.
- `/Users/nantas-agent/projects/game-design-patterns/WORKFLOW.md`
  - Document the dual closeout behavior for operators.

**Live smoke acceptance:** Either of these outcomes is valid:

1. The PR is approved, the orchestrator merges it, and the issue is closed on the next reconciliation pass.
2. The user merges the PR manually, and the orchestrator closes the issue on the next reconciliation pass.

Both paths must end with:
- PR watch entry removed
- run record `Completed`
- issue labeled `done`
- issue state `closed`
