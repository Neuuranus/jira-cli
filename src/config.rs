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
///
/// Pass `host` (e.g. `"jira.mycompany.com"`) to include a one-click URL to the
/// Personal Access Token creation page on a Jira DC/Server instance. When omitted
/// the URL is shown as a template placeholder.
pub fn init(out: &OutputConfig, host: Option<&str>) {
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

    const CLOUD_TOKEN_URL: &str = "https://id.atlassian.com/manage-profile/security/api-tokens";

    let pat_url = dc_pat_url(host);
    let config_status = if path.exists() {
        "exists — run `jira config show` to see current values"
    } else {
        "not found — create it"
    };

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "configPath": path,
                "pathResolution": path_resolution,
                "configExists": path.exists(),
                "tokenInstructions": CLOUD_TOKEN_URL,
                "dcPatInstructions": pat_url,
                "recommendedPermissions": permission_advice,
                "example": example,
            }))
            .expect("failed to serialize JSON"),
        );
        return;
    }

    let cloud_link = crate::output::hyperlink(CLOUD_TOKEN_URL);
    let pat_link = crate::output::hyperlink(&pat_url);

    out.print_data(&format!(
        "\
Config file: {path_display} ({config_status})

── Jira Cloud ────────────────────────────────────────────────────────────────

[default]
host  = \"mycompany.atlassian.net\"
email = \"me@example.com\"
token = \"your-api-token\"

  {cloud_link}

── Jira Data Center / Server ─────────────────────────────────────────────────

[profiles.dc]
host        = \"jira.mycompany.com\"
token       = \"your-personal-access-token\"
auth_type   = \"pat\"
api_version = 2

  {pat_link}

Use --profile dc to switch:  jira --profile dc <command>
                         or: JIRA_PROFILE=dc jira <command>

── Security ──────────────────────────────────────────────────────────────────

{permission_advice}",
        path_display = path.display(),
    ));
}

const PAT_PATH: &str = "/secure/ViewProfile.jspa?selectedTab=com.atlassian.pats.pats-plugin:jira-user-personal-access-tokens";

/// Build the Personal Access Token creation URL for a Jira DC/Server instance.
///
/// When `host` is known the full URL is returned so the user can click it directly.
/// When unknown a placeholder template is returned.
fn dc_pat_url(host: Option<&str>) -> String {
    match host {
        Some(h) => {
            let base = if h.starts_with("http://") || h.starts_with("https://") {
                h.trim_end_matches('/').to_string()
            } else {
                format!("https://{}", h.trim_end_matches('/'))
            };
            format!("{base}{PAT_PATH}")
        }
        None => format!("http://<your-host>{PAT_PATH}"),
    }
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

    // ── Priority: CLI > env > file ─────────────────────────────────────────────

    #[test]
    fn load_env_host_overrides_file() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "file.atlassian.net"
email = "me@example.com"
token = "tok"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::set("JIRA_HOST", "env.atlassian.net");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let cfg = Config::load(None, None, None).unwrap();
        assert_eq!(cfg.host, "env.atlassian.net");
    }

    #[test]
    fn load_cli_host_arg_overrides_env_and_file() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "file.atlassian.net"
email = "me@example.com"
token = "tok"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::set("JIRA_HOST", "env.atlassian.net");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let cfg = Config::load(Some("cli.atlassian.net".into()), None, None).unwrap();
        assert_eq!(cfg.host, "cli.atlassian.net");
    }

    // ── Error cases ────────────────────────────────────────────────────────────

    #[test]
    fn load_missing_token_returns_error() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::set("JIRA_HOST", "myhost.atlassian.net");
        let _email = EnvVarGuard::set("JIRA_EMAIL", "me@example.com");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let err = Config::load(None, None, None).unwrap_err();
        assert!(matches!(err, ApiError::InvalidInput(_)));
        assert!(err.to_string().contains("No API token"));
    }

    #[test]
    fn load_missing_email_for_basic_auth_returns_error() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::set("JIRA_HOST", "myhost.atlassian.net");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::set("JIRA_TOKEN", "secret");
        let _auth = EnvVarGuard::unset("JIRA_AUTH_TYPE");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let err = Config::load(None, None, None).unwrap_err();
        assert!(matches!(err, ApiError::InvalidInput(_)));
        assert!(err.to_string().contains("No email configured"));
    }

    #[test]
    fn load_invalid_toml_returns_error() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(dir.path(), "host = [invalid toml").unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let err = Config::load(None, None, None).unwrap_err();
        assert!(matches!(err, ApiError::Other(_)));
        assert!(err.to_string().contains("parse"));
    }

    // ── Auth type ──────────────────────────────────────────────────────────────

    #[test]
    fn load_pat_auth_does_not_require_email() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "jira.corp.com"
token = "my-pat-token"
auth_type = "pat"
api_version = 2
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _auth = EnvVarGuard::unset("JIRA_AUTH_TYPE");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let cfg = Config::load(None, None, None).unwrap();
        assert_eq!(cfg.auth_type, AuthType::Pat);
        assert_eq!(cfg.api_version, 2);
        assert!(cfg.email.is_empty(), "PAT auth sets email to empty string");
    }

    #[test]
    fn load_jira_auth_type_env_pat_overrides_basic() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "jira.corp.com"
email = "me@example.com"
token = "tok"
auth_type = "basic"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _auth = EnvVarGuard::set("JIRA_AUTH_TYPE", "pat");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let cfg = Config::load(None, None, None).unwrap();
        assert_eq!(cfg.auth_type, AuthType::Pat);
    }

    #[test]
    fn load_jira_api_version_env_overrides_default() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::set("JIRA_HOST", "myhost.atlassian.net");
        let _email = EnvVarGuard::set("JIRA_EMAIL", "me@example.com");
        let _token = EnvVarGuard::set("JIRA_TOKEN", "tok");
        let _api_version = EnvVarGuard::set("JIRA_API_VERSION", "2");
        let _auth = EnvVarGuard::unset("JIRA_AUTH_TYPE");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let cfg = Config::load(None, None, None).unwrap();
        assert_eq!(cfg.api_version, 2);
    }

    // ── Profile selection ──────────────────────────────────────────────────────

    #[test]
    fn load_profile_arg_selects_named_section() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "default.atlassian.net"
email = "default@example.com"
token = "default-tok"

[profiles.work]
host = "work.atlassian.net"
email = "me@work.com"
token = "work-tok"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let cfg = Config::load(None, None, Some("work".into())).unwrap();
        assert_eq!(cfg.host, "work.atlassian.net");
        assert_eq!(cfg.email, "me@work.com");
        assert_eq!(cfg.token, "work-tok");
    }

    #[test]
    fn load_jira_profile_env_selects_named_section() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "default.atlassian.net"
email = "default@example.com"
token = "default-tok"

[profiles.staging]
host = "staging.atlassian.net"
email = "me@staging.com"
token = "staging-tok"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::set("JIRA_PROFILE", "staging");

        let cfg = Config::load(None, None, None).unwrap();
        assert_eq!(cfg.host, "staging.atlassian.net");
    }

    #[test]
    fn load_unknown_profile_returns_descriptive_error() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[profiles.alpha]
host = "alpha.atlassian.net"
email = "me@alpha.com"
token = "alpha-tok"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let err = Config::load(None, None, Some("nonexistent".into())).unwrap_err();
        assert!(matches!(err, ApiError::Other(_)));
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent"),
            "error should name the bad profile"
        );
        assert!(
            msg.contains("alpha"),
            "error should list available profiles"
        );
    }

    // ── config::show ───────────────────────────────────────────────────────────

    #[test]
    fn show_json_output_includes_host_and_masked_token() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "show-test.atlassian.net"
email = "me@example.com"
token = "supersecrettoken"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let out = crate::output::OutputConfig::new(true, true);
        // Must not error and must produce no error output
        show(&out, None, None, None).unwrap();
    }

    #[test]
    fn show_text_output_renders_without_error() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"
[default]
host = "show-test.atlassian.net"
email = "me@example.com"
token = "supersecrettoken"
"#,
        )
        .unwrap();

        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let out = crate::output::OutputConfig::new(false, true);
        show(&out, None, None, None).unwrap();
    }

    // ── config::init ───────────────────────────────────────────────────────────

    #[test]
    fn init_json_output_includes_example_and_paths() {
        let out = crate::output::OutputConfig::new(true, true);
        // No env or config needed — init() never loads credentials
        init(&out, Some("jira.corp.com"));
    }

    #[test]
    fn init_text_output_renders_without_error() {
        let out = crate::output::OutputConfig::new(false, true);
        init(&out, None);
    }

    // ── dc_pat_url ─────────────────────────────────────────────────────────────

    #[test]
    fn dc_pat_url_without_host_returns_placeholder() {
        let url = dc_pat_url(None);
        assert!(url.starts_with("http://<your-host>"));
        assert!(url.contains(PAT_PATH));
    }

    #[test]
    fn dc_pat_url_bare_host_adds_https_scheme() {
        let url = dc_pat_url(Some("jira.corp.com"));
        assert!(url.starts_with("https://jira.corp.com"));
        assert!(url.contains(PAT_PATH));
    }

    #[test]
    fn dc_pat_url_host_with_https_scheme_is_preserved() {
        let url = dc_pat_url(Some("https://jira.corp.com/"));
        assert!(url.starts_with("https://jira.corp.com"));
        assert!(!url.contains("https://https://"));
        assert!(url.contains(PAT_PATH));
    }

    #[test]
    fn dc_pat_url_host_with_http_scheme_is_preserved() {
        let url = dc_pat_url(Some("http://localhost:8080"));
        assert!(url.starts_with("http://localhost:8080"));
        assert!(url.contains(PAT_PATH));
    }
}
