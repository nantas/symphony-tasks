mod files;
pub mod layout;

use crate::models::run_record::RunRecord;
use anyhow::Result;
use layout::StateLayout;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct StateStore {
    layout: StateLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryEntry {
    pub issue_id: String,
    pub identifier: String,
    pub attempt: u32,
    pub due_at: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrWatchEntry {
    pub issue_id: String,
    pub repo_id: String,
    pub pr_ref: String,
    pub status: String,
}

impl StateStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            layout: StateLayout::new(root),
        }
    }

    pub fn save_run_record(&self, record: &RunRecord) -> Result<()> {
        let path = self.layout.run_record_path(&record.repo_id, &record.issue_id);
        files::write_json_file(&path, record)
    }

    pub fn load_run_record(&self, repo_id: &str, issue_id: &str) -> Result<RunRecord> {
        let path = self.layout.run_record_path(repo_id, issue_id);
        files::read_json_file(&path)
    }

    pub fn save_retry_queue(&self, entries: &[RetryEntry]) -> Result<()> {
        files::write_json_file(&self.layout.retry_queue_path(), entries)
    }

    pub fn load_retry_queue(&self) -> Result<Vec<RetryEntry>> {
        files::read_json_file(&self.layout.retry_queue_path())
    }

    pub fn save_pr_watch_state(&self, entries: &[PrWatchEntry]) -> Result<()> {
        files::write_json_file(&self.layout.pr_watch_path(), entries)
    }

    pub fn load_pr_watch_state(&self) -> Result<Vec<PrWatchEntry>> {
        files::read_json_file(&self.layout.pr_watch_path())
    }
}
