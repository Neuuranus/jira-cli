use crate::api::{ApiError, JiraClient};
use crate::output::OutputConfig;

use super::issues::{issue_to_json, render_issue_table};

/// Run a raw JQL search and render the results.
///
/// The JQL string is passed verbatim to the Jira search API — no clauses or
/// ORDER BY are appended. Use `issues list` for a filtered view with automatic
/// ordering.
pub async fn run(
    client: &JiraClient,
    out: &OutputConfig,
    jql: &str,
    limit: usize,
    offset: usize,
) -> Result<(), ApiError> {
    let resp = client.search(jql, limit, offset).await?;

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "total": resp.total,
                "startAt": resp.start_at,
                "maxResults": resp.max_results,
                "issues": resp.issues.iter().map(|i| issue_to_json(i, client.host())).collect::<Vec<_>>(),
            }))
            .expect("failed to serialize JSON"),
        );
    } else {
        render_issue_table(&resp.issues, out);
        if resp.total > resp.start_at + resp.issues.len() {
            out.print_message(&format!(
                "Showing {}-{} of {} issues — use --limit or --offset to paginate",
                resp.start_at + 1,
                resp.start_at + resp.issues.len(),
                resp.total
            ));
        } else {
            out.print_message(&format!("{} issues", resp.issues.len()));
        }
    }
    Ok(())
}
