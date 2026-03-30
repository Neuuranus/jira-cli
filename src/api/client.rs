use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;

use super::types::*;
use super::ApiError;

pub struct JiraClient {
    http: reqwest::Client,
    base_url: String,
    host: String,
}

impl JiraClient {
    pub fn new(host: &str, email: &str, token: &str) -> Result<Self, ApiError> {
        // Determine the scheme. An explicit `http://` prefix is preserved as-is
        // (useful for local testing); everything else defaults to HTTPS.
        let (scheme, domain) = if host.starts_with("http://") {
            ("http", host.trim_start_matches("http://").trim_end_matches('/'))
        } else {
            ("https", host.trim_start_matches("https://").trim_end_matches('/'))
        };

        if domain.is_empty() {
            return Err(ApiError::Other("Host cannot be empty".into()));
        }

        let credentials = BASE64.encode(format!("{email}:{token}"));
        let auth_value = format!("Basic {credentials}");

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value).map_err(|e| ApiError::Other(e.to_string()))?,
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(ApiError::Http)?;

        let base_url = format!("{scheme}://{domain}/rest/api/3");

        Ok(Self {
            http,
            base_url,
            host: domain.to_string(),
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    fn map_status(status: u16, body: String) -> ApiError {
        // Truncate body to avoid leaking large/sensitive API responses
        let message = truncate_error_body(&body);
        match status {
            401 | 403 => ApiError::Auth(message),
            404 => ApiError::NotFound(message),
            429 => ApiError::RateLimit,
            _ => ApiError::Api { status, message },
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let url = format!("{}/{path}", self.base_url);
        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Self::map_status(status.as_u16(), body));
        }
        resp.json::<T>().await.map_err(ApiError::Http)
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, ApiError> {
        let url = format!("{}/{path}", self.base_url);
        let resp = self.http.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(Self::map_status(status.as_u16(), body_text));
        }
        resp.json::<T>().await.map_err(ApiError::Http)
    }

    async fn post_empty_response(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<(), ApiError> {
        let url = format!("{}/{path}", self.base_url);
        let resp = self.http.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(Self::map_status(status.as_u16(), body_text));
        }
        Ok(())
    }

    async fn put_empty_response(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<(), ApiError> {
        let url = format!("{}/{path}", self.base_url);
        let resp = self.http.put(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(Self::map_status(status.as_u16(), body_text));
        }
        Ok(())
    }

    // ── Issues ────────────────────────────────────────────────────────────────

    /// Search issues using JQL.
    pub async fn search(
        &self,
        jql: &str,
        max_results: usize,
        start_at: usize,
    ) -> Result<SearchResponse, ApiError> {
        let fields = "summary,status,assignee,priority,issuetype,created,updated";
        let path = format!(
            "search?jql={}&maxResults={max_results}&startAt={start_at}&fields={fields}",
            percent_encode(jql)
        );
        self.get(&path).await
    }

    /// Fetch a single issue by key (e.g. `PROJ-123`), including all comments.
    ///
    /// Jira embeds only the first page of comments in the issue response. When
    /// the embedded page is incomplete, additional requests are made to fetch
    /// the remaining comments.
    pub async fn get_issue(&self, key: &str) -> Result<Issue, ApiError> {
        validate_issue_key(key)?;
        let fields =
            "summary,status,assignee,reporter,priority,issuetype,description,labels,created,updated,comment";
        let path = format!("issue/{key}?fields={fields}");
        let mut issue: Issue = self.get(&path).await?;

        // Fetch remaining comment pages if the embedded page is incomplete
        if let Some(ref mut comment_list) = issue.fields.comment
            && comment_list.total > comment_list.comments.len()
        {
            let mut start_at = comment_list.comments.len();
            while comment_list.comments.len() < comment_list.total {
                let page: CommentList = self
                    .get(&format!(
                        "issue/{key}/comment?startAt={start_at}&maxResults=100"
                    ))
                    .await?;
                if page.comments.is_empty() {
                    break;
                }
                start_at += page.comments.len();
                comment_list.comments.extend(page.comments);
            }
        }

        Ok(issue)
    }

    /// Create a new issue.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_issue(
        &self,
        project_key: &str,
        issue_type: &str,
        summary: &str,
        description: Option<&str>,
        priority: Option<&str>,
        labels: Option<&[&str]>,
        assignee: Option<&str>,
    ) -> Result<CreateIssueResponse, ApiError> {
        let mut fields = serde_json::json!({
            "project": { "key": project_key },
            "issuetype": { "name": issue_type },
            "summary": summary,
        });

        if let Some(desc) = description {
            fields["description"] = text_to_adf(desc);
        }
        if let Some(p) = priority {
            fields["priority"] = serde_json::json!({ "name": p });
        }
        if let Some(lbls) = labels
            && !lbls.is_empty()
        {
            fields["labels"] = serde_json::json!(lbls);
        }
        if let Some(account_id) = assignee {
            fields["assignee"] = serde_json::json!({ "accountId": account_id });
        }

        self.post("issue", &serde_json::json!({ "fields": fields }))
            .await
    }

    /// Add a comment to an issue.
    pub async fn add_comment(&self, key: &str, body: &str) -> Result<Comment, ApiError> {
        validate_issue_key(key)?;
        let payload = serde_json::json!({ "body": text_to_adf(body) });
        self.post(&format!("issue/{key}/comment"), &payload).await
    }

    /// List available transitions for an issue.
    pub async fn get_transitions(&self, key: &str) -> Result<Vec<Transition>, ApiError> {
        validate_issue_key(key)?;
        let resp: TransitionsResponse = self.get(&format!("issue/{key}/transitions")).await?;
        Ok(resp.transitions)
    }

    /// Execute a transition by transition ID.
    pub async fn do_transition(&self, key: &str, transition_id: &str) -> Result<(), ApiError> {
        validate_issue_key(key)?;
        let payload = serde_json::json!({ "transition": { "id": transition_id } });
        self.post_empty_response(&format!("issue/{key}/transitions"), &payload)
            .await
    }

    /// Assign an issue to a user by account ID, or unassign with `None`.
    pub async fn assign_issue(
        &self,
        key: &str,
        account_id: Option<&str>,
    ) -> Result<(), ApiError> {
        validate_issue_key(key)?;
        let payload = serde_json::json!({
            "accountId": account_id
        });
        self.put_empty_response(&format!("issue/{key}/assignee"), &payload)
            .await
    }

    /// Get the currently authenticated user.
    pub async fn get_myself(&self) -> Result<Myself, ApiError> {
        self.get("myself").await
    }

    /// Update issue fields (summary, description, priority).
    pub async fn update_issue(
        &self,
        key: &str,
        summary: Option<&str>,
        description: Option<&str>,
        priority: Option<&str>,
    ) -> Result<(), ApiError> {
        validate_issue_key(key)?;
        let mut fields = serde_json::Map::new();
        if let Some(s) = summary {
            fields.insert("summary".into(), serde_json::Value::String(s.into()));
        }
        if let Some(d) = description {
            fields.insert("description".into(), text_to_adf(d));
        }
        if let Some(p) = priority {
            fields.insert(
                "priority".into(),
                serde_json::json!({ "name": p }),
            );
        }
        if fields.is_empty() {
            return Err(ApiError::InvalidInput(
                "At least one field (--summary, --description, --priority) is required".into(),
            ));
        }
        self.put_empty_response(
            &format!("issue/{key}"),
            &serde_json::json!({ "fields": fields }),
        )
        .await
    }

    // ── Projects ──────────────────────────────────────────────────────────────

    /// List all accessible projects, fetching all pages from the paginated endpoint.
    pub async fn list_projects(&self) -> Result<Vec<Project>, ApiError> {
        let mut all: Vec<Project> = Vec::new();
        let mut start_at: usize = 0;
        const PAGE: usize = 50;

        loop {
            let path = format!(
                "project/search?startAt={start_at}&maxResults={PAGE}&orderBy=key"
            );
            let page: ProjectSearchResponse = self.get(&path).await?;
            let is_last = page.is_last || page.values.len() < PAGE;
            all.extend(page.values);
            if is_last {
                break;
            }
            start_at += PAGE;
        }

        Ok(all)
    }

    /// Fetch a single project by key.
    pub async fn get_project(&self, key: &str) -> Result<Project, ApiError> {
        self.get(&format!("project/{key}")).await
    }
}

/// Validate that a key matches the `[A-Z][A-Z0-9]*-[0-9]+` format
/// before using it in a URL path.
///
/// Jira project keys start with an uppercase letter and may contain further
/// uppercase letters or digits (e.g. `ABC2-123` is valid).
fn validate_issue_key(key: &str) -> Result<(), ApiError> {
    let mut parts = key.splitn(2, '-');
    let project = parts.next().unwrap_or("");
    let number = parts.next().unwrap_or("");

    let valid = !project.is_empty()
        && !number.is_empty()
        && project.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && project
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && number.chars().all(|c| c.is_ascii_digit());

    if valid {
        Ok(())
    } else {
        Err(ApiError::InvalidInput(format!(
            "Invalid issue key '{key}'. Expected format: PROJECT-123"
        )))
    }
}

/// Percent-encode a string for use in a URL query parameter.
///
/// Uses `%20` for spaces (not `+`) per standard URL encoding.
fn percent_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 2);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b => encoded.push_str(&format!("%{b:02X}")),
        }
    }
    encoded
}

/// Truncate an API error body to avoid leaking large or sensitive responses.
fn truncate_error_body(body: &str) -> String {
    const MAX: usize = 200;
    if body.chars().count() <= MAX {
        body.to_string()
    } else {
        let truncated: String = body.chars().take(MAX).collect();
        format!("{truncated}… (truncated)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_spaces_use_percent_20() {
        assert_eq!(percent_encode("project = FOO"), "project%20%3D%20FOO");
    }

    #[test]
    fn percent_encode_complex_jql() {
        let jql = r#"project = "MY PROJECT""#;
        let encoded = percent_encode(jql);
        assert!(encoded.contains("project"));
        assert!(!encoded.contains('"'));
        assert!(!encoded.contains(' '));
    }

    #[test]
    fn validate_issue_key_valid() {
        assert!(validate_issue_key("PROJ-123").is_ok());
        assert!(validate_issue_key("ABC-1").is_ok());
        assert!(validate_issue_key("MYPROJECT-9999").is_ok());
        // Digits are allowed in the project key after the initial letter
        assert!(validate_issue_key("ABC2-123").is_ok());
        assert!(validate_issue_key("P1-1").is_ok());
    }

    #[test]
    fn validate_issue_key_invalid() {
        assert!(validate_issue_key("proj-123").is_err()); // lowercase
        assert!(validate_issue_key("PROJ123").is_err());  // no dash
        assert!(validate_issue_key("PROJ-abc").is_err()); // non-numeric suffix
        assert!(validate_issue_key("../etc/passwd").is_err());
        assert!(validate_issue_key("").is_err());
        assert!(validate_issue_key("1PROJ-123").is_err()); // starts with digit
    }

    #[test]
    fn truncate_error_body_short() {
        let body = "short error";
        assert_eq!(truncate_error_body(body), body);
    }

    #[test]
    fn truncate_error_body_long() {
        let body = "x".repeat(300);
        let result = truncate_error_body(&body);
        assert!(result.len() < body.len());
        assert!(result.ends_with("(truncated)"));
    }
}
