#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryBackoffEntry {
    pub issue_id: String,
    pub due_at_epoch_ms: u64,
}
