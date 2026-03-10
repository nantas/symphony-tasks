use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct StateLayout {
    root: PathBuf,
}

impl StateLayout {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn runs_dir(&self) -> PathBuf {
        self.root.join("runs")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn run_record_path(&self, repo_id: &str, issue_id: &str) -> PathBuf {
        self.runs_dir()
            .join(repo_id)
            .join(format!("{issue_id}.json"))
    }

    pub fn retry_queue_path(&self) -> PathBuf {
        self.state_dir().join("retry_queue.json")
    }

    pub fn pr_watch_path(&self) -> PathBuf {
        self.state_dir().join("pr_watch.json")
    }
}
