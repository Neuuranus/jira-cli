use jira_cli::api::{ApiError, JiraClient};
use jira_cli::commands;
use jira_cli::config::Config;
use jira_cli::output::{OutputConfig, exit_code_for_error};

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(name = "jira", version, about = "CLI for Jira")]
struct Cli {
    /// Atlassian domain (e.g. mycompany.atlassian.net) [env: JIRA_HOST]
    #[arg(long, env = "JIRA_HOST")]
    host: Option<String>,

    /// Account email [env: JIRA_EMAIL]
    #[arg(long, env = "JIRA_EMAIL")]
    email: Option<String>,

    /// Config profile to use [env: JIRA_PROFILE]
    #[arg(long, env = "JIRA_PROFILE")]
    profile: Option<String>,

    /// Output as JSON (auto-enabled when stdout is not a terminal)
    #[arg(long, global = true)]
    json: bool,

    /// Suppress non-data output (counts, confirmations)
    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage issues
    #[command(subcommand)]
    Issues(IssuesCommand),

    /// List projects
    #[command(subcommand)]
    Projects(ProjectsCommand),

    /// Search issues with JQL
    Search {
        /// JQL query string
        jql: String,

        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,

        /// Skip the first N results (for pagination)
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    /// Show the currently authenticated user
    Myself,

    /// Manage configuration
    #[command(subcommand)]
    Config(ConfigCommand),

    /// Bootstrap config and API token setup (alias for `config init`)
    Init,

    /// Dump all commands and arguments as JSON for agent introspection
    Schema,

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
        /// Install completions for supported shells (bash, zsh, fish)
        #[arg(long)]
        install: bool,
    },
}

#[derive(Subcommand)]
enum IssuesCommand {
    /// List issues
    List {
        /// Filter by project key
        #[arg(short, long)]
        project: Option<String>,

        /// Filter by status (e.g. "In Progress", "Done")
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by assignee (use "me" for current user)
        #[arg(short, long)]
        assignee: Option<String>,

        /// Filter by sprint name or use "active" for open sprints
        #[arg(long)]
        sprint: Option<String>,

        /// Additional JQL to append
        #[arg(long)]
        jql: Option<String>,

        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,

        /// Skip the first N results (for pagination)
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    /// Show a single issue in detail
    Show {
        /// Issue key (e.g. PROJ-123)
        key: String,

        /// Open the issue in your default browser
        #[arg(long)]
        open: bool,
    },

    /// Create a new issue
    Create {
        /// Project key
        #[arg(short, long)]
        project: String,

        /// Issue type (e.g. Bug, Story, Task)
        #[arg(short = 't', long, default_value = "Task")]
        issue_type: String,

        /// Issue summary
        #[arg(short, long)]
        summary: String,

        /// Issue description (plain text; newlines become separate paragraphs)
        #[arg(short, long)]
        description: Option<String>,

        /// Priority (e.g. High, Medium, Low)
        #[arg(long)]
        priority: Option<String>,

        /// Labels to apply (can be specified multiple times)
        #[arg(long)]
        labels: Vec<String>,

        /// Assign to this account ID (use "me" for yourself)
        #[arg(long)]
        assignee: Option<String>,
    },

    /// Update fields on an existing issue
    Update {
        /// Issue key (e.g. PROJ-123)
        key: String,

        /// New summary text
        #[arg(long)]
        summary: Option<String>,

        /// New description (plain text)
        #[arg(long)]
        description: Option<String>,

        /// New priority (e.g. High, Medium, Low)
        #[arg(long)]
        priority: Option<String>,
    },

    /// Add a comment to an issue
    Comment {
        /// Issue key (e.g. PROJ-123)
        key: String,

        /// Comment body (plain text)
        #[arg(short, long)]
        body: String,
    },

    /// Move an issue to a new status
    Transition {
        /// Issue key (e.g. PROJ-123)
        key: String,

        /// Target status name or transition ID
        #[arg(long)]
        to: String,
    },

    /// List available transitions for an issue
    ListTransitions {
        /// Issue key (e.g. PROJ-123)
        key: String,
    },

    /// Assign an issue to a user
    Assign {
        /// Issue key (e.g. PROJ-123)
        key: String,

        /// Account ID, "me" for yourself, or "none" to unassign
        #[arg(long)]
        assignee: String,
    },
}

#[derive(Subcommand)]
enum ProjectsCommand {
    /// List accessible projects
    List,
    /// Show details for a single project
    Show {
        /// Project key (e.g. PROJ)
        key: String,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Show current config (token masked)
    Show,
    /// Print example config file and token instructions
    Init,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let out = OutputConfig::new(cli.json, cli.quiet);

    let result = run(cli, out).await;

    if let Err(ref e) = result {
        eprintln!("Error: {e}");
        std::process::exit(exit_code_for_error(e.as_ref()));
    }
}

async fn run(cli: Cli, out: OutputConfig) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Command::Schema => {
            print_schema();
            return Ok(());
        }

        Command::Completions { shell, install } => {
            handle_completions(shell, install, &out)?;
            return Ok(());
        }

        Command::Init => {
            jira_cli::config::init(&out);
            return Ok(());
        }

        Command::Config(cmd) => {
            match cmd {
                ConfigCommand::Show => {
                    jira_cli::config::show(&out, cli.host, cli.email, cli.profile)?;
                }
                ConfigCommand::Init => {
                    jira_cli::config::init(&out);
                }
            }
            return Ok(());
        }

        _ => {}
    }

    let cfg = Config::load(cli.host, cli.email, cli.profile)?;
    let client = JiraClient::new(
        &cfg.host,
        &cfg.email,
        &cfg.token,
        cfg.auth_type,
        cfg.api_version,
    )?;

    match cli.command {
        Command::Issues(cmd) => match cmd {
            IssuesCommand::List {
                project,
                status,
                assignee,
                sprint,
                jql,
                limit,
                offset,
            } => {
                commands::issues::list(
                    &client,
                    &out,
                    project.as_deref(),
                    status.as_deref(),
                    assignee.as_deref(),
                    sprint.as_deref(),
                    jql.as_deref(),
                    limit,
                    offset,
                )
                .await?
            }
            IssuesCommand::Show { key, open } => {
                commands::issues::show(&client, &out, &key, open).await?
            }
            IssuesCommand::Create {
                project,
                issue_type,
                summary,
                description,
                priority,
                labels,
                assignee,
            } => {
                let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                let labels_opt = if label_refs.is_empty() {
                    None
                } else {
                    Some(label_refs.as_slice())
                };
                let assignee_str = match assignee.as_deref() {
                    Some("me") => {
                        let me = client.get_myself().await?;
                        Some(me.account_id)
                    }
                    Some(id) => Some(id.to_string()),
                    None => None,
                };
                commands::issues::create(
                    &client,
                    &out,
                    &project,
                    &issue_type,
                    &summary,
                    description.as_deref(),
                    priority.as_deref(),
                    labels_opt,
                    assignee_str.as_deref(),
                )
                .await?
            }
            IssuesCommand::Update {
                key,
                summary,
                description,
                priority,
            } => {
                commands::issues::update(
                    &client,
                    &out,
                    &key,
                    summary.as_deref(),
                    description.as_deref(),
                    priority.as_deref(),
                )
                .await?
            }
            IssuesCommand::Comment { key, body } => {
                commands::issues::comment(&client, &out, &key, &body).await?
            }
            IssuesCommand::Transition { key, to } => {
                commands::issues::transition(&client, &out, &key, &to).await?
            }
            IssuesCommand::ListTransitions { key } => {
                commands::issues::list_transitions(&client, &out, &key).await?
            }
            IssuesCommand::Assign { key, assignee } => {
                commands::issues::assign(&client, &out, &key, &assignee).await?
            }
        },

        Command::Projects(cmd) => match cmd {
            ProjectsCommand::List => commands::projects::list(&client, &out).await?,
            ProjectsCommand::Show { key } => commands::projects::show(&client, &out, &key).await?,
        },

        Command::Search { jql, limit, offset } => {
            commands::search::run(&client, &out, &jql, limit, offset).await?
        }

        Command::Myself => commands::myself::show(&client, &out).await?,

        // Already handled above
        Command::Schema | Command::Completions { .. } | Command::Config(_) | Command::Init => {}
    }

    Ok(())
}

fn print_schema() {
    println!(
        "{}",
        serde_json::to_string_pretty(&schema_json()).expect("failed to serialize schema")
    );
}

fn schema_json() -> serde_json::Value {
    let config_path = jira_cli::config::schema_config_path();
    let config_path_description = jira_cli::config::schema_config_path_description();
    let permission_advice = jira_cli::config::schema_recommended_permissions_example();
    serde_json::json!({
        "name": "jira",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "CLI for Jira — optimized for humans and agents",
        "auth": {
            "note": format!(
                "Provide host and email via CLI flags, environment variables, or the config file at {config_path}. Provide the API token via JIRA_TOKEN or that config file."
            ),
            "token_instructions": "https://id.atlassian.com/manage-profile/security/api-tokens",
            "required_fields": ["host", "email", "token"],
            "config_file": {
                "path": config_path,
                "description": config_path_description,
                "profile_selector": {
                    "flag": "--profile",
                    "env": "JIRA_PROFILE"
                }
            },
            "resolution_order": {
                "host": ["--host", "JIRA_HOST", "config profile/default host"],
                "email": ["--email", "JIRA_EMAIL", "config profile/default email"],
                "token": ["JIRA_TOKEN", "config profile/default token"],
                "auth_type": ["JIRA_AUTH_TYPE", "config profile/default auth_type"],
                "api_version": ["JIRA_API_VERSION", "config profile/default api_version"]
            },
            "env": [
                { "name": "JIRA_HOST", "description": "Atlassian domain override", "required": false },
                { "name": "JIRA_EMAIL", "description": "Account email override (not required when auth_type=pat)", "required": false },
                { "name": "JIRA_TOKEN", "description": "API token override (env/config only)", "required": false },
                { "name": "JIRA_PROFILE", "description": "Config profile", "required": false },
                { "name": "JIRA_AUTH_TYPE", "description": "Authentication type: 'basic' (default, Jira Cloud) or 'pat' (Personal Access Token, Jira Data Center/Server)", "required": false },
                { "name": "JIRA_API_VERSION", "description": "Jira REST API version: 3 (default, Cloud) or 2 (Data Center/Server)", "required": false }
            ]
        },
        "global_flags": [
            { "name": "--host", "env": "JIRA_HOST", "description": "Atlassian domain", "required": false },
            { "name": "--email", "env": "JIRA_EMAIL", "description": "Account email (not required when auth_type=pat)", "required": false },
            { "name": "--profile", "env": "JIRA_PROFILE", "description": "Config profile", "required": false },
            { "name": "--json", "description": "Force JSON output (auto when stdout is not a TTY)", "required": false },
            { "name": "--quiet", "description": "Suppress non-data output", "required": false },
        ],
        "exit_codes": {
            "0": "success",
            "1": "general / unexpected error",
            "2": "bad user input or config error",
            "3": "authentication failed",
            "4": "resource not found",
            "5": "Jira API error",
            "6": "rate limited"
        },
        "json_notes": {
            "assignee_field": "JSON assignee is { displayName, accountId }. Use accountId with 'issues assign --assignee'.",
            "type_field": "JSON 'type' is normalized from Jira's 'issuetype' field.",
            "pagination": "'issues list' and 'search' JSON includes total/startAt/maxResults for pagination. Use --offset to page."
        },
        "commands": [
            {
                "name": "issues list",
                "description": "List issues with optional filters; results ordered by last updated",
                "flags": [
                    { "name": "--project", "short": "-p", "description": "Filter by project key", "required": false },
                    { "name": "--status", "short": "-s", "description": "Filter by status", "required": false },
                    { "name": "--assignee", "short": "-a", "description": "Filter by assignee ('me' = current user)", "required": false },
                    { "name": "--sprint", "description": "Filter by sprint name or 'active' for open sprints", "required": false },
                    { "name": "--jql", "description": "Additional JQL clause to append", "required": false },
                    { "name": "--limit", "short": "-n", "default": 50, "description": "Maximum results", "required": false },
                    { "name": "--offset", "default": 0, "description": "Skip first N results for pagination", "required": false },
                ],
                "json_shape": { "total": "N", "startAt": 0, "maxResults": 50, "issues": "[...]" }
            },
            {
                "name": "issues show <key>",
                "description": "Show full issue detail including description and all comments",
                "args": [{ "name": "key", "description": "Issue key, e.g. PROJ-123", "required": true }],
                "flags": [
                    { "name": "--open", "description": "Open the issue in your default browser", "required": false }
                ]
            },
            {
                "name": "issues create",
                "description": "Create a new issue. Returns key, id, url.",
                "flags": [
                    { "name": "--project", "short": "-p", "description": "Project key", "required": true },
                    { "name": "--issue-type", "short": "-t", "default": "Task", "description": "Issue type", "required": false },
                    { "name": "--summary", "short": "-s", "description": "Issue summary", "required": true },
                    { "name": "--description", "short": "-d", "description": "Issue description (plain text)", "required": false },
                    { "name": "--priority", "description": "Priority (e.g. High, Medium, Low)", "required": false },
                    { "name": "--labels", "description": "Labels to apply (repeatable)", "required": false },
                    { "name": "--assignee", "description": "Account ID or 'me' to self-assign", "required": false },
                ]
            },
            {
                "name": "issues update <key>",
                "description": "Update fields on an existing issue. At least one field required.",
                "args": [{ "name": "key", "description": "Issue key", "required": true }],
                "flags": [
                    { "name": "--summary", "description": "New summary", "required": false },
                    { "name": "--description", "description": "New description (plain text)", "required": false },
                    { "name": "--priority", "description": "New priority (e.g. High, Medium, Low)", "required": false },
                ]
            },
            {
                "name": "issues comment <key>",
                "description": "Add a comment. Returns id, url, author, created.",
                "args": [{ "name": "key", "description": "Issue key", "required": true }],
                "flags": [
                    { "name": "--body", "short": "-b", "description": "Comment body (plain text)", "required": true },
                ]
            },
            {
                "name": "issues transition <key>",
                "description": "Move an issue to a new workflow status. Matches by name (case-insensitive) or ID.",
                "args": [{ "name": "key", "description": "Issue key", "required": true }],
                "flags": [
                    { "name": "--to", "description": "Target status name or transition ID", "required": true },
                ]
            },
            {
                "name": "issues list-transitions <key>",
                "description": "List available workflow transitions. Use before 'issues transition' if unsure of names.",
                "args": [{ "name": "key", "description": "Issue key", "required": true }],
                "json_shape": [{ "id": "21", "name": "In Progress", "to": { "name": "In Progress", "statusCategory": { "key": "indeterminate", "name": "In Progress" } } }]
            },
            {
                "name": "issues assign <key>",
                "description": "Assign an issue. Use 'me' to self-assign, 'none' to unassign, or an accountId.",
                "args": [{ "name": "key", "description": "Issue key", "required": true }],
                "flags": [
                    { "name": "--assignee", "description": "accountId, 'me', or 'none'", "required": true },
                ]
            },
            {
                "name": "projects list",
                "description": "List all accessible Jira projects (all pages fetched automatically)",
                "json_shape": { "total": "N", "projects": "[{ key, name, id, type }]" }
            },
            {
                "name": "projects show <key>",
                "description": "Show details for a single project",
                "args": [{ "name": "key", "description": "Project key", "required": true }]
            },
            {
                "name": "search <jql>",
                "description": "Search issues with raw JQL. JQL is passed verbatim — no ORDER BY is appended. Same JSON shape as 'issues list'.",
                "args": [{ "name": "jql", "description": "JQL query string", "required": true }],
                "flags": [
                    { "name": "--limit", "short": "-n", "default": 50, "description": "Maximum results", "required": false },
                    { "name": "--offset", "default": 0, "description": "Skip first N results for pagination", "required": false },
                ]
            },
            {
                "name": "myself",
                "description": "Show the authenticated user's accountId and displayName. Use accountId with 'issues assign --assignee'.",
                "json_shape": { "accountId": "...", "displayName": "..." }
            },
            {
                "name": "config show",
                "description": "Show resolved config (token masked)",
                "json_shape": {
                    "configPath": "/path/to/config.toml",
                    "host": "example.atlassian.net",
                    "email": "me@example.com",
                    "tokenMasked": "***abcd"
                }
            },
            {
                "name": "config init",
                "description": "Print or emit example config file and API token instructions",
                "json_shape": {
                    "configPath": "/path/to/config.toml",
                    "pathResolution": config_path_description,
                    "tokenInstructions": "https://id.atlassian.com/manage-profile/security/api-tokens",
                    "recommendedPermissions": permission_advice,
                    "example": {
                        "default": { "host": "...", "email": "...", "token": "..." },
                        "profiles": { "work": { "host": "...", "email": "...", "token": "..." } }
                    }
                }
            },
            {
                "name": "init",
                "description": "Alias for `config init`",
                "alias_for": "config init",
                "json_shape": {
                    "configPath": "/path/to/config.toml",
                    "pathResolution": config_path_description,
                    "tokenInstructions": "https://id.atlassian.com/manage-profile/security/api-tokens",
                    "recommendedPermissions": permission_advice,
                    "example": {
                        "default": { "host": "...", "email": "...", "token": "..." },
                        "profiles": { "work": { "host": "...", "email": "...", "token": "..." } }
                    }
                }
            },
            {
                "name": "schema",
                "description": "Dump this document as JSON for agent introspection"
            },
            {
                "name": "completions <shell>",
                "description": "Generate shell completions",
                "args": [{ "name": "shell", "description": "bash, zsh, fish, powershell, or elvish", "required": true }],
                "flags": [
                    { "name": "--install", "description": "Install completions for supported shells (bash, zsh, fish). PowerShell and elvish must be redirected manually.", "required": false }
                ]
            },
        ]
    })
}

fn handle_completions(
    shell: Shell,
    install: bool,
    out: &OutputConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    use clap_complete::generate;
    use std::io;

    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();

    if install {
        let (path, mut writer, note) = match shell {
            Shell::Bash => {
                let p = bash_completion_path()?;
                let writer = create_completion_writer(&p)?;
                let note = format!(
                    "Generated completion file at {}. Source it from your shell startup if ~/.bash_completion.d is not loaded automatically.",
                    p.display()
                );
                (p, writer, note)
            }
            Shell::Zsh => {
                let p = zsh_completion_path()?;
                let writer = create_completion_writer(&p)?;
                let note = format!(
                    "Generated completion file at {}. Ensure its parent directory is in `fpath`, then run `autoload -Uz compinit && compinit`.",
                    p.display()
                );
                (p, writer, note)
            }
            Shell::Fish => {
                let p = fish_completion_path()?;
                let writer = create_completion_writer(&p)?;
                let note = format!(
                    "Generated completion file at {}. Fish loads this path automatically.",
                    p.display()
                );
                (p, writer, note)
            }
            Shell::PowerShell => {
                return Err(ApiError::InvalidInput(
                    "`jira completions powershell --install` is not supported. Redirect `jira completions powershell` into your PowerShell profile or completion path manually.".into(),
                )
                .into());
            }
            _ => {
                let shell_name = shell.to_string();
                return Err(ApiError::InvalidInput(format!(
                    "`jira completions {shell_name} --install` is not supported. Redirect `jira completions {shell_name}` into your shell completion path manually."
                ))
                .into());
            }
        };
        generate(shell, &mut cmd, bin_name, &mut writer);
        out.print_message(&note);
        out.print_message(&format!("Completion file path: {}", path.display()));
    } else {
        generate(shell, &mut cmd, bin_name, &mut io::stdout());
    }
    Ok(())
}

fn create_completion_writer(path: &std::path::Path) -> Result<Box<dyn std::io::Write>, ApiError> {
    let parent = path.parent().unwrap_or(path);
    std::fs::create_dir_all(parent)
        .map_err(|e| ApiError::Other(format!("cannot create {}: {e}", parent.display())))?;
    let file = std::fs::File::create(path)
        .map_err(|e| ApiError::Other(format!("cannot write {}: {e}", path.display())))?;
    Ok(Box::new(file) as Box<dyn std::io::Write>)
}

fn home_dir() -> Result<std::path::PathBuf, ApiError> {
    dirs::home_dir().ok_or_else(|| ApiError::Other("cannot determine home directory".into()))
}

fn bash_completion_path() -> Result<std::path::PathBuf, ApiError> {
    Ok(home_dir()?.join(".bash_completion.d").join("jira"))
}

fn zsh_completion_path() -> Result<std::path::PathBuf, ApiError> {
    Ok(home_dir()?.join(".zsh").join("completions").join("_jira"))
}

fn fish_completion_path() -> Result<std::path::PathBuf, ApiError> {
    #[cfg(target_os = "windows")]
    let base = dirs::config_dir().ok_or_else(|| {
        ApiError::Other("cannot determine config directory for fish completions".into())
    })?;

    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or(home_dir()?.join(".config"));

    Ok(base.join("fish").join("completions").join("jira.fish"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jira_cli::api::ApiError;
    use jira_cli::test_support::{EnvVarGuard, ProcessEnvLock, set_config_dir_env};
    use tempfile::TempDir;

    #[test]
    fn schema_does_not_advertise_nonexistent_token_flag() {
        let schema = schema_json();
        let global_flags = schema["global_flags"].as_array().unwrap();
        assert!(
            !global_flags.iter().any(|flag| flag["name"] == "--token"),
            "schema must not invent a --token CLI flag"
        );

        let auth_env = schema["auth"]["env"].as_array().unwrap();
        assert!(
            auth_env.iter().any(|entry| entry["name"] == "JIRA_TOKEN"),
            "schema must still document JIRA_TOKEN as an auth source"
        );
    }

    #[test]
    fn schema_auth_describes_runtime_config_path_and_effective_requirements() {
        let schema = schema_json();
        let auth = &schema["auth"];

        assert_eq!(
            auth["config_file"]["path"].as_str(),
            Some(jira_cli::config::schema_config_path().as_str())
        );
        assert_eq!(
            auth["config_file"]["description"].as_str(),
            Some(jira_cli::config::schema_config_path_description())
        );
        assert_eq!(
            auth["required_fields"],
            serde_json::json!(["host", "email", "token"])
        );

        let auth_env = auth["env"].as_array().unwrap();
        assert!(
            auth_env.iter().all(|entry| entry["required"] == false),
            "individual env vars are optional auth sources, not mandatory on their own"
        );
    }

    #[test]
    fn schema_config_init_uses_platform_specific_bootstrap_guidance() {
        let schema = schema_json();
        let config_init = schema["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["name"] == "config init")
            .unwrap();

        assert_eq!(
            config_init["json_shape"]["pathResolution"].as_str(),
            Some(jira_cli::config::schema_config_path_description())
        );
        assert_eq!(
            config_init["json_shape"]["recommendedPermissions"].as_str(),
            Some(jira_cli::config::schema_recommended_permissions_example())
        );
    }

    #[test]
    fn config_show_propagates_invalid_config_as_error() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let dir = TempDir::new().unwrap();
        let _config_dir = set_config_dir_env(dir.path());
        let _host = EnvVarGuard::unset("JIRA_HOST");
        let _email = EnvVarGuard::unset("JIRA_EMAIL");
        let _token = EnvVarGuard::unset("JIRA_TOKEN");
        let _profile = EnvVarGuard::unset("JIRA_PROFILE");

        let err =
            jira_cli::config::show(&OutputConfig::new(true, true), None, None, None).unwrap_err();
        assert!(matches!(err, ApiError::InvalidInput(_)));
    }
}
