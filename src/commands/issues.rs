use owo_colors::OwoColorize;

use crate::api::{ApiError, Issue, JiraClient, escape_jql};
use crate::output::{OutputConfig, use_color};

#[allow(clippy::too_many_arguments)]
pub async fn list(
    client: &JiraClient,
    out: &OutputConfig,
    project: Option<&str>,
    status: Option<&str>,
    assignee: Option<&str>,
    sprint: Option<&str>,
    jql_extra: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<(), ApiError> {
    let jql = build_list_jql(project, status, assignee, sprint, jql_extra);
    let resp = client.search(&jql, limit, offset).await?;

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "total": resp.total,
                "startAt": resp.start_at,
                "maxResults": resp.max_results,
                "issues": resp.issues.iter().map(|i| issue_to_json(i, client)).collect::<Vec<_>>(),
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

pub async fn show(
    client: &JiraClient,
    out: &OutputConfig,
    key: &str,
    open: bool,
) -> Result<(), ApiError> {
    let issue = client.get_issue(key).await?;

    if open {
        open_in_browser(&client.browse_url(&issue.key));
    }

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&issue_detail_to_json(&issue, client))
                .expect("failed to serialize JSON"),
        );
    } else {
        render_issue_detail(&issue);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    client: &JiraClient,
    out: &OutputConfig,
    project: &str,
    issue_type: &str,
    summary: &str,
    description: Option<&str>,
    priority: Option<&str>,
    labels: Option<&[&str]>,
    assignee: Option<&str>,
) -> Result<(), ApiError> {
    let resp = client
        .create_issue(
            project,
            issue_type,
            summary,
            description,
            priority,
            labels,
            assignee,
        )
        .await?;
    let url = client.browse_url(&resp.key);
    out.print_result(
        &serde_json::json!({ "key": resp.key, "id": resp.id, "url": url }),
        &resp.key,
    );
    Ok(())
}

pub async fn update(
    client: &JiraClient,
    out: &OutputConfig,
    key: &str,
    summary: Option<&str>,
    description: Option<&str>,
    priority: Option<&str>,
) -> Result<(), ApiError> {
    client
        .update_issue(key, summary, description, priority)
        .await?;
    out.print_result(
        &serde_json::json!({ "key": key, "updated": true }),
        &format!("Updated {key}"),
    );
    Ok(())
}

pub async fn comment(
    client: &JiraClient,
    out: &OutputConfig,
    key: &str,
    body: &str,
) -> Result<(), ApiError> {
    let c = client.add_comment(key, body).await?;
    let url = client.browse_url(key);
    out.print_result(
        &serde_json::json!({
            "id": c.id,
            "issue": key,
            "url": url,
            "author": c.author.display_name,
            "created": c.created,
        }),
        &format!("Comment added to {key}"),
    );
    Ok(())
}

pub async fn transition(
    client: &JiraClient,
    out: &OutputConfig,
    key: &str,
    to: &str,
) -> Result<(), ApiError> {
    let transitions = client.get_transitions(key).await?;

    let matched = transitions
        .iter()
        .find(|t| t.name.to_lowercase() == to.to_lowercase() || t.id == to);

    match matched {
        Some(t) => {
            let name = t.name.clone();
            let id = t.id.clone();
            client.do_transition(key, &id).await?;
            out.print_result(
                &serde_json::json!({ "issue": key, "transition": name, "id": id }),
                &format!("Transitioned {key} → {name}"),
            );
        }
        None => {
            let hint = transitions
                .iter()
                .map(|t| format!("  {} ({})", t.name, t.id))
                .collect::<Vec<_>>()
                .join("\n");
            out.print_message(&format!(
                "Transition '{to}' not found for {key}. Available:\n{hint}"
            ));
            out.print_message(&format!(
                "Tip: `jira issues list-transitions {key}` shows transitions as JSON."
            ));
            return Err(ApiError::NotFound(format!(
                "Transition '{to}' not found for {key}"
            )));
        }
    }
    Ok(())
}

pub async fn list_transitions(
    client: &JiraClient,
    out: &OutputConfig,
    key: &str,
) -> Result<(), ApiError> {
    let ts = client.get_transitions(key).await?;

    if out.json {
        out.print_data(&serde_json::to_string_pretty(&ts).expect("failed to serialize JSON"));
    } else {
        let color = use_color();
        let header = format!("{:<6} {}", "ID", "Name");
        if color {
            println!("{}", header.bold());
        } else {
            println!("{header}");
        }
        for t in &ts {
            println!("{:<6} {}", t.id, t.name);
        }
    }
    Ok(())
}

pub async fn assign(
    client: &JiraClient,
    out: &OutputConfig,
    key: &str,
    assignee: &str,
) -> Result<(), ApiError> {
    let account_id = if assignee == "me" {
        let me = client.get_myself().await?;
        me.account_id
    } else if assignee == "none" || assignee == "unassign" {
        client.assign_issue(key, None).await?;
        out.print_result(
            &serde_json::json!({ "issue": key, "assignee": null }),
            &format!("Unassigned {key}"),
        );
        return Ok(());
    } else {
        assignee.to_string()
    };

    client.assign_issue(key, Some(&account_id)).await?;
    out.print_result(
        &serde_json::json!({ "issue": key, "accountId": account_id }),
        &format!("Assigned {key} to {assignee}"),
    );
    Ok(())
}

// ── Rendering ─────────────────────────────────────────────────────────────────

pub(crate) fn render_issue_table(issues: &[Issue], out: &OutputConfig) {
    if issues.is_empty() {
        out.print_message("No issues found.");
        return;
    }

    let color = use_color();
    let term_width = terminal_width();

    let key_w = issues.iter().map(|i| i.key.len()).max().unwrap_or(4).max(4) + 1;
    let status_w = issues
        .iter()
        .map(|i| i.status().len())
        .max()
        .unwrap_or(6)
        .clamp(6, 14)
        + 2;
    let assignee_w = issues
        .iter()
        .map(|i| i.assignee().len())
        .max()
        .unwrap_or(8)
        .clamp(8, 18)
        + 2;
    let type_w = issues
        .iter()
        .map(|i| i.issue_type().len())
        .max()
        .unwrap_or(4)
        .clamp(4, 12)
        + 2;

    // Give remaining width to summary, minimum 20
    let fixed = key_w + 1 + status_w + 1 + assignee_w + 1 + type_w + 1;
    let summary_w = term_width.saturating_sub(fixed).max(20);

    let header = format!(
        "{:<key_w$} {:<status_w$} {:<assignee_w$} {:<type_w$} {}",
        "Key", "Status", "Assignee", "Type", "Summary"
    );
    if color {
        println!("{}", header.bold());
    } else {
        println!("{header}");
    }

    for issue in issues {
        let key = if color {
            format!("{:<key_w$}", issue.key).yellow().to_string()
        } else {
            format!("{:<key_w$}", issue.key)
        };
        let status_val = truncate(issue.status(), status_w - 2);
        let status = if color {
            colorize_status(issue.status(), &format!("{:<status_w$}", status_val))
        } else {
            format!("{:<status_w$}", status_val)
        };
        println!(
            "{key} {status} {:<assignee_w$} {:<type_w$} {}",
            truncate(issue.assignee(), assignee_w - 2),
            truncate(issue.issue_type(), type_w - 2),
            truncate(issue.summary(), summary_w),
        );
    }
}

fn render_issue_detail(issue: &Issue) {
    let color = use_color();
    let key = if color {
        issue.key.yellow().bold().to_string()
    } else {
        issue.key.clone()
    };
    println!("{key}  {}", issue.summary());
    println!();
    println!("  Type:     {}", issue.issue_type());
    let status_str = if color {
        colorize_status(issue.status(), issue.status())
    } else {
        issue.status().to_string()
    };
    println!("  Status:   {status_str}");
    println!("  Priority: {}", issue.priority());
    println!("  Assignee: {}", issue.assignee());
    if let Some(ref reporter) = issue.fields.reporter {
        println!("  Reporter: {}", reporter.display_name);
    }
    if let Some(ref labels) = issue.fields.labels
        && !labels.is_empty()
    {
        println!("  Labels:   {}", labels.join(", "));
    }
    if let Some(ref created) = issue.fields.created {
        println!("  Created:  {}", format_date(created));
    }
    if let Some(ref updated) = issue.fields.updated {
        println!("  Updated:  {}", format_date(updated));
    }

    let desc = issue.description_text();
    if !desc.is_empty() {
        println!();
        println!("Description:");
        for line in desc.lines() {
            println!("  {line}");
        }
    }

    if let Some(ref comment_list) = issue.fields.comment
        && !comment_list.comments.is_empty()
    {
        println!();
        println!("Comments ({}):", comment_list.total);
        for c in &comment_list.comments {
            println!();
            let author = if color {
                c.author.display_name.bold().to_string()
            } else {
                c.author.display_name.clone()
            };
            println!("  {} — {}", author, format_date(&c.created));
            let body = c.body_text();
            for line in body.lines() {
                println!("    {line}");
            }
        }
    }
}

// ── JSON serialization ────────────────────────────────────────────────────────

pub(crate) fn issue_to_json(issue: &Issue, client: &JiraClient) -> serde_json::Value {
    serde_json::json!({
        "key": issue.key,
        "id": issue.id,
        "url": client.browse_url(&issue.key),
        "summary": issue.summary(),
        "status": issue.status(),
        "assignee": {
            "displayName": issue.assignee(),
            "accountId": issue.fields.assignee.as_ref().and_then(|a| a.account_id.as_deref()),
        },
        "priority": issue.priority(),
        "type": issue.issue_type(),
        "created": issue.fields.created,
        "updated": issue.fields.updated,
    })
}

fn issue_detail_to_json(issue: &Issue, client: &JiraClient) -> serde_json::Value {
    let comments: Vec<serde_json::Value> = issue
        .fields
        .comment
        .as_ref()
        .map(|cl| {
            cl.comments
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "id": c.id,
                        "author": {
                            "displayName": c.author.display_name,
                            "accountId": c.author.account_id,
                        },
                        "body": c.body_text(),
                        "created": c.created,
                        "updated": c.updated,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    serde_json::json!({
        "key": issue.key,
        "id": issue.id,
        "url": client.browse_url(&issue.key),
        "summary": issue.summary(),
        "status": issue.status(),
        "type": issue.issue_type(),
        "priority": issue.priority(),
        "assignee": {
            "displayName": issue.assignee(),
            "accountId": issue.fields.assignee.as_ref().and_then(|a| a.account_id.as_deref()),
        },
        "reporter": issue.fields.reporter.as_ref().map(|r| serde_json::json!({
            "displayName": r.display_name,
            "accountId": r.account_id,
        })),
        "labels": issue.fields.labels,
        "description": issue.description_text(),
        "created": issue.fields.created,
        "updated": issue.fields.updated,
        "comments": comments,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_list_jql(
    project: Option<&str>,
    status: Option<&str>,
    assignee: Option<&str>,
    sprint: Option<&str>,
    extra: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(p) = project {
        parts.push(format!(r#"project = "{}""#, escape_jql(p)));
    }
    if let Some(s) = status {
        parts.push(format!(r#"status = "{}""#, escape_jql(s)));
    }
    if let Some(a) = assignee {
        if a == "me" {
            parts.push("assignee = currentUser()".into());
        } else {
            parts.push(format!(r#"assignee = "{}""#, escape_jql(a)));
        }
    }
    if let Some(s) = sprint {
        if s == "active" || s == "open" {
            parts.push("sprint in openSprints()".into());
        } else {
            parts.push(format!(r#"sprint = "{}""#, escape_jql(s)));
        }
    }
    if let Some(e) = extra {
        parts.push(format!("({e})"));
    }

    if parts.is_empty() {
        "ORDER BY updated DESC".into()
    } else {
        format!("{} ORDER BY updated DESC", parts.join(" AND "))
    }
}

/// Color-code a Jira status string for terminal output.
fn colorize_status(status: &str, display: &str) -> String {
    let lower = status.to_lowercase();
    if lower.contains("done") || lower.contains("closed") || lower.contains("resolved") {
        display.green().to_string()
    } else if lower.contains("progress") || lower.contains("review") || lower.contains("testing") {
        display.yellow().to_string()
    } else if lower.contains("blocked") || lower.contains("impediment") {
        display.red().to_string()
    } else {
        display.to_string()
    }
}

/// Open a URL in the system default browser, printing a warning if it fails.
fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).status();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(url).status();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/c", "start", url])
        .status();

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    if let Err(e) = result {
        eprintln!("Warning: could not open browser: {e}");
    }
}

/// Truncate a string to `max` characters (not bytes), appending `…` if cut.
fn truncate(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let mut result: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}

/// Shorten an ISO-8601 timestamp to just the date portion.
fn format_date(s: &str) -> String {
    s.chars().take(10).collect()
}

/// Get the terminal width from the COLUMNS env var, defaulting to 120.
fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_string() {
        assert_eq!(truncate("hello world", 5), "hello…");
    }

    #[test]
    fn truncate_multibyte_safe() {
        let result = truncate("日本語テスト", 3);
        assert_eq!(result, "日本語…");
    }

    #[test]
    fn build_list_jql_empty() {
        assert_eq!(
            build_list_jql(None, None, None, None, None),
            "ORDER BY updated DESC"
        );
    }

    #[test]
    fn build_list_jql_escapes_quotes() {
        let jql = build_list_jql(None, Some(r#"Done" OR 1=1"#), None, None, None);
        // The double quote must be backslash-escaped so it cannot break out of the JQL string.
        // The resulting clause should be:  status = "Done\" OR 1=1"
        assert!(jql.contains(r#"\""#), "double quote must be escaped");
        assert!(
            jql.contains(r#"status = "Done\""#),
            "escaped quote must remain inside the status value string"
        );
    }

    #[test]
    fn build_list_jql_project_and_status() {
        let jql = build_list_jql(Some("PROJ"), Some("In Progress"), None, None, None);
        assert!(jql.contains(r#"project = "PROJ""#));
        assert!(jql.contains(r#"status = "In Progress""#));
    }

    #[test]
    fn build_list_jql_assignee_me() {
        let jql = build_list_jql(None, None, Some("me"), None, None);
        assert!(jql.contains("currentUser()"));
    }

    #[test]
    fn build_list_jql_sprint_active() {
        let jql = build_list_jql(None, None, None, Some("active"), None);
        assert!(jql.contains("sprint in openSprints()"));
    }

    #[test]
    fn build_list_jql_sprint_named() {
        let jql = build_list_jql(None, None, None, Some("Sprint 42"), None);
        assert!(jql.contains(r#"sprint = "Sprint 42""#));
    }

    #[test]
    fn colorize_status_done_is_green() {
        let result = colorize_status("Done", "Done");
        assert!(result.contains("Done"));
        // Green ANSI escape code starts with \x1b[32m
        assert!(result.contains("\x1b["));
    }

    #[test]
    fn colorize_status_unknown_unchanged() {
        let result = colorize_status("Backlog", "Backlog");
        assert_eq!(result, "Backlog");
    }

    /// Ensures an environment variable is removed even if the test panics.
    struct EnvVarGuard(&'static str);

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe { std::env::remove_var(self.0) }
        }
    }

    #[test]
    fn terminal_width_fallback_parses_columns() {
        unsafe { std::env::set_var("COLUMNS", "200") };
        let _guard = EnvVarGuard("COLUMNS");
        assert_eq!(terminal_width(), 200);
    }
}
