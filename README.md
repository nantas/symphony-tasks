# Symphony Tasks

Independent Symphony-style orchestrator for GitHub Issues and GitHub PR workflows.

## Current Scope

- Rust single-binary orchestrator
- GitHub tracker and PR adapter
- local-file runtime state under `var/`
- `daemon`, `reconcile-once`, and `validate-config` CLI entrypoints
- per-issue workspaces and process-backed agent runner

## Configuration

Global orchestrator settings live in `config/orchestrator.toml`.

Repository registrations live in `config/repositories/*.toml`.

Each target repository must provide its own `WORKFLOW.md`.

Required environment variables:

- `GITHUB_TOKEN`

Live tracker selection is configured in `config/orchestrator.toml` with:

- `default_tracker_kind = "github"`
- `github_token_env = "GITHUB_TOKEN"`

Repository registrations use tracker-neutral fields:

- `tracker_kind = "github"`
- `tracker_project_ref = "owner/repo"`

Example repository config:

```toml
repo_id = "demo"
repo_path = "/absolute/path/to/repo"
workflow_path = "/absolute/path/to/repo/WORKFLOW.md"
tracker_kind = "github"
tracker_project_ref = "owner/repo"
default_runner = "process"
enabled = true
max_concurrent_runs = 1
```

Runner configuration lives in `config/orchestrator.toml`:

- `runner_program`: executable used by the built-in `process` runner
- `runner_args`: argument array passed to that executable

## Commands

Validate configuration:

```bash
cargo run -- --config config/orchestrator.toml validate-config
```

Run one coordination pass:

```bash
cargo run -- --config config/orchestrator.toml reconcile-once
```

Safe no-op smoke pass:

```bash
GITHUB_TOKEN=placeholder cargo run -- --config config/orchestrator.smoke.toml reconcile-once
```

`config/orchestrator.smoke.toml` points at `config/repositories-smoke/`, which only ships a sample registration with `enabled = false`, so `reconcile-once` exits without loading a live repository or dispatching work. The placeholder token is still required because config validation rejects an empty `GITHUB_TOKEN`.

Run as a daemon:

```bash
cargo run -- --config config/orchestrator.toml daemon
```

Run tests:

```bash
cargo test
```

## Development Notes

- Workspaces are created under `var/workspaces/<repo-id>/<issue-key>/`
- Run records are persisted under `var/runs/<repo-id>/<issue-id>.json`
- PR watch state is persisted under `var/state/pr_watch.json`
- Retry queue is persisted under `var/state/retry_queue.json`
