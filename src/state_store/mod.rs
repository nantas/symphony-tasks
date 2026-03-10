mod files;
pub mod layout;

use crate::models::run_record::RunRecord;
use anyhow::Result;
use layout::StateLayout;
use serde::{Deserialize, Serialize};
use std::fs;
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
        let path = self
            .layout
            .run_record_path(&record.repo_id, &record.issue_id);
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

    pub fn load_retry_queue_or_default(&self) -> Result<Vec<RetryEntry>> {
        let path = self.layout.retry_queue_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        self.load_retry_queue()
    }

    pub fn save_pr_watch_state(&self, entries: &[PrWatchEntry]) -> Result<()> {
        files::write_json_file(&self.layout.pr_watch_path(), entries)
    }

    pub fn load_pr_watch_state(&self) -> Result<Vec<PrWatchEntry>> {
        files::read_json_file(&self.layout.pr_watch_path())
    }

    pub fn load_pr_watch_state_or_default(&self) -> Result<Vec<PrWatchEntry>> {
        let path = self.layout.pr_watch_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        self.load_pr_watch_state()
    }

    pub fn upsert_pr_watch_entry(&self, entry: PrWatchEntry) -> Result<()> {
        let mut entries = self.load_pr_watch_state_or_default()?;
        match entries.iter_mut().find(|existing| {
            existing.repo_id == entry.repo_id && existing.issue_id == entry.issue_id
        }) {
            Some(existing) => *existing = entry,
            None => entries.push(entry),
        }
        sort_pr_watch_entries(&mut entries);
        self.save_pr_watch_state(&entries)
    }

    pub fn remove_pr_watch_entry(&self, repo_id: &str, issue_id: &str) -> Result<()> {
        let mut entries = self.load_pr_watch_state_or_default()?;
        entries.retain(|entry| !(entry.repo_id == repo_id && entry.issue_id == issue_id));
        sort_pr_watch_entries(&mut entries);
        self.save_pr_watch_state(&entries)
    }

    pub fn load_all_run_records(&self) -> Result<Vec<RunRecord>> {
        let runs_dir = self.layout.runs_dir();
        if !runs_dir.exists() {
            return Ok(Vec::new());
        }

        let mut records: Vec<RunRecord> = Vec::new();
        for repo_dir in fs::read_dir(&runs_dir)? {
            let repo_dir = repo_dir?;
            if !repo_dir.path().is_dir() {
                continue;
            }
            for entry in fs::read_dir(repo_dir.path())? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                records.push(files::read_json_file(&path)?);
            }
        }

        records.sort_by(|a, b| a.issue_id.cmp(&b.issue_id));
        Ok(records)
    }
}

fn sort_pr_watch_entries(entries: &mut [PrWatchEntry]) {
    entries.sort_by(|left, right| {
        left.repo_id
            .cmp(&right.repo_id)
            .then_with(|| left.issue_id.cmp(&right.issue_id))
            .then_with(|| left.pr_ref.cmp(&right.pr_ref))
    });
}
