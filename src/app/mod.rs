pub mod lock;
pub mod config;

use crate::models::run_record::{RunRecord, RunStatus};
use crate::state_store::{PrWatchEntry, RetryEntry, StateStore};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryState {
    pub retry_queue: Vec<RetryEntry>,
    pub pr_watch_entries: Vec<PrWatchEntry>,
    pub interrupted_issue_ids: Vec<String>,
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

fn is_interrupted_run(record: &RunRecord) -> bool {
    matches!(
        record.status,
        RunStatus::Claiming | RunStatus::PreparingWorkspace | RunStatus::RunningAgent
    )
}
