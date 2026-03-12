use tracing_subscriber::EnvFilter;

pub fn format_reconcile_summary_event(
    dispatched_runs: usize,
    reconciled_prs: usize,
    retries_requeued: usize,
    skipped_due_to_backoff: usize,
    terminal_converged: usize,
) -> String {
    format!(
        "event=reconcile_summary dispatched_runs={dispatched_runs} reconciled_prs={reconciled_prs} retries_requeued={retries_requeued} skipped_due_to_backoff={skipped_due_to_backoff} terminal_converged={terminal_converged}"
    )
}

pub fn format_issue_event(event: &str, repo_id: &str, issue_id: &str) -> String {
    format!("event={event} repo_id={repo_id} issue_id={issue_id}")
}

pub fn log_reconcile_summary(
    dispatched_runs: usize,
    reconciled_prs: usize,
    retries_requeued: usize,
    skipped_due_to_backoff: usize,
    terminal_converged: usize,
) {
    tracing::info!(
        "{}",
        format_reconcile_summary_event(
            dispatched_runs,
            reconciled_prs,
            retries_requeued,
            skipped_due_to_backoff,
            terminal_converged,
        )
    );
}

pub fn log_issue_event(event: &str, repo_id: &str, issue_id: &str) {
    tracing::info!("{}", format_issue_event(event, repo_id, issue_id));
}

pub fn init_logging(level: &str, json: bool) {
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);

    if json {
        let _ = builder.json().try_init();
    } else {
        let _ = builder.try_init();
    }
}
