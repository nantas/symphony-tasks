# Symphony Tasks

Independent Symphony-style orchestrator for GitCode Issues and GitCode PR workflows.

## Current Scope

- Rust single-binary orchestrator
- GitCode tracker and PR adapter
- local-file runtime state under `var/`
- `daemon`, `reconcile-once`, and `validate-config` CLI entrypoints
- per-issue workspaces and process-backed agent runner

## Configuration

Global orchestrator settings live in `config/orchestrator.toml`.

Repository registrations live in `config/repositories/*.toml`.

Each target repository must provide its own `WORKFLOW.md`.

Required environment variables:

- `GITCODE_TOKEN`

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
