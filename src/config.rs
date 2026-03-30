use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::api::ApiError;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ProfileConfig {
    pub host: Option<String>,
    pub email: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawConfig {
    #[serde(flatten)]
    default: ProfileConfig,
    #[serde(default)]
    profiles: BTreeMap<String, ProfileConfig>,
}

/// Resolved credentials for a single profile.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub email: String,
    pub token: String,
}

impl Config {
    /// Load config with priority: CLI args > env vars > config file.
    ///
    /// The API token must be supplied via the `JIRA_TOKEN` environment variable
    /// or the config file — not via a CLI flag, to avoid leaking it in process
    /// argument lists visible to other users.
    pub fn load(
        host_arg: Option<String>,
        email_arg: Option<String>,
        profile_arg: Option<String>,
    ) -> Result<Self, ApiError> {
        let file_profile = load_file_profile(profile_arg.as_deref())?;

        let host = host_arg
            .or_else(|| std::env::var("JIRA_HOST").ok())
            .or(file_profile.host)
            .ok_or_else(|| {
                ApiError::InvalidInput(
                    "No Jira host configured. Set JIRA_HOST or run `jira config init`.".into(),
                )
            })?;

        let email = email_arg
            .or_else(|| std::env::var("JIRA_EMAIL").ok())
            .or(file_profile.email)
            .ok_or_else(|| {
                ApiError::InvalidInput(
                    "No email configured. Set JIRA_EMAIL or run `jira config init`.".into(),
                )
            })?;

        let token = std::env::var("JIRA_TOKEN")
            .ok()
            .or(file_profile.token)
            .ok_or_else(|| {
                ApiError::InvalidInput(
                    "No API token configured. Set JIRA_TOKEN or run `jira config init`.".into(),
                )
            })?;

        Ok(Self { host, email, token })
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("jira")
        .join("config.toml")
}

fn load_file_profile(profile: Option<&str>) -> Result<ProfileConfig, ApiError> {
    let path = config_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ProfileConfig::default()),
        Err(e) => return Err(ApiError::Other(format!("Failed to read config: {e}"))),
    };

    let raw: RawConfig = toml::from_str(&content)
        .map_err(|e| ApiError::Other(format!("Failed to parse config: {e}")))?;

    let profile_name = profile
        .map(String::from)
        .or_else(|| std::env::var("JIRA_PROFILE").ok());

    match profile_name {
        Some(name) => {
            // BTreeMap gives sorted, deterministic output in error messages
            let available: Vec<&str> = raw.profiles.keys().map(String::as_str).collect();
            raw.profiles.get(&name).cloned().ok_or_else(|| {
                ApiError::Other(format!(
                    "Profile '{name}' not found in config. Available: {}",
                    available.join(", ")
                ))
            })
        }
        None => Ok(raw.default),
    }
}

/// Print the config file path and current resolved values (masking the token).
pub fn show(host_arg: Option<String>, email_arg: Option<String>, profile_arg: Option<String>) {
    let path = config_path();
    eprintln!("Config file: {}", path.display());

    match Config::load(host_arg, email_arg, profile_arg) {
        Ok(cfg) => {
            let masked = mask_token(&cfg.token);
            println!("host:  {}", cfg.host);
            println!("email: {}", cfg.email);
            println!("token: {masked}");
        }
        Err(e) => {
            eprintln!("Config error: {e}");
        }
    }
}

/// Print example config file and instructions for obtaining an API token.
pub fn init() {
    let path = config_path();
    println!("Create or edit: {}", path.display());
    println!();
    println!("Example config:");
    println!();
    println!("[default]");
    println!("host  = \"mycompany.atlassian.net\"");
    println!("email = \"me@example.com\"");
    println!("token = \"your-api-token\"");
    println!();
    println!("# Optional named profiles:");
    println!("# [profiles.work]");
    println!("# host  = \"work.atlassian.net\"");
    println!("# email = \"me@work.com\"");
    println!("# token = \"work-token\"");
    println!();
    println!(
        "Get your API token at: https://id.atlassian.com/manage-profile/security/api-tokens"
    );
    println!();
    println!("Permissions: chmod 600 {}", path.display());
}

/// Mask a token for display, showing only the last 4 characters.
///
/// Atlassian tokens begin with a predictable prefix, so showing the
/// start provides no meaningful identification — the end is more useful.
fn mask_token(token: &str) -> String {
    let n = token.chars().count();
    if n > 4 {
        let suffix: String = token.chars().skip(n - 4).collect();
        format!("***{suffix}")
    } else {
        "***".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_token_long() {
        let masked = mask_token("ATATxxx1234abcd");
        assert!(masked.starts_with("***"));
        assert!(masked.ends_with("abcd"));
    }

    #[test]
    fn mask_token_short() {
        assert_eq!(mask_token("abc"), "***");
    }

    #[test]
    fn mask_token_unicode_safe() {
        // Ensure char-based indexing doesn't panic on multi-byte chars
        let token = "token-日本語-end";
        let result = mask_token(token);
        assert!(result.starts_with("***"));
    }
}
