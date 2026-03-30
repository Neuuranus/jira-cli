use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::api::ApiError;
use crate::api::AuthType;
use crate::output::OutputConfig;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ProfileConfig {
    pub host: Option<String>,
    pub email: Option<String>,
    pub token: Option<String>,
    pub auth_type: Option<String>,
    pub api_version: Option<u8>,
}

#[derive(Debug, Deserialize, Default)]
struct RawConfig {
    #[serde(default)]
    default: ProfileConfig,
    #[serde(default)]
    profiles: BTreeMap<String, ProfileConfig>,
    host: Option<String>,
    email: Option<String>,
    token: Option<String>,
    auth_type: Option<String>,
    api_version: Option<u8>,
}

impl RawConfig {
    fn default_profile(&self) -> ProfileConfig {
        ProfileConfig {
            host: self.default.host.clone().or_else(|| self.host.clone()),
            email: self.default.email.clone().or_else(|| self.email.clone()),
            token: self.default.token.clone().or_else(|| self.token.clone()),
            auth_type: self
                .default
                .auth_type
                .clone()
                .or_else(|| self.auth_type.clone()),
            api_version: self.default.api_version.or(self.api_version),
        }
    }
}

/// Resolved credentials for a single profile.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub email: String,
    pub token: String,
    pub auth_type: AuthType,
    pub api_version: u8,
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

        let host = normalize_value(host_arg)
            .or_else(|| env_var("JIRA_HOST"))
            .or_else(|| normalize_value(file_profile.host))
            .ok_or_else(|| {
                ApiError::InvalidInput(
                    "No Jira host configured. Set JIRA_HOST or run `jira config init`.".into(),
                )
            })?;

        let token = env_var("JIRA_TOKEN")
            .or_else(|| normalize_value(file_profile.token.clone()))
            .ok_or_else(|| {
                ApiError::InvalidInput(
                    "No API token configured. Set JIRA_TOKEN or run `jira config init`.".into(),
                )
            })?;

        let auth_type = env_var("JIRA_AUTH_TYPE")
            .as_deref()
            .map(|v| {
                if v.eq_ignore_ascii_case("pat") {
                    AuthType::Pat
                } else {
                    AuthType::Basic
                }
            })
            .or_else(|| {
                file_profile.auth_type.as_deref().map(|v| {
                    if v.eq_ignore_ascii_case("pat") {
                        AuthType::Pat
                    } else {
                        AuthType::Basic
                    }
                })
            })
            .unwrap_or_default();

        let api_version = env_var("JIRA_API_VERSION")
            .and_then(|v| v.parse::<u8>().ok())
            .or(file_profile.api_version)
            .unwrap_or(3);

        // Email is required for Basic auth; PAT auth uses a token only.
        let email = normalize_value(email_arg)
            .or_else(|| env_var("JIRA_EMAIL"))
            .or_else(|| normalize_value(file_profile.email));

        let email = match auth_type {
            AuthType::Basic => email.ok_or_else(|| {
                ApiError::InvalidInput(
                    "No email configured. Set JIRA_EMAIL or run `jira config init`.".into(),
                )
            })?,
            AuthType::Pat => email.unwrap_or_default(),
        };

        Ok(Self {
            host,
            email,
            token,
            auth_type,
            api_version,
        })
    }
}

fn config_path() -> PathBuf {
    config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("jira")
        .join("config.toml")
}

pub fn schema_config_path() -> String {
    config_path().display().to_string()
}

pub fn schema_config_path_description() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Resolved at runtime to %APPDATA%\\jira\\config.toml by default."
    }

    #[cfg(not(target_os = "windows"))]
    {
        "Resolved at runtime to $XDG_CONFIG_HOME/jira/config.toml when set, otherwise ~/.config/jira/config.toml."
    }
}

pub fn recommended_permissions(path: &std::path::Path) -> String {
    #[cfg(target_os = "windows")]
    {
        format!(
            "Store this file in your per-user AppData directory ({}) and keep it out of shared folders; Windows applies per-user ACLs there by default.",
            path.display()
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        format!("chmod 600 {}", path.display())
    }
}

pub fn schema_recommended_permissions_example() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Keep the file in your per-user %APPDATA% directory and out of shared folders."
    }

    #[cfg(not(target_os = "windows"))]
    {
        "chmod 600 /path/to/config.toml"
    }
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
    }
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

    let profile_name = normalize_str(profile)
        .map(str::to_owned)
        .or_else(|| env_var("JIRA_PROFILE"));

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
        None => Ok(raw.default_profile()),
    }
}

/// Print the config file path and current resolved values (masking the token).
pub fn show(
    out: &OutputConfig,
    host_arg: Option<String>,
    email_arg: Option<String>,
    profile_arg: Option<String>,
) -> Result<(), ApiError> {
    let path = config_path();
    let cfg = Config::load(host_arg, email_arg, profile_arg)?;
    let masked = mask_token(&cfg.token);

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "configPath": path,
                "host": cfg.host,
                "email": cfg.email,
                "tokenMasked": masked,
            }))
            .expect("failed to serialize JSON"),
        );
    } else {
        out.print_message(&format!("Config file: {}", path.display()));
        out.print_data(&format!(
            "host:  {}\nemail: {}\ntoken: {masked}",
            cfg.host, cfg.email
        ));
    }
    Ok(())
}

/// Print example config file and instructions for obtaining an API token.
pub fn init(out: &OutputConfig) {
    let path = config_path();
    let path_resolution = schema_config_path_description();
    let permission_advice = recommended_permissions(&path);
    let example = serde_json::json!({
        "default": {
            "host": "mycompany.atlassian.net",
            "email": "me@example.com",
            "token": "your-api-token",
            "auth_type": "basic",
            "api_version": 3,
        },
        "profiles": {
            "work": {
                "host": "work.atlassian.net",
                "email": "me@work.com",
                "token": "work-token",
            },
            "datacenter": {
                "host": "jira.mycompany.com",
                "token": "your-personal-access-token",
                "auth_type": "pat",
                "api_version": 2,
            }
        }
    });

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "configPath": path,
                "pathResolution": path_resolution,
                "tokenInstructions": "https://id.atlassian.com/manage-profile/security/api-tokens",
                "recommendedPermissions": permission_advice,
                "example": example,
            }))
            .expect("failed to serialize JSON"),
        );
        return;
    }

    out.print_data(&format!(
        "Create or edit: {}\nPath resolution: {}\n\nExample config:\n\n[default]\nhost        = \"mycompany.atlassian.net\"\nemail       = \"me@example.com\"\ntoken       = \"your-api-token\"\nauth_type   = \"basic\"   # or \"pat\" for Jira Data Center / Server\napi_version = 3         # or 2 for Jira Data Center / Server\n\n# Optional named profiles:\n# [profiles.work]\n# host  = \"work.atlassian.net\"\n# email = \"me@work.com\"\n# token = \"work-token\"\n\n# Example Jira Data Center / Server profile (PAT auth):\n# [profiles.datacenter]\n# host        = \"jira.mycompany.com\"\n# token       = \"your-personal-access-token\"\n# auth_type   = \"pat\"\n# api_version = 2\n\nGet your API token at: https://id.atlassian.com/manage-profile/security/api-tokens\n\nPermissions: {}",
        path.display(),
        path_resolution,
        permission_advice,
    ));
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

fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .and_then(|value| normalize_value(Some(value)))
}

fn normalize_value(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_str(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{EnvVarGuard, ProcessEnvLock, set_config_dir_env, write_config};
    use tempfile::TempDir;

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

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn config_path_prefers_xdg_config_home() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        let _config_dir = set_config_dir_env(dir.path());

        assert_eq!(config_path(), dir.path().join("jira").join("config.toml"));
    }

    #[test]
    fn load_ignores_blank_env_vars_and_falls_back_to_file() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "work.atlassian.net"
email = "me@example.com"
token = "secret-token"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::set("JIRA_HOST", "   ");
        let _email = EnvVarGuard::set("JIRA_EMAIL", "");
        let _token = EnvVarGuard::set("JIRA_TOKEN", " ");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let cfg = Config::load(None, None, None).unwrap();
        assert_eq!(cfg.host, "work.atlassian.net");
        assert_eq!(cfg.email, "me@example.com");
        assert_eq!(cfg.token, "secret-token");
    }

    #[test]
    fn load_accepts_documented_default_section() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "example.atlassian.net"
email = "me@example.com"
token = "secret-token"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let cfg = Config::load(None, None, None).unwrap();
        assert_eq!(cfg.host, "example.atlassian.net");
        assert_eq!(cfg.email, "me@example.com");
        assert_eq!(cfg.token, "secret-token");
    }

    #[test]
    fn load_treats_blank_env_vars_as_missing_when_no_file_exists() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::set("JIRA_HOST", "");
        let _email = EnvVarGuard::set("JIRA_EMAIL", "");
        let _token = EnvVarGuard::set("JIRA_TOKEN", "");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let err = Config::load(None, None, None).unwrap_err();
        assert!(matches!(err, ApiError::InvalidInput(_)));
        assert!(err.to_string().contains("No Jira host configured"));
    }

    #[test]
    fn permission_guidance_matches_platform() {
        let guidance = recommended_permissions(std::path::Path::new("/tmp/jira/config.toml"));

        #[cfg(target_os = "windows")]
        assert!(guidance.contains("AppData"));

        #[cfg(not(target_os = "windows"))]
        assert!(guidance.starts_with("chmod 600 "));
    }
}
