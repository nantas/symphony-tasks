pub mod config;
pub mod lock;

use crate::agent_runner::AgentRunner;
use crate::agent_runner::process::{ProcessRunner, ProcessRunnerConfig};
use crate::app::config::OrchestratorConfig;
use crate::models::repository::RepositoryProfile;
use crate::models::run_record::{RunRecord, RunStatus};
use crate::models::workflow::WorkflowDefinition;
use crate::orchestrator::reconcile::{
    DispatchRequest, PrLifecycleRequest, SelectionContext, WatchPrRequest, create_pr_for_run,
    dispatch_issue, reconcile_pr_watch, select_dispatch_candidates,
};
use crate::orchestrator::retry::RetryBackoffEntry;
use crate::registry::load::load_repository_profiles;
use crate::state_store::{PrWatchEntry, RetryEntry, StateStore};
use crate::tracker::Tracker;
use crate::tracker::gitcode::client::GitCodeClient;
use crate::tracker::github::client::GitHubClient;
use crate::workflow::parser::load_workflow_definition;
use crate::workspace::WorkspaceManager;
use anyhow::Result;
use anyhow::{Context, bail};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryState {
    pub retry_queue: Vec<RetryEntry>,
    pub pr_watch_entries: Vec<PrWatchEntry>,
    pub interrupted_issue_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReconcileSummary {
    pub dispatched_runs: usize,
    pub reconciled_prs: usize,
    pub retries_requeued: usize,
    pub skipped_due_to_backoff: usize,
    pub terminal_converged: usize,
}

pub fn recover_runtime_state(store: &StateStore) -> Result<RecoveryState> {
    let retry_queue = store.load_retry_queue_or_default()?;
    let pr_watch_entries = store.load_pr_watch_state_or_default()?;
    let interrupted_issue_ids = store
        .load_all_run_records()?
        .into_iter()
        .filter(is_interrupted_run)
        .map(|record| record.issue_id)
        .collect();

    Ok(RecoveryState {
        retry_queue,
        pr_watch_entries,
        interrupted_issue_ids,
    })
}

pub fn live_tracker_kind(config: &OrchestratorConfig) -> Result<&str> {
    match config.default_tracker_kind.as_str() {
        "github" => Ok("github"),
        "gitcode" => Ok("gitcode"),
        other => bail!("unsupported default_tracker_kind {other}"),
    }
}

fn is_interrupted_run(record: &RunRecord) -> bool {
    matches!(
        record.status,
        RunStatus::Claiming | RunStatus::PreparingWorkspace | RunStatus::RunningAgent
    )
}

pub async fn reconcile_once(config: &OrchestratorConfig) -> Result<ReconcileSummary> {
    let runner = build_process_runner(config)?;

    match live_tracker_kind(config)? {
        "github" => {
            let token = std::env::var(&config.github_token_env).with_context(|| {
                format!(
                    "missing required environment variable {}",
                    config.github_token_env
                )
            })?;
            let tracker = GitHubClient::new("https://api.github.com", token);
            reconcile_once_with(config, &tracker, &runner).await
        }
        "gitcode" => {
            let token = std::env::var(&config.github_token_env).with_context(|| {
                format!(
                    "missing required environment variable {}",
                    config.github_token_env
                )
            })?;
            let tracker = GitCodeClient::new("https://gitcode.com", token);
            reconcile_once_with(config, &tracker, &runner).await
        }
        _ => unreachable!(),
    }
}

pub async fn reconcile_once_with<T: Tracker + ?Sized, R: AgentRunner>(
    config: &OrchestratorConfig,
    tracker: &T,
    runner: &R,
) -> Result<ReconcileSummary> {
    let state_store = StateStore::new(state_store_root(&config.state_root));
    let workspace_manager = WorkspaceManager::new(&config.workspace_root);
    let profiles = load_repository_profiles(config)?;
    let workflows = load_workflows(&profiles)?;
    let profiles_by_id: HashMap<&str, &RepositoryProfile> = profiles
        .iter()
        .map(|profile| (profile.repo_id.as_str(), profile))
        .collect();
    let mut summary = ReconcileSummary::default();
    let now_ms = now_epoch_ms();

    for entry in state_store.load_pr_watch_state_or_default()? {
        let Some(repo) = profiles_by_id.get(entry.repo_id.as_str()).copied() else {
            continue;
        };
        let Some(workflow) = workflows.get(entry.repo_id.as_str()) else {
            continue;
        };
        let run_record = state_store.load_run_record(&entry.repo_id, &entry.issue_id)?;
        let issue = tracker.fetch_issue(repo, &entry.issue_id).await?;
        let previous_status = run_record.status.clone();
        let updated = reconcile_pr_watch(
            tracker,
            &state_store,
            WatchPrRequest {
                repo,
                issue: &issue,
                workflow,
                run_record,
                updated_at: &timestamp_now(),
            },
        )
        .await?;
        if !is_terminal_run_status(&previous_status) && is_terminal_run_status(&updated.status) {
            summary.terminal_converged += 1;
        }
        summary.reconciled_prs += 1;
    }

    let retry_queue = state_store.load_retry_queue_or_default()?;
    let (due_retries, pending_retries): (Vec<_>, Vec<_>) = retry_queue
        .iter()
        .partition(|entry| parse_retry_due_at(&entry.due_at) <= now_ms);
    summary.skipped_due_to_backoff = pending_retries.len();

    let mut run_records = state_store.load_all_run_records()?;
    let mut claimed_issue_ids = active_claimed_issue_ids(&run_records);
    claimed_issue_ids.extend(terminal_issue_ids(&run_records));
    let mut global_running = count_active_runs(&run_records, None);
    let retry_backoff: Vec<_> = pending_retries
        .iter()
        .map(|entry| RetryBackoffEntry {
            issue_id: entry.issue_id.clone(),
            due_at_epoch_ms: parse_retry_due_at(&entry.due_at),
        })
        .collect();
    let mut dispatched_retry_ids: HashSet<String> = HashSet::new();

    for repo in &profiles {
        ensure_process_runner(repo)?;
        let workflow = workflows
            .get(repo.repo_id.as_str())
            .context("missing workflow for enabled repository")?;
        let repo_running = count_active_runs(&run_records, Some(repo.repo_id.as_str()));
        let candidates = tracker.fetch_candidate_issues(repo).await?;
        let selected = select_dispatch_candidates(
            &candidates,
            repo,
            workflow,
            &SelectionContext {
                global_limit: config.global_concurrency,
                global_running,
                repo_running,
                claimed_issue_ids: claimed_issue_ids.clone(),
                retry_backoff: retry_backoff.clone(),
                now_epoch_ms: now_ms,
            },
        );

        for issue in selected {
            let started_at = timestamp_now();
            let dispatched = dispatch_issue(
                tracker,
                runner,
                &workspace_manager,
                &state_store,
                DispatchRequest {
                    repo,
                    issue: &issue,
                    workflow,
                    started_at: &started_at,
                },
            )
            .await?;

            claimed_issue_ids.insert(issue.id.clone());
            global_running += 1;
            summary.dispatched_runs += 1;

            if due_retries.iter().any(|r| r.issue_id == issue.id) {
                dispatched_retry_ids.insert(issue.id.clone());
            }

            if workflow.pr_policy.require_pr {
                let updated_at = timestamp_now();
                let updated = create_pr_for_run(
                    tracker,
                    &state_store,
                    PrLifecycleRequest {
                        repo,
                        issue: &issue,
                        workflow,
                        run_record: dispatched.run_record,
                        base_branch: "main",
                        updated_at: &updated_at,
                    },
                )
                .await?;
                update_run_record_cache(&mut run_records, updated);
            } else {
                update_run_record_cache(&mut run_records, dispatched.run_record);
            }
        }
    }

    summary.retries_requeued = dispatched_retry_ids.len();
    let remaining_retry: Vec<_> = retry_queue
        .into_iter()
        .filter(|entry| !dispatched_retry_ids.contains(&entry.issue_id))
        .collect();
    state_store.save_retry_queue(&remaining_retry)?;

    Ok(summary)
}

pub async fn run_daemon(config: &OrchestratorConfig, lock_path: impl AsRef<Path>) -> Result<()> {
    let _lock = lock::DaemonLock::acquire(lock_path)?;
    loop {
        reconcile_once(config).await?;
        tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal?;
                break;
            }
            _ = sleep(Duration::from_secs(config.poll_interval_secs)) => {}
        }
    }

    Ok(())
}

fn build_process_runner(config: &OrchestratorConfig) -> Result<ProcessRunner> {
    if config.default_runner != "process" {
        bail!("unsupported default_runner {}", config.default_runner);
    }

    let program = config
        .runner_program
        .clone()
        .filter(|value| !value.trim().is_empty())
        .context("runner_program must be set when default_runner is process")?;

    Ok(ProcessRunner::new(ProcessRunnerConfig {
        program,
        args: config.runner_args.clone(),
    }))
}

fn load_workflows(profiles: &[RepositoryProfile]) -> Result<HashMap<String, WorkflowDefinition>> {
    let mut workflows = HashMap::new();
    for profile in profiles {
        workflows.insert(
            profile.repo_id.clone(),
            load_workflow_definition(&profile.workflow_path)?,
        );
    }
    Ok(workflows)
}

fn ensure_process_runner(repo: &RepositoryProfile) -> Result<()> {
    if repo.default_runner != "process" {
        bail!(
            "repository {} uses unsupported runner {}",
            repo.repo_id,
            repo.default_runner
        );
    }
    Ok(())
}

fn state_store_root(state_root: &Path) -> PathBuf {
    state_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn timestamp_now() -> String {
    format!("{}", now_epoch_ms())
}

fn parse_retry_due_at(due_at: &str) -> u64 {
    due_at.parse::<u64>().unwrap_or(0)
}

fn count_active_runs(records: &[RunRecord], repo_id: Option<&str>) -> usize {
    records
        .iter()
        .filter(|record| repo_id.is_none_or(|value| record.repo_id == value))
        .filter(|record| is_active_run_status(&record.status))
        .count()
}

fn active_claimed_issue_ids(records: &[RunRecord]) -> HashSet<String> {
    records
        .iter()
        .filter(|record| is_active_run_status(&record.status))
        .map(|record| record.issue_id.clone())
        .collect()
}

fn terminal_issue_ids(records: &[RunRecord]) -> HashSet<String> {
    records
        .iter()
        .filter(|record| is_terminal_run_status(&record.status))
        .map(|record| record.issue_id.clone())
        .collect()
}

fn is_active_run_status(status: &RunStatus) -> bool {
    !matches!(status, RunStatus::Completed | RunStatus::Failed)
}

fn is_terminal_run_status(status: &RunStatus) -> bool {
    matches!(status, RunStatus::Completed | RunStatus::Failed)
}

fn update_run_record_cache(run_records: &mut Vec<RunRecord>, run_record: RunRecord) {
    match run_records.iter_mut().find(|existing| {
        existing.repo_id == run_record.repo_id && existing.issue_id == run_record.issue_id
    }) {
        Some(existing) => *existing = run_record,
        None => run_records.push(run_record),
    }
}
