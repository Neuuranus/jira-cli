use wiremock::matchers::{header, method, path, path_regex, query_param, query_param_contains};
use wiremock::{Mock, MockServer, ResponseTemplate};

use jira_cli::api::{ApiError, AuthType, JiraClient};
use jira_cli::output::OutputConfig;

// ── Fixtures ──────────────────────────────────────────────────────────────────

fn test_client(server: &MockServer) -> JiraClient {
    JiraClient::new(
        &server.uri(),
        "test@example.com",
        "test-token",
        AuthType::Basic,
        3,
    )
    .unwrap()
}

fn test_client_pat(server: &MockServer) -> JiraClient {
    JiraClient::new(&server.uri(), "", "my-pat-token", AuthType::Pat, 3).unwrap()
}

fn test_client_v2(server: &MockServer) -> JiraClient {
    JiraClient::new(
        &server.uri(),
        "test@example.com",
        "test-token",
        AuthType::Basic,
        2,
    )
    .unwrap()
}

fn json_out() -> OutputConfig {
    OutputConfig {
        json: true,
        quiet: true,
    }
}

fn issue_fixture(key: &str, summary: &str, status: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "10001",
        "key": key,
        "self": format!("https://test.atlassian.net/rest/api/3/issue/{key}"),
        "fields": {
            "summary": summary,
            "status": { "name": status },
            "assignee": { "displayName": "Alice", "accountId": "abc123" },
            "reporter": { "displayName": "Bob", "accountId": "def456" },
            "priority": { "name": "Medium" },
            "issuetype": { "name": "Bug" },
            "description": {
                "type": "doc", "version": 1,
                "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Test description"}]}]
            },
            "labels": ["backend", "urgent"],
            "created": "2024-01-15T10:00:00.000Z",
            "updated": "2024-01-20T15:30:00.000Z",
            "comment": {
                "comments": [
                    {
                        "id": "10100",
                        "author": { "displayName": "Alice", "accountId": "abc123" },
                        "body": {
                            "type": "doc", "version": 1,
                            "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Looks good"}]}]
                        },
                        "created": "2024-01-21T09:00:00.000Z"
                    }
                ],
                "total": 1
            }
        }
    })
}

fn search_response(issues: Vec<serde_json::Value>) -> serde_json::Value {
    let count = issues.len();
    serde_json::json!({
        "issues": issues,
        "total": count,
        "startAt": 0,
        "maxResults": 50
    })
}

fn project_search_response(projects: Vec<serde_json::Value>) -> serde_json::Value {
    let total = projects.len();
    serde_json::json!({
        "values": projects,
        "total": total,
        "startAt": 0,
        "maxResults": 50,
        "isLast": true
    })
}

// ── Auth header ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn client_sends_basic_auth_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(header(
            "authorization",
            "Basic dGVzdEBleGFtcGxlLmNvbTp0ZXN0LXRva2Vu",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    client.list_projects().await.unwrap();
}

// ── Issue validation ───────────────────────────────────────────────────────────

#[tokio::test]
async fn get_issue_rejects_invalid_key() {
    let server = MockServer::start().await;
    let client = test_client(&server);

    let cases = [
        "proj-123",
        "PROJ123",
        "PROJ-abc",
        "../etc/passwd",
        "",
        "1PROJ-123",
    ];
    for key in cases {
        let err = client.get_issue(key).await.unwrap_err();
        assert!(
            matches!(err, ApiError::InvalidInput(_)),
            "expected InvalidInput for key={key:?}, got {err}"
        );
    }
}

#[tokio::test]
async fn get_issue_accepts_valid_key() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex(r"/rest/api/3/issue/PROJ-\d+"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_fixture("PROJ-123", "Fix bug", "Open")),
        )
        .mount(&server)
        .await;

    let client = test_client(&server);
    let issue = client.get_issue("PROJ-123").await.unwrap();
    assert_eq!(issue.key, "PROJ-123");
    assert_eq!(issue.summary(), "Fix bug");
}

#[tokio::test]
async fn get_issue_accepts_key_with_digit_in_project_part() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex(r"/rest/api/3/issue/ABC2-\d+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_fixture(
            "ABC2-1",
            "Digit key",
            "Open",
        )))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let issue = client.get_issue("ABC2-1").await.unwrap();
    assert_eq!(issue.key, "ABC2-1");
}

// ── Search / list ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn search_returns_issues_with_pagination_metadata() {
    let server = MockServer::start().await;
    let issue = issue_fixture("PROJ-1", "First issue", "To Do");

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issues": [issue],
            "total": 42,
            "startAt": 0,
            "maxResults": 1,
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let resp = client.search("project = PROJ", 1, 0).await.unwrap();
    assert_eq!(resp.total, 42);
    assert_eq!(resp.start_at, 0);
    assert_eq!(resp.issues.len(), 1);
    assert_eq!(resp.issues[0].key, "PROJ-1");
}

#[tokio::test]
async fn search_passes_jql_as_query_param() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search"))
        .and(query_param_contains("jql", "project"))
        .and(query_param_contains("jql", "PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_response(vec![])))
        .mount(&server)
        .await;

    let client = test_client(&server);
    client.search("project = PROJ", 50, 0).await.unwrap();
}

#[tokio::test]
async fn search_passes_offset_as_start_at() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search"))
        .and(query_param("startAt", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    client.search("project = PROJ", 25, 25).await.unwrap();
}

#[tokio::test]
async fn search_uses_post_for_long_jql_queries() {
    let server = MockServer::start().await;
    let long_clause = "x".repeat(2000);
    let jql = format!("summary ~ \"{long_clause}\"");

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    client.search(&jql, 50, 0).await.unwrap();
}

// ── Issue detail ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn show_issue_includes_description_and_comments() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_fixture(
            "PROJ-42",
            "Important bug",
            "In Progress",
        )))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let issue = client.get_issue("PROJ-42").await.unwrap();

    // Verify description extraction
    assert_eq!(issue.description_text(), "Test description");
    // Verify comment is present and has correct content
    let comment_list = issue.fields.comment.as_ref().unwrap();
    assert_eq!(comment_list.total, 1);
    assert_eq!(comment_list.comments.len(), 1);
    assert_eq!(comment_list.comments[0].body_text(), "Looks good");
    assert_eq!(comment_list.comments[0].author.display_name, "Alice");
}

#[tokio::test]
async fn show_issue_json_contains_expected_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_fixture(
            "PROJ-42",
            "Important bug",
            "In Progress",
        )))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let issue = client.get_issue("PROJ-42").await.unwrap();
    assert_eq!(issue.key, "PROJ-42");
    assert_eq!(issue.summary(), "Important bug");
    assert_eq!(issue.status(), "In Progress");
    assert_eq!(issue.issue_type(), "Bug");
    assert_eq!(issue.priority(), "Medium");
    assert_eq!(issue.assignee(), "Alice");
    assert_eq!(issue.description_text(), "Test description");
}

#[tokio::test]
async fn show_issue_extracts_adf_description() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex(r"/rest/api/3/issue/PROJ-\d+"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_fixture("PROJ-1", "Test", "Open")),
        )
        .mount(&server)
        .await;

    let client = test_client(&server);
    let issue = client.get_issue("PROJ-1").await.unwrap();
    assert_eq!(issue.description_text(), "Test description");
}

// ── Create issue ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_issue_posts_correct_payload() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10042",
            "key": "PROJ-42",
            "self": "https://test.atlassian.net/rest/api/3/issue/10042"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let resp = client
        .create_issue(
            "PROJ",
            "Bug",
            "Something broke",
            Some("Details here"),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(resp.key, "PROJ-42");
    assert_eq!(resp.id, "10042");
}

// ── Comments ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn add_comment_posts_adf_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10200",
            "author": { "displayName": "Alice", "accountId": "abc123" },
            "body": {
                "type": "doc", "version": 1,
                "content": [{"type": "paragraph", "content": [{"type": "text", "text": "My comment"}]}]
            },
            "created": "2024-01-22T08:00:00.000Z"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let comment = client.add_comment("PROJ-1", "My comment").await.unwrap();
    assert_eq!(comment.id, "10200");
    assert_eq!(comment.author.display_name, "Alice");
    assert_eq!(comment.body_text(), "My comment");
}

#[tokio::test]
async fn get_issue_fetches_additional_comment_pages() {
    let server = MockServer::start().await;

    // Issue with 1 embedded comment but total=3 — client must fetch 2 more
    let issue_body = serde_json::json!({
        "id": "10001",
        "key": "PROJ-1",
        "fields": {
            "summary": "Test",
            "status": { "name": "Open" },
            "issuetype": { "name": "Bug" },
            "comment": {
                "comments": [
                    {
                        "id": "1",
                        "author": { "displayName": "Alice", "accountId": "abc" },
                        "body": null,
                        "created": "2024-01-01T00:00:00.000Z"
                    }
                ],
                "total": 3,
                "startAt": 0,
                "maxResults": 1
            }
        }
    });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_body))
        .mount(&server)
        .await;

    // Additional comment page
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [
                {
                    "id": "2",
                    "author": { "displayName": "Bob", "accountId": "def" },
                    "body": null,
                    "created": "2024-01-02T00:00:00.000Z"
                },
                {
                    "id": "3",
                    "author": { "displayName": "Charlie", "accountId": "ghi" },
                    "body": null,
                    "created": "2024-01-03T00:00:00.000Z"
                }
            ],
            "total": 3,
            "startAt": 1,
            "maxResults": 100
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let issue = client.get_issue("PROJ-1").await.unwrap();
    let comment_list = issue.fields.comment.as_ref().unwrap();
    // All 3 comments must be present after pagination
    assert_eq!(comment_list.comments.len(), 3);
    assert_eq!(comment_list.comments[0].id, "1");
    assert_eq!(comment_list.comments[1].id, "2");
    assert_eq!(comment_list.comments[2].id, "3");
}

// ── Transitions ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_transitions_returns_list() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [
                { "id": "11", "name": "To Do" },
                { "id": "21", "name": "In Progress" },
                { "id": "31", "name": "Done" },
            ]
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let ts = client.get_transitions("PROJ-1").await.unwrap();
    assert_eq!(ts.len(), 3);
    assert_eq!(ts[1].name, "In Progress");
    assert_eq!(ts[1].id, "21");
}

#[tokio::test]
async fn get_transitions_includes_to_field_when_present() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [{
                "id": "21",
                "name": "In Progress",
                "to": {
                    "name": "In Progress",
                    "statusCategory": { "key": "indeterminate", "name": "In Progress" }
                }
            }]
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let ts = client.get_transitions("PROJ-1").await.unwrap();
    assert_eq!(ts.len(), 1);
    let to = ts[0].to.as_ref().unwrap();
    assert_eq!(to.name, "In Progress");
    let cat = to.status_category.as_ref().unwrap();
    assert_eq!(cat.key, "indeterminate");
}

#[tokio::test]
async fn transition_matches_by_name_case_insensitive() {
    let server = MockServer::start().await;

    // transitions endpoint
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [
                { "id": "11", "name": "To Do" },
                { "id": "21", "name": "In Progress" },
            ]
        })))
        .mount(&server)
        .await;

    // transition POST
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let out = json_out();
    jira_cli::commands::issues::transition(&client, &out, "PROJ-1", "in progress")
        .await
        .unwrap();
}

#[tokio::test]
async fn transition_not_found_returns_structured_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [{ "id": "11", "name": "Done" }]
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let out = json_out();
    let err = jira_cli::commands::issues::transition(&client, &out, "PROJ-1", "Nonexistent")
        .await
        .unwrap_err();
    assert!(matches!(err, ApiError::NotFound(_)));
    // Error message must reference the missing transition name
    if let ApiError::NotFound(msg) = &err {
        assert!(
            msg.contains("Nonexistent"),
            "expected transition name in error, got: {msg}"
        );
    }
}

// ── Projects ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_projects_returns_all_projects() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_search_response(vec![
            serde_json::json!({ "id": "10000", "key": "ALPHA", "name": "Alpha Project", "projectTypeKey": "software" }),
            serde_json::json!({ "id": "10001", "key": "BETA", "name": "Beta Project", "projectTypeKey": "business" }),
        ])))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let projects = client.list_projects().await.unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].key, "ALPHA");
    assert_eq!(projects[0].name, "Alpha Project");
    assert_eq!(projects[1].key, "BETA");
}

#[tokio::test]
async fn list_projects_handles_short_non_terminal_pages() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [
                { "id": "10000", "key": "ALPHA", "name": "Alpha Project", "projectTypeKey": "software" },
                { "id": "10001", "key": "BETA", "name": "Beta Project", "projectTypeKey": "business" }
            ],
            "total": 3,
            "startAt": 0,
            "maxResults": 50,
            "isLast": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(query_param("startAt", "2"))
        .and(query_param("maxResults", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [
                { "id": "10002", "key": "GAMMA", "name": "Gamma Project", "projectTypeKey": "service_desk" }
            ],
            "total": 3,
            "startAt": 2,
            "maxResults": 50,
            "isLast": true
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let projects = client.list_projects().await.unwrap();
    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0].key, "ALPHA");
    assert_eq!(projects[1].key, "BETA");
    assert_eq!(projects[2].key, "GAMMA");
}

// ── Error mapping ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn api_401_maps_to_auth_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.list_projects().await.unwrap_err();
    assert!(matches!(err, ApiError::Auth(_)));
}

#[tokio::test]
async fn api_404_maps_to_not_found_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex(r"/rest/api/3/issue/PROJ-\d+"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Issue does not exist"))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.get_issue("PROJ-999").await.unwrap_err();
    assert!(matches!(err, ApiError::NotFound(_)));
}

#[tokio::test]
async fn api_429_maps_to_rate_limit_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.list_projects().await.unwrap_err();
    assert!(matches!(err, ApiError::RateLimit));
}

#[tokio::test]
async fn api_500_maps_to_api_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let err = client.list_projects().await.unwrap_err();
    assert!(matches!(err, ApiError::Api { status: 500, .. }));
}

// ── JQL escaping ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn search_encodes_jql_in_query_string() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_response(vec![])))
        .mount(&server)
        .await;

    let client = test_client(&server);
    // JQL with special characters — should not panic or produce a malformed URL
    client
        .search(
            r#"project = "My Project" AND status = "In Progress""#,
            10,
            0,
        )
        .await
        .unwrap();
}

// ── ADF helpers ───────────────────────────────────────────────────────────────

#[test]
fn text_to_adf_multiline_produces_multiple_paragraphs() {
    use jira_cli::api::text_to_adf;
    let adf = text_to_adf("First line\nSecond line");
    let content = adf["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "paragraph");
    assert_eq!(content[1]["type"], "paragraph");
}

#[test]
fn text_to_adf_blank_line_produces_empty_content_array() {
    use jira_cli::api::text_to_adf;
    let adf = text_to_adf("Before\n\nAfter");
    let content = adf["content"].as_array().unwrap();
    assert_eq!(content.len(), 3);
    // The blank line must produce an empty content array — not a text node
    // with "" which some Jira Cloud instances reject with 400.
    let blank = &content[1];
    assert_eq!(blank["type"], "paragraph");
    let blank_content = blank["content"].as_array().unwrap();
    assert!(
        blank_content.is_empty(),
        "blank line must produce empty content, not text nodes"
    );
}

#[test]
fn adf_extract_handles_code_block() {
    use jira_cli::api::extract_adf_text;
    let doc = serde_json::json!({
        "type": "doc",
        "version": 1,
        "content": [{
            "type": "codeBlock",
            "content": [{"type": "text", "text": "let x = 1;"}]
        }]
    });
    assert_eq!(extract_adf_text(&doc), "let x = 1;");
}

#[test]
fn escape_jql_prevents_injection() {
    use jira_cli::api::escape_jql;
    let malicious = r#"Done" OR 1=1 --"#;
    let escaped = escape_jql(malicious);
    // The double quote must be backslash-escaped so it cannot break out of a JQL string literal.
    // After escaping, `Done" OR 1=1 --` becomes `Done\" OR 1=1 --`.
    assert!(
        escaped.contains("\\\""),
        "double quote must be backslash-escaped"
    );
    // The escaped value must start with the prefix up to and including the escaped quote,
    // confirming the quote is inside the escaped string, not acting as a string terminator.
    assert!(
        escaped.starts_with(r#"Done\""#),
        "escaped value must begin with the literal prefix Done\""
    );
}

// ── issue update ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn update_issue_sends_put_request() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    client
        .update_issue("PROJ-1", Some("New summary"), None, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn update_issue_requires_at_least_one_field() {
    let server = MockServer::start().await;
    let client = test_client(&server);

    let err = client
        .update_issue("PROJ-1", None, None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, ApiError::InvalidInput(_)));
}

// ── myself ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn myself_returns_account_id_and_display_name() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "abc123",
            "displayName": "Alice Smith",
            "emailAddress": "alice@example.com"
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let me = client.get_myself().await.unwrap();
    assert_eq!(me.account_id, "abc123");
    assert_eq!(me.display_name, "Alice Smith");
}

// ── accountId in issue JSON ───────────────────────────────────────────────────

#[tokio::test]
async fn issue_json_includes_assignee_account_id() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex(r"/rest/api/3/issue/PROJ-\d+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-1",
            "fields": {
                "summary": "Test",
                "status": { "name": "Open" },
                "assignee": {
                    "displayName": "Alice",
                    "accountId": "alice-account-id-123",
                    "emailAddress": "alice@example.com"
                },
                "issuetype": { "name": "Bug" },
                "priority": { "name": "High" }
            }
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let issue = client.get_issue("PROJ-1").await.unwrap();
    let account_id = issue
        .fields
        .assignee
        .as_ref()
        .and_then(|a| a.account_id.as_deref());
    assert_eq!(account_id, Some("alice-account-id-123"));
}

// ── transition error stdout contract ─────────────────────────────────────────

#[tokio::test]
async fn transition_not_found_produces_no_stdout_data() {
    // Verifies that a failed transition does NOT write JSON to stdout.
    // Error information goes to stderr via print_message; stdout stays clean
    // so agents piping stdout get nothing on failure.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [{ "id": "11", "name": "Done" }]
        })))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let out = OutputConfig {
        json: true,
        quiet: true,
    };
    let err = jira_cli::commands::issues::transition(&client, &out, "PROJ-1", "Nonexistent")
        .await
        .unwrap_err();

    // Must return NotFound (exit code 4), not a generic error
    assert!(
        matches!(err, ApiError::NotFound(_)),
        "expected NotFound, got: {err}"
    );
    // The error message must identify the missing transition
    if let ApiError::NotFound(msg) = &err {
        assert!(
            msg.contains("Nonexistent"),
            "error message must include the requested transition name; got: {msg}"
        );
        assert!(
            msg.contains("PROJ-1"),
            "error message must include the issue key; got: {msg}"
        );
    }
}

// ── invalid input exit code ───────────────────────────────────────────────────

#[test]
fn invalid_issue_key_maps_to_input_error_exit_code() {
    use jira_cli::output::{exit_code_for_error, exit_codes};
    let err = ApiError::InvalidInput("bad key".into());
    assert_eq!(exit_code_for_error(&err), exit_codes::INPUT_ERROR);
}

#[test]
fn missing_credentials_maps_to_input_error_exit_code() {
    use jira_cli::output::{exit_code_for_error, exit_codes};
    let err = ApiError::InvalidInput("No Jira host configured.".into());
    assert_eq!(exit_code_for_error(&err), exit_codes::INPUT_ERROR);
}

// ── Empty results ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn issues_list_with_no_results_succeeds() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_response(vec![])))
        .mount(&server)
        .await;

    let client = test_client(&server);
    let out = json_out();
    jira_cli::commands::issues::list(&client, &out, None, None, None, None, None, 50, 0)
        .await
        .unwrap();
}

// ── Myself command ────────────────────────────────────────────────────────────

#[tokio::test]
async fn myself_command_returns_account_info() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "user-abc-123",
            "displayName": "Test User",
            "emailAddress": "test@example.com"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let out = json_out();
    jira_cli::commands::myself::show(&client, &out)
        .await
        .unwrap();
}

// ── Projects show ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn projects_show_returns_project_details() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ",
            "name": "Test Project",
            "projectTypeKey": "software"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let out = json_out();
    jira_cli::commands::projects::show(&client, &out, "PROJ")
        .await
        .unwrap();
}

// ── PAT auth ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pat_auth_sends_bearer_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(header("authorization", "Bearer my-pat-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client_pat(&server);
    client.list_projects().await.unwrap();
}

#[tokio::test]
async fn basic_auth_does_not_send_bearer_header() {
    let server = MockServer::start().await;

    // Basic auth header must NOT start with "Bearer"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(header(
            "authorization",
            "Basic dGVzdEBleGFtcGxlLmNvbTp0ZXN0LXRva2Vu",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    client.list_projects().await.unwrap();
}

// ── API v2 ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn api_v2_uses_v2_base_path() {
    let server = MockServer::start().await;

    // Must hit /rest/api/2/myself, not /rest/api/3/myself
    Mock::given(method("GET"))
        .and(path("/rest/api/2/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "testuser",
            "displayName": "Test User",
            "key": "testuser",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client_v2(&server);
    client.get_myself().await.unwrap();
}

#[tokio::test]
async fn api_v2_add_comment_sends_plain_string_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/2/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10100",
            "author": { "displayName": "Test", "accountId": "abc" },
            "body": "Hello world",
            "created": "2024-01-01T00:00:00.000Z"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client_v2(&server);
    let comment = client.add_comment("PROJ-1", "Hello world").await.unwrap();
    assert_eq!(comment.id, "10100");
}

#[tokio::test]
async fn api_v2_plain_string_description_is_extracted_as_text() {
    use jira_cli::api::extract_adf_text;
    let plain = serde_json::Value::String("This is a plain description".to_string());
    assert_eq!(extract_adf_text(&plain), "This is a plain description");
}

#[tokio::test]
async fn api_v3_adf_description_still_extracted_correctly() {
    use jira_cli::api::extract_adf_text;
    let adf = serde_json::json!({
        "type": "doc", "version": 1,
        "content": [{"type": "paragraph", "content": [{"type": "text", "text": "ADF paragraph"}]}]
    });
    assert_eq!(extract_adf_text(&adf), "ADF paragraph");
}
