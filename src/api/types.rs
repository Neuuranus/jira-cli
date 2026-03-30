use serde::{Deserialize, Serialize};

/// Jira issue as returned by the search and issue endpoints.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Issue {
    pub id: String,
    pub key: String,
    #[serde(rename = "self")]
    pub url: Option<String>,
    pub fields: IssueFields,
}

impl Issue {
    pub fn summary(&self) -> &str {
        &self.fields.summary
    }

    pub fn status(&self) -> &str {
        &self.fields.status.name
    }

    pub fn assignee(&self) -> &str {
        self.fields
            .assignee
            .as_ref()
            .map(|a| a.display_name.as_str())
            .unwrap_or("-")
    }

    pub fn priority(&self) -> &str {
        self.fields
            .priority
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("-")
    }

    pub fn issue_type(&self) -> &str {
        &self.fields.issuetype.name
    }

    /// Extract plain text from the Atlassian Document Format description.
    pub fn description_text(&self) -> String {
        match &self.fields.description {
            Some(doc) => extract_adf_text(doc),
            None => String::new(),
        }
    }

    /// Construct the browser URL from the site base URL.
    pub fn browser_url(&self, site_url: &str) -> String {
        format!("{site_url}/browse/{}", self.key)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IssueFields {
    pub summary: String,
    pub status: StatusField,
    pub assignee: Option<UserField>,
    pub reporter: Option<UserField>,
    pub priority: Option<PriorityField>,
    pub issuetype: IssueTypeField,
    pub description: Option<serde_json::Value>,
    pub labels: Option<Vec<String>>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub comment: Option<CommentList>,
    #[serde(rename = "issuelinks")]
    pub issue_links: Option<Vec<IssueLink>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StatusField {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserField {
    pub display_name: String,
    pub email_address: Option<String>,
    /// Cloud: `accountId`. DC/Server: `name` (username).
    #[serde(alias = "name")]
    pub account_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PriorityField {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IssueTypeField {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommentList {
    pub comments: Vec<Comment>,
    pub total: usize,
    #[serde(default)]
    pub start_at: usize,
    #[serde(default)]
    pub max_results: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub author: UserField,
    pub body: Option<serde_json::Value>,
    pub created: String,
    pub updated: Option<String>,
}

impl Comment {
    pub fn body_text(&self) -> String {
        match &self.body {
            Some(doc) => extract_adf_text(doc),
            None => String::new(),
        }
    }
}

/// A Jira user returned from the user search endpoint.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    /// Cloud: `accountId`. DC/Server: `name` (username).
    #[serde(alias = "name")]
    pub account_id: String,
    pub display_name: String,
    pub email_address: Option<String>,
}

/// An issue link (relationship between two issues).
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IssueLink {
    pub id: String,
    #[serde(rename = "type")]
    pub link_type: IssueLinkType,
    pub outward_issue: Option<LinkedIssue>,
    pub inward_issue: Option<LinkedIssue>,
}

/// The type of an issue link (e.g. "Blocks", "Duplicate").
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IssueLinkType {
    pub id: String,
    pub name: String,
    pub inward: String,
    pub outward: String,
}

/// A summary view of an issue referenced in a link.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LinkedIssue {
    pub key: String,
    pub fields: LinkedIssueFields,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LinkedIssueFields {
    pub summary: String,
    pub status: StatusField,
}

/// A Jira Agile board.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type")]
    pub board_type: String,
}

/// Paginated board response from the Agile API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardSearchResponse {
    pub values: Vec<Board>,
    pub is_last: bool,
    #[serde(default)]
    pub start_at: usize,
    pub total: usize,
}

/// A Jira sprint.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Sprint {
    pub id: u64,
    pub name: String,
    pub state: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub complete_date: Option<String>,
    pub origin_board_id: Option<u64>,
}

/// Paginated sprint response from the Agile API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SprintSearchResponse {
    pub values: Vec<Sprint>,
    pub is_last: bool,
    #[serde(default)]
    pub start_at: usize,
}

/// A Jira field (system or custom).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Field {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub custom: bool,
    pub schema: Option<FieldSchema>,
}

/// The schema of a field, describing its type.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FieldSchema {
    #[serde(rename = "type")]
    pub field_type: String,
    pub items: Option<String>,
    pub system: Option<String>,
    pub custom: Option<String>,
}

/// Jira project.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Project {
    pub id: String,
    pub key: String,
    pub name: String,
    #[serde(rename = "projectTypeKey")]
    pub project_type: Option<String>,
}

/// Response from the paginated project search endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSearchResponse {
    pub values: Vec<Project>,
    pub total: usize,
    #[serde(default)]
    pub start_at: usize,
    #[serde(default)]
    pub max_results: usize,
    pub is_last: bool,
}

/// A single issue transition (workflow action).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Transition {
    pub id: String,
    pub name: String,
    /// The status this transition leads to, including its workflow category.
    pub to: Option<TransitionTo>,
}

/// The target status of a transition.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransitionTo {
    pub name: String,
    pub status_category: Option<StatusCategory>,
}

/// Workflow category for a status (e.g. "new", "indeterminate", "done").
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StatusCategory {
    pub key: String,
    pub name: String,
}

/// Response from the Jira search endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct SearchResponse {
    pub issues: Vec<Issue>,
    pub total: usize,
    #[serde(rename = "startAt")]
    pub start_at: usize,
    #[serde(rename = "maxResults")]
    pub max_results: usize,
}

/// Response from the transitions endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct TransitionsResponse {
    pub transitions: Vec<Transition>,
}

/// Response from creating an issue.
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateIssueResponse {
    pub id: String,
    pub key: String,
    #[serde(rename = "self")]
    pub url: String,
}

/// Current authenticated user.
///
/// Jira Cloud (API v3) identifies users by `accountId`.
/// Jira Data Center / Server (API v2) identifies users by `name` (username).
/// Both forms deserialize into `account_id` so callers can use it uniformly.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Myself {
    /// Cloud: `accountId`. DC/Server: `name` (username).
    #[serde(alias = "name")]
    pub account_id: String,
    pub display_name: String,
}

/// Build an Atlassian Document Format document from plain text.
///
/// Each newline-separated line becomes a separate ADF paragraph node.
/// Blank lines produce empty paragraphs (no content array items), which is the
/// correct ADF representation accepted by Jira Cloud.
pub fn text_to_adf(text: &str) -> serde_json::Value {
    let paragraphs: Vec<serde_json::Value> = text
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                serde_json::json!({ "type": "paragraph", "content": [] })
            } else {
                serde_json::json!({
                    "type": "paragraph",
                    "content": [{"type": "text", "text": line}]
                })
            }
        })
        .collect();

    serde_json::json!({
        "type": "doc",
        "version": 1,
        "content": paragraphs
    })
}

/// Extract plain text from an ADF node or a plain string value.
///
/// API v2 (Jira Data Center / Server) returns descriptions and comment bodies
/// as plain JSON strings. API v3 (Jira Cloud) uses Atlassian Document Format.
/// Both forms are handled here so the same display path works for both versions.
pub fn extract_adf_text(node: &serde_json::Value) -> String {
    if let Some(s) = node.as_str() {
        return s.to_string();
    }
    let mut buf = String::new();
    collect_text(node, &mut buf);
    buf.trim().to_string()
}

fn collect_text(node: &serde_json::Value, buf: &mut String) {
    let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("");

    if node_type == "text" {
        if let Some(text) = node.get("text").and_then(|v| v.as_str()) {
            buf.push_str(text);
        }
        return;
    }

    if node_type == "hardBreak" {
        buf.push('\n');
        return;
    }

    if let Some(content) = node.get("content").and_then(|v| v.as_array()) {
        for child in content {
            collect_text(child, buf);
        }
    }

    // Block-level nodes get a trailing newline
    if matches!(
        node_type,
        "paragraph"
            | "heading"
            | "bulletList"
            | "orderedList"
            | "listItem"
            | "codeBlock"
            | "blockquote"
            | "rule"
    ) && !buf.ends_with('\n')
    {
        buf.push('\n');
    }
}

/// Escape a value for use inside a JQL double-quoted string literal.
///
/// JQL escapes double quotes as `\"` inside a quoted string.
pub fn escape_jql(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_simple_paragraph() {
        let doc = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Hello world"}]}]
        });
        assert_eq!(extract_adf_text(&doc), "Hello world");
    }

    #[test]
    fn extract_multiple_paragraphs() {
        let doc = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "First"}]},
                {"type": "paragraph", "content": [{"type": "text", "text": "Second"}]}
            ]
        });
        let text = extract_adf_text(&doc);
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
    }

    #[test]
    fn text_to_adf_preserves_newlines() {
        let original = "Line one\nLine two\nLine three";
        let adf = text_to_adf(original);
        let extracted = extract_adf_text(&adf);
        assert!(extracted.contains("Line one"));
        assert!(extracted.contains("Line two"));
        assert!(extracted.contains("Line three"));
    }

    #[test]
    fn text_to_adf_single_line_roundtrip() {
        let original = "My description text";
        let adf = text_to_adf(original);
        let extracted = extract_adf_text(&adf);
        assert_eq!(extracted, original);
    }

    #[test]
    fn text_to_adf_blank_line_produces_empty_paragraph() {
        let adf = text_to_adf("First\n\nThird");
        let content = adf["content"].as_array().unwrap();
        assert_eq!(content.len(), 3);
        // The blank middle line must produce an empty content array, not a text node
        // with an empty string — the latter is rejected by some Jira Cloud instances.
        let blank_paragraph = &content[1];
        assert_eq!(blank_paragraph["type"], "paragraph");
        let blank_content = blank_paragraph["content"].as_array().unwrap();
        assert!(blank_content.is_empty());
    }

    #[test]
    fn escape_jql_double_quotes() {
        assert_eq!(escape_jql(r#"say "hello""#), r#"say \"hello\""#);
    }

    #[test]
    fn escape_jql_clean_input() {
        assert_eq!(escape_jql("In Progress"), "In Progress");
    }

    #[test]
    fn escape_jql_backslash() {
        assert_eq!(escape_jql(r"foo\bar"), r"foo\\bar");
    }
}
