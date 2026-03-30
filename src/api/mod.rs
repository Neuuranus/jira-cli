pub mod client;
pub mod types;

pub use client::JiraClient;
pub use types::*;

use std::fmt;

#[derive(Debug)]
pub enum ApiError {
    /// Bad credentials or forbidden.
    Auth(String),
    /// Resource not found.
    NotFound(String),
    /// Invalid user input (bad key format, missing required value, etc.).
    InvalidInput(String),
    /// HTTP 429 rate limit.
    RateLimit,
    /// Non-2xx response from the Jira API.
    Api { status: u16, message: String },
    /// Network / TLS error.
    Http(reqwest::Error),
    /// Any other error.
    Other(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Auth(msg) => write!(
                f,
                "Authentication failed: {msg}\nCheck JIRA_TOKEN or run `jira config show` to verify credentials."
            ),
            ApiError::NotFound(msg) => write!(f, "Not found: {msg}"),
            ApiError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            ApiError::RateLimit => write!(f, "Rate limited by Jira. Please wait and try again."),
            ApiError::Api { status, message } => write!(f, "API error {status}: {message}"),
            ApiError::Http(e) => write!(f, "HTTP error: {e}"),
            ApiError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ApiError::Http(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::Http(e)
    }
}
