#![recursion_limit = "256"]

use jira_cli::api::{ApiError, IssueDraft, IssueUpdate, JiraClient};
use jira_cli::commands;
use jira_cli::config::Config;
use jira_cli::output::{OutputConfig, exit_code_for_error};

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

fn parse_field(s: &str) -> Result<(String, serde_json::Value), String> {
    let (key, raw) = s
        .split_once('=')
        .ok_or_else(|| format!("field must be in key=value format, got: {s}"))?;
    // Try to parse as JSON (handles numbers, booleans, objects, arrays).
    // Fall back to a plain string.
    let value =
        serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()));
    Ok((key.to_string(), value))
}

/// Parse a repeated `--components` `Vec<String>` into the three-state form
/// expected by `JiraClient::update_issue`:
///   - `None` if no values were given (leave field untouched)
///   - `Some(vec![])` if the single value is the `"none"` sentinel (clear field)
///   - `Some(refs)` otherwise (set field to these names)
fn parse_components_update_arg(values: &[String]) -> Option<Vec<&str>> {
    if values.is_empty() {
        return None;
    }
    if values.len() == 1 && values[0] == "none" {
        return Some(Vec::new());
    }
    Some(values.iter().map(String::as_str).collect())
}

/// Parse a repeated `--fix-versions` `Vec<String>` into the three-state form.
/// Sentinel semantics are identical to `parse_components_update_arg`.
fn parse_fix_versions_update_arg(values: &[String]) -> Option<Vec<&str>> {
    if values.is_empty() {
        return None;
    }
    if values.len() == 1 && values[0] == "none" {
        return Some(Vec::new());
    }
    Some(values.iter().map(String::as_str).collect())
}

/// Parse `--labels` for update: same three-state sentinel contract.
fn parse_labels_update_arg(values: &[String]) -> Option<Vec<&str>> {
    if values.is_empty() {
        return None;
    }
    if values.len() == 1 && values[0] == "none" {
        return Some(Vec::new());
    }
    Some(values.iter().map(String::as_str).collect())
}

/// Convert a `Vec<String>` of CLI-repeated values into an `Option<Vec<&str>>`.
/// `None` if empty, `Some(refs)` otherwise. The caller then `as_deref()`s into
/// `Option<&[&str]>` for the API layer.
fn vec_to_opt_refs(values: &[String]) -> Option<Vec<&str>> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().map(String::as_str).collect())
    }
}

#[derive(Parser)]
#[command(
    name = "jira",
    version,
    about = "CLI for Jira",
    arg_required_else_help = true
)]
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
    #[command(subcommand, visible_alias = "issue")]
    Issues(Box<IssuesCommand>),

    /// List projects
    #[command(subcommand, visible_alias = "project", arg_required_else_help = true)]
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

        /// Fetch all pages (overrides --limit and --offset)
        #[arg(long)]
        all: bool,
    },

    /// Search for users by name or email
    #[command(subcommand, visible_alias = "user", arg_required_else_help = true)]
    Users(UsersCommand),

    /// List boards
    #[command(subcommand, visible_alias = "board", arg_required_else_help = true)]
    Boards(BoardsCommand),

    /// List sprints
    #[command(subcommand, visible_alias = "sprint", arg_required_else_help = true)]
    Sprints(SprintsCommand),

    /// Show the currently authenticated user
    Myself,

    /// Manage configuration
    #[command(subcommand)]
    Config(ConfigCommand),

    /// Bootstrap config and API token setup (alias for `config init`)
    Init,

    /// List available fields (system and custom)
    #[command(subcommand, visible_alias = "field", arg_required_else_help = true)]
    Fields(FieldsCommand),

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

        /// Filter by issue type (e.g. Bug, Story, Task)
        #[arg(short = 't', long = "type")]
        issue_type: Option<String>,

        /// Filter by sprint name or use "active" for open sprints
        #[arg(long)]
        sprint: Option<String>,

        /// Filter by component (can be specified multiple times)
        #[arg(long)]
        components: Vec<String>,

        /// Filter by label (can be specified multiple times)
        #[arg(long)]
        labels: Vec<String>,

        /// Additional JQL to append
        #[arg(long)]
        jql: Option<String>,

        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,

        /// Skip the first N results (for pagination)
        #[arg(long, default_value = "0")]
        offset: usize,

        /// Fetch all pages (overrides --limit and --offset)
        #[arg(long)]
        all: bool,
    },

    /// List issues assigned to you
    Mine {
        /// Filter by project key
        #[arg(short, long)]
        project: Option<String>,

        /// Filter by status (e.g. "In Progress", "Done")
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by issue type (e.g. Bug, Story, Task)
        #[arg(short = 't', long)]
        issue_type: Option<String>,

        /// Filter by sprint name or use "active" for open sprints
        #[arg(long)]
        sprint: Option<String>,

        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,

        /// Fetch all pages (overrides --limit)
        #[arg(long)]
        all: bool,
    },

    /// List comments on an issue
    Comments {
        /// Issue key (e.g. PROJ-123)
        key: String,
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
        #[arg(short = 't', long = "type", default_value = "Task")]
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

        /// Components to attach (can be specified multiple times)
        #[arg(long)]
        components: Vec<String>,

        /// Fix versions to set (can be specified multiple times)
        #[arg(long)]
        fix_versions: Vec<String>,

        /// Assign to this account ID (use "me" for yourself)
        #[arg(long)]
        assignee: Option<String>,

        /// Add to a sprint (sprint ID, name substring, or "active")
        #[arg(long)]
        sprint: Option<String>,

        /// Parent issue key (creates a subtask or child issue)
        #[arg(long)]
        parent: Option<String>,

        /// Custom field values as key=value pairs (e.g. --field customfield_10016=5)
        #[arg(long, value_parser = parse_field)]
        field: Vec<(String, serde_json::Value)>,
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

        /// Components to set (replaces existing; use "none" alone to clear)
        #[arg(long)]
        components: Vec<String>,

        /// Fix versions to set (replaces existing; use "none" alone to clear)
        #[arg(long)]
        fix_versions: Vec<String>,

        /// Labels to set (replaces existing; use "none" alone to clear)
        #[arg(long)]
        labels: Vec<String>,

        /// Assign to this account ID (use "me" for yourself, "none" to unassign)
        #[arg(long)]
        assignee: Option<String>,

        /// Custom field values as key=value pairs (e.g. --field customfield_10016=5)
        #[arg(long, value_parser = parse_field)]
        field: Vec<(String, serde_json::Value)>,
    },

    /// Move an issue to a sprint
    Move {
        /// Issue key (e.g. PROJ-123)
        key: String,

        /// Sprint ID, sprint name substring, or "active"
        #[arg(long)]
        sprint: String,
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

    /// List available issue link types
    LinkTypes,

    /// Link this issue to another issue
    Link {
        /// Issue key (e.g. PROJ-123)
        key: String,

        /// Key of the issue to link to
        #[arg(long)]
        to: String,

        /// Link type name (e.g. "Blocks", "Duplicate", "Relates")
        #[arg(long, default_value = "Relates")]
        link_type: String,
    },

    /// Remove a link between issues by link ID
    Unlink {
        /// Link ID (shown in `issues show` output and JSON)
        link_id: String,
    },

    /// Log work (time) on an issue
    LogWork {
        /// Issue key (e.g. PROJ-123)
        key: String,

        /// Time spent (e.g. 2h, 30m, 1d 4h)
        #[arg(short, long)]
        time: String,

        /// Comment describing the work done
        #[arg(short, long)]
        comment: Option<String>,

        /// When the work was started (ISO-8601, e.g. 2024-01-15T09:00:00.000+0000)
        #[arg(long)]
        started: Option<String>,
    },

    /// Transition all issues matching a JQL query to a new status
    BulkTransition {
        /// JQL query selecting the issues to transition
        #[arg(long)]
        jql: String,

        /// Target status name or transition ID
        #[arg(long)]
        to: String,

        /// Preview without making any changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Assign all issues matching a JQL query to a user
    BulkAssign {
        /// JQL query selecting the issues to assign
        #[arg(long)]
        jql: String,

        /// Account ID, "me" for yourself, or "none" to unassign
        #[arg(long)]
        assignee: String,

        /// Preview without making any changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Catch bare issue keys: `jira issue PROJ-123` → `jira issues show PROJ-123`
    #[command(external_subcommand)]
    External(Vec<String>),
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
    /// List components for a project
    Components {
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
    /// Remove a profile from the config file
    Remove {
        /// Profile name to remove (use "default" for the default profile)
        profile: String,
    },
}

#[derive(Subcommand)]
enum UsersCommand {
    /// Search for users by name or email
    Search {
        /// Name, username, or email fragment to search for
        query: String,
    },
}

#[derive(Subcommand)]
enum BoardsCommand {
    /// List all boards
    List,
}

#[derive(Subcommand)]
enum SprintsCommand {
    /// List sprints, optionally filtered by board and/or state
    List {
        /// Board name or ID (lists all boards if omitted)
        #[arg(long)]
        board: Option<String>,

        /// Filter by state: active (default), closed, future, or all
        #[arg(long, default_value = "active")]
        state: String,
    },
}

#[derive(Subcommand)]
enum FieldsCommand {
    /// List all fields with their IDs and types
    List {
        /// Show only custom fields
        #[arg(long)]
        custom: bool,
    },
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
            jira_cli::config::init(&out, cli.host.as_deref()).await;
            return Ok(());
        }

        Command::Config(cmd) => {
            match cmd {
                ConfigCommand::Show => {
                    jira_cli::config::show(&out, cli.host, cli.email, cli.profile)?;
                }
                ConfigCommand::Init => {
                    jira_cli::config::init(&out, cli.host.as_deref()).await;
                }
                ConfigCommand::Remove { profile } => {
                    jira_cli::config::remove_profile(&profile);
                }
            }
            return Ok(());
        }

        _ => {}
    }

    let cfg = Config::load(cli.host, cli.email, cli.profile)?;

    if cfg.read_only {
        let is_write = matches!(
            &cli.command,
            Command::Issues(cmd) if matches!(
                cmd.as_ref(),
                IssuesCommand::Create { .. }
                    | IssuesCommand::Update { .. }
                    | IssuesCommand::Move { .. }
                    | IssuesCommand::Comment { .. }
                    | IssuesCommand::Transition { .. }
                    | IssuesCommand::Assign { .. }
                    | IssuesCommand::Link { .. }
                    | IssuesCommand::Unlink { .. }
                    | IssuesCommand::LogWork { .. }
                    | IssuesCommand::BulkTransition { .. }
                    | IssuesCommand::BulkAssign { .. }
            )
        );
        if is_write {
            return Err(ApiError::InvalidInput(
                "read-only mode is enabled (unset JIRA_READ_ONLY or remove read_only from config to allow writes)".into(),
            )
            .into());
        }
    }

    let client = JiraClient::new(
        &cfg.host,
        &cfg.email,
        &cfg.token,
        cfg.auth_type,
        cfg.api_version,
    )?;

    match cli.command {
        Command::Issues(cmd) => match *cmd {
            IssuesCommand::List {
                project,
                status,
                assignee,
                issue_type,
                sprint,
                components,
                labels,
                jql,
                limit,
                offset,
                all,
            } => {
                let parsed_components = vec_to_opt_refs(&components);
                let parsed_labels = vec_to_opt_refs(&labels);
                let filters = commands::issues::ListFilters {
                    project: project.as_deref(),
                    status: status.as_deref(),
                    assignee: assignee.as_deref(),
                    issue_type: issue_type.as_deref(),
                    sprint: sprint.as_deref(),
                    components: parsed_components.as_deref(),
                    labels: parsed_labels.as_deref(),
                    jql_extra: jql.as_deref(),
                };
                commands::issues::list(&client, &out, filters, limit, offset, all).await?
            }
            IssuesCommand::Mine {
                project,
                status,
                issue_type,
                sprint,
                limit,
                all,
            } => {
                let filters = commands::issues::ListFilters {
                    project: project.as_deref(),
                    status: status.as_deref(),
                    issue_type: issue_type.as_deref(),
                    sprint: sprint.as_deref(),
                    ..Default::default()
                };
                commands::issues::mine(&client, &out, filters, limit, all).await?
            }
            IssuesCommand::Comments { key } => {
                commands::issues::comments(&client, &out, &key).await?
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
                components,
                fix_versions,
                assignee,
                sprint,
                parent,
                field,
            } => {
                let parsed_labels = vec_to_opt_refs(&labels);
                let parsed_components = vec_to_opt_refs(&components);
                let parsed_fix_versions = vec_to_opt_refs(&fix_versions);
                let assignee_str = match assignee.as_deref() {
                    Some("me") => {
                        let me = client.get_myself().await?;
                        Some(me.account_id)
                    }
                    Some(id) => Some(id.to_string()),
                    None => None,
                };
                let draft = IssueDraft {
                    project_key: &project,
                    issue_type: &issue_type,
                    summary: &summary,
                    description: description.as_deref(),
                    priority: priority.as_deref(),
                    labels: parsed_labels.as_deref(),
                    components: parsed_components.as_deref(),
                    fix_versions: parsed_fix_versions.as_deref(),
                    assignee: assignee_str.as_deref(),
                    parent: parent.as_deref(),
                };
                commands::issues::create(&client, &out, &draft, sprint.as_deref(), &field).await?
            }
            IssuesCommand::Update {
                key,
                summary,
                description,
                priority,
                components,
                fix_versions,
                labels,
                assignee,
                field,
            } => {
                let parsed_components = parse_components_update_arg(&components);
                let parsed_fix_versions = parse_fix_versions_update_arg(&fix_versions);
                let parsed_labels = parse_labels_update_arg(&labels);

                let resolved_assignee =
                    commands::issues::resolve_assignee_arg(&client, assignee.as_deref()).await?;
                let assignee_ref: Option<Option<&str>> =
                    resolved_assignee.as_ref().map(|inner| inner.as_deref());

                let update = IssueUpdate {
                    summary: summary.as_deref(),
                    description: description.as_deref(),
                    priority: priority.as_deref(),
                    components: parsed_components.as_deref(),
                    fix_versions: parsed_fix_versions.as_deref(),
                    labels: parsed_labels.as_deref(),
                    assignee: assignee_ref,
                };
                commands::issues::update(&client, &out, &key, &update, &field).await?
            }
            IssuesCommand::Move { key, sprint } => {
                commands::issues::move_to_sprint(&client, &out, &key, &sprint).await?
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
            IssuesCommand::LinkTypes => commands::issues::link_types(&client, &out).await?,
            IssuesCommand::Link { key, to, link_type } => {
                commands::issues::link(&client, &out, &key, &to, &link_type).await?
            }
            IssuesCommand::Unlink { link_id } => {
                commands::issues::unlink(&client, &out, &link_id).await?
            }
            IssuesCommand::LogWork {
                key,
                time,
                comment,
                started,
            } => {
                commands::issues::log_work(
                    &client,
                    &out,
                    &key,
                    &time,
                    comment.as_deref(),
                    started.as_deref(),
                )
                .await?
            }
            IssuesCommand::BulkTransition { jql, to, dry_run } => {
                commands::issues::bulk_transition(&client, &out, &jql, &to, dry_run).await?
            }
            IssuesCommand::BulkAssign {
                jql,
                assignee,
                dry_run,
            } => commands::issues::bulk_assign(&client, &out, &jql, &assignee, dry_run).await?,
            IssuesCommand::External(args) => {
                let key = args
                    .first()
                    .ok_or_else(|| ApiError::InvalidInput("missing issue key".into()))?;
                let open = args.iter().any(|a| a == "--open");
                commands::issues::show(&client, &out, key, open).await?
            }
        },

        Command::Projects(cmd) => match cmd {
            ProjectsCommand::List => commands::projects::list(&client, &out).await?,
            ProjectsCommand::Show { key } => commands::projects::show(&client, &out, &key).await?,
            ProjectsCommand::Components { key } => {
                commands::projects::components(&client, &out, &key).await?
            }
        },

        Command::Users(cmd) => match cmd {
            UsersCommand::Search { query } => {
                commands::users::search(&client, &out, &query).await?
            }
        },

        Command::Boards(cmd) => match cmd {
            BoardsCommand::List => commands::boards::list(&client, &out).await?,
        },

        Command::Sprints(cmd) => match cmd {
            SprintsCommand::List { board, state } => {
                // "all" is a special token meaning no state filter.
                let state_filter = if state == "all" {
                    None
                } else {
                    Some(state.as_str())
                };
                commands::sprints::list(&client, &out, board.as_deref(), state_filter).await?
            }
        },

        Command::Search {
            jql,
            limit,
            offset,
            all,
        } => commands::search::run(&client, &out, &jql, limit, offset, all).await?,

        Command::Myself => commands::myself::show(&client, &out).await?,

        Command::Fields(cmd) => match cmd {
            FieldsCommand::List { custom } => commands::fields::list(&client, &out, custom).await?,
        },

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
    use std::collections::{HashMap, HashSet};

    let config_path = jira_cli::config::schema_config_path();
    let config_path_description = jira_cli::config::schema_config_path_description();
    let permission_advice = jira_cli::config::schema_recommended_permissions_example();

    // Annotations keyed by base command path (no <arg> suffixes).
    // Only things clap cannot express: json_shape and alias_for.
    let init_shape = serde_json::json!({
        "configPath": "/path/to/config.toml",
        "pathResolution": config_path_description,
        "tokenInstructions": "https://id.atlassian.com/manage-profile/security/api-tokens",
        "configExists": false,
        "recommendedPermissions": permission_advice,
        "example": {
            "default": { "host": "mycompany.atlassian.net", "email": "me@example.com", "token": "..." },
            "profiles": { "work": { "host": "...", "email": "...", "token": "..." } }
        }
    });

    let annotations: HashMap<&str, serde_json::Value> = [
        ("issues list", serde_json::json!({ "json_shape": {
            "total": "N", "startAt": 0, "maxResults": 50,
            "issues": "[{ key, id, url, summary, status, assignee: { displayName, accountId }, priority, type, created, updated }]"
        }})),
        ("issues show", serde_json::json!({ "json_shape": {
            "key": "PROJ-1", "id": "10001", "url": "https://...",
            "summary": "...", "status": "In Progress", "type": "Bug", "priority": "High",
            "assignee": { "displayName": "Alice", "accountId": "abc123" },
            "reporter": { "displayName": "Bob", "accountId": "xyz" },
            "labels": ["backend"], "components": [{ "id": "10010", "name": "Backend", "description": "Server-side" }],
            "fixVersions": [{ "id": "10010", "name": "1.2.0", "description": "...", "released": true, "archived": false, "releaseDate": "2024-03-01" }],
            "affectedVersions": [{ "id": "10005", "name": "1.1.0", "description": "...", "released": true, "archived": false, "releaseDate": "2024-02-01" }],
            "description": "...",
            "created": "2024-01-01", "updated": "2024-01-02",
            "comments": "[{ id, author: { displayName, accountId }, body, created, updated }]",
            "issueLinks": "[{ id, sentence, type: { name, inward, outward }, outwardIssue, inwardIssue }]"
        }})),
        ("issues create", serde_json::json!({ "json_shape": {
            "key": "PROJ-1", "id": "10001", "url": "https://...",
            "sprintId": "(present when --sprint used)", "sprintName": "(present when --sprint used)"
        }})),
        ("issues update", serde_json::json!({ "json_shape": { "key": "PROJ-1", "updated": true } })),
        ("issues move", serde_json::json!({ "json_shape": { "issue": "PROJ-1", "sprintId": 5, "sprintName": "Sprint 1" } })),
        ("issues comment", serde_json::json!({ "json_shape": {
            "id": "10042", "issue": "PROJ-1", "url": "https://...", "author": "Alice", "created": "2024-01-01"
        }})),
        ("issues transition", serde_json::json!({ "json_shape": {
            "issue": "PROJ-1", "transition": "Start Progress", "status": "In Progress", "id": "21"
        }})),
        ("issues list-transitions", serde_json::json!({ "json_shape": [
            { "id": "21", "name": "In Progress", "to": { "name": "In Progress", "statusCategory": { "key": "indeterminate", "name": "In Progress" } } }
        ]})),
        ("issues assign", serde_json::json!({ "json_shape": { "issue": "PROJ-1", "accountId": "abc123" } })),
        ("issues link-types", serde_json::json!({ "json_shape": [
            { "id": "1", "name": "Blocks", "inward": "is blocked by", "outward": "blocks" }
        ]})),
        ("issues link", serde_json::json!({ "json_shape": { "from": "PROJ-1", "to": "PROJ-2", "type": "Relates" } })),
        ("issues unlink", serde_json::json!({ "json_shape": { "linkId": "10001" } })),
        ("users search", serde_json::json!({ "json_shape": { "total": "N", "users": "[{ accountId, displayName, email }]" } })),
        ("boards list", serde_json::json!({ "json_shape": { "total": "N", "boards": "[{ id, name, type }]" } })),
        ("sprints list", serde_json::json!({ "json_shape": {
            "total": "N", "sprints": "[{ id, name, state, boardId, boardName, startDate, endDate, completeDate }]"
        }})),
        ("fields list", serde_json::json!({ "json_shape": { "total": "N", "fields": "[{ id, name, custom, type }]" } })),
        ("projects list", serde_json::json!({ "json_shape": { "total": "N", "projects": "[{ key, name, id, type }]" } })),
        ("projects show", serde_json::json!({ "json_shape": { "id": "10001", "key": "PROJ", "name": "My Project", "type": "software" } })),
        ("projects components", serde_json::json!({ "json_shape": {
            "project": "PROJ", "total": "N",
            "components": "[{ id, name, description }]"
        }})),
        ("search", serde_json::json!({ "json_shape": { "total": "N", "startAt": 0, "maxResults": 50, "issues": "[...]" } })),
        ("myself", serde_json::json!({ "json_shape": { "accountId": "abc123", "displayName": "Alice" } })),
        ("config show", serde_json::json!({ "json_shape": {
            "configPath": "/path/to/config.toml", "host": "example.atlassian.net",
            "email": "me@example.com", "tokenMasked": "***abcd"
        }})),
        ("config init", serde_json::json!({ "json_shape": init_shape })),
        ("init", serde_json::json!({ "alias_for": "config init", "json_shape": init_shape })),
        ("issue", serde_json::json!({ "alias_for": "issues show" })),
    ]
    .into_iter()
    .collect();

    // Arg IDs of global flags — excluded from per-command flag lists.
    let global_ids: HashSet<&str> = ["json", "quiet", "host", "email", "profile"]
        .iter()
        .copied()
        .collect();

    let root = Cli::command();
    let commands = walk_commands(&root, &[], &annotations, &global_ids);

    serde_json::json!({
        "name": "jira",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "CLI for Jira — optimized for humans and agents",
        "auth": {
            "note": format!(
                "Provide host and email via CLI flags, environment variables, or the config file at {config_path}. Provide the API token via JIRA_TOKEN or that config file."
            ),
            "token_instructions": "https://id.atlassian.com/manage-profile/security/api-tokens",
            "required_fields": ["host", "token"],
            "email_note": "email is required for basic auth (Jira Cloud) but not for pat auth (Jira Data Center/Server)",
            "config_file": {
                "path": config_path,
                "description": config_path_description,
                "profile_selector": { "flag": "--profile", "env": "JIRA_PROFILE" }
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
                { "name": "JIRA_EMAIL", "description": "Account email (not required when auth_type=pat)", "required": false },
                { "name": "JIRA_TOKEN", "description": "API token (env/config only)", "required": false },
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
            "issue_links": "issueLinks[].sentence is a plain-English summary e.g. 'PROJ-1 blocks PROJ-2'. Use it instead of parsing inward/outward fields.",
            "pagination": "'issues list' and 'search' JSON includes total/startAt/maxResults. Use --offset to page through results.",
            "sprint_fields": "sprintId and sprintName are only present in 'issues create' output when --sprint is used."
        },
        "commands": commands
    })
}

/// Walk the clap command tree and emit a schema entry for every leaf command.
///
/// Intermediate subcommand groups (e.g. `issues`, `projects`) are not emitted;
/// only leaf commands that perform an action produce an entry. Command names are
/// built as space-joined paths (e.g. `"issues list"`). Positional argument names
/// are appended in angle brackets to form the display name (e.g. `"issues show <key>"`).
fn walk_commands(
    cmd: &clap::Command,
    path: &[String],
    annotations: &std::collections::HashMap<&str, serde_json::Value>,
    global_ids: &std::collections::HashSet<&str>,
) -> Vec<serde_json::Value> {
    let subs: Vec<_> = cmd
        .get_subcommands()
        .filter(|s| s.get_name() != "help")
        .collect();

    if subs.is_empty() {
        // Leaf command — emit a schema entry.
        let positionals: Vec<_> = cmd.get_arguments().filter(|a| a.is_positional()).collect();
        let flags: Vec<_> = cmd
            .get_arguments()
            .filter(|a| {
                !a.is_positional()
                    && a.get_long() != Some("help")
                    && a.get_long() != Some("version")
                    && !global_ids.contains(a.get_id().as_str())
            })
            .collect();

        let base_path = path.join(" ");
        let display_name = if positionals.is_empty() {
            base_path.clone()
        } else {
            let suffix: Vec<String> = positionals
                .iter()
                .map(|a| format!("<{}>", a.get_id().as_str()))
                .collect();
            format!("{base_path} {}", suffix.join(" "))
        };

        let mut entry = serde_json::Map::new();
        entry.insert("name".into(), serde_json::json!(display_name));
        entry.insert(
            "description".into(),
            serde_json::json!(cmd.get_about().map(|s| s.to_string()).unwrap_or_default()),
        );

        let ann = annotations.get(base_path.as_str());

        if let Some(alias) = ann.and_then(|a| a.get("alias_for")) {
            entry.insert("alias_for".into(), alias.clone());
        }

        if !positionals.is_empty() {
            let args: Vec<serde_json::Value> = positionals
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "name": a.get_id().as_str(),
                        "description": a.get_help().map(|s| s.to_string()).unwrap_or_default(),
                        "required": a.is_required_set(),
                    })
                })
                .collect();
            entry.insert("args".into(), serde_json::json!(args));
        }

        if !flags.is_empty() {
            let flag_entries: Vec<serde_json::Value> = flags
                .iter()
                .map(|a| {
                    let long_name = a
                        .get_long()
                        .map(|l| format!("--{l}"))
                        .unwrap_or_else(|| format!("--{}", a.get_id().as_str().replace('_', "-")));
                    let mut f = serde_json::Map::new();
                    f.insert("name".into(), serde_json::json!(long_name));
                    if let Some(short) = a.get_short() {
                        f.insert("short".into(), serde_json::json!(format!("-{short}")));
                    }
                    f.insert(
                        "description".into(),
                        serde_json::json!(a.get_help().map(|s| s.to_string()).unwrap_or_default()),
                    );
                    f.insert("required".into(), serde_json::json!(a.is_required_set()));
                    if !a.get_default_values().is_empty() {
                        let dv = a.get_default_values()[0].to_string_lossy();
                        if let Ok(n) = dv.parse::<i64>() {
                            f.insert("default".into(), serde_json::json!(n));
                        } else {
                            f.insert("default".into(), serde_json::json!(dv.as_ref()));
                        }
                    }
                    serde_json::Value::Object(f)
                })
                .collect();
            entry.insert("flags".into(), serde_json::json!(flag_entries));
        }

        if let Some(shape) = ann.and_then(|a| a.get("json_shape")) {
            entry.insert("json_shape".into(), shape.clone());
        }

        vec![serde_json::Value::Object(entry)]
    } else {
        // Intermediate group — recurse into subcommands.
        subs.iter()
            .flat_map(|sub| {
                let mut new_path = path.to_vec();
                new_path.push(sub.get_name().to_string());
                walk_commands(sub, &new_path, annotations, global_ids)
            })
            .collect()
    }
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
    use jira_cli::test_support::{
        EnvVarGuard, ProcessEnvLock, set_config_dir_env, unset_config_dir_env,
    };
    use tempfile::TempDir;

    #[test]
    fn parse_components_update_arg_empty_is_none() {
        let values: Vec<String> = vec![];
        assert!(parse_components_update_arg(&values).is_none());
    }

    #[test]
    fn parse_components_update_arg_none_sentinel_clears() {
        let values = vec!["none".to_string()];
        assert_eq!(parse_components_update_arg(&values), Some(vec![]));
    }

    #[test]
    fn parse_components_update_arg_values_pass_through() {
        let values = vec!["Backend".to_string(), "API".to_string()];
        assert_eq!(
            parse_components_update_arg(&values),
            Some(vec!["Backend", "API"])
        );
    }

    #[test]
    fn parse_components_update_arg_real_component_named_none_with_others() {
        // If a user passes --components none --components Backend, "none" is
        // treated as a literal component name, not the sentinel.
        let values = vec!["none".to_string(), "Backend".to_string()];
        assert_eq!(
            parse_components_update_arg(&values),
            Some(vec!["none", "Backend"])
        );
    }

    #[test]
    fn parse_fix_versions_update_arg_empty_is_none() {
        assert!(parse_fix_versions_update_arg(&[]).is_none());
    }

    #[test]
    fn parse_fix_versions_update_arg_none_sentinel_clears() {
        let values = vec!["none".to_string()];
        assert_eq!(parse_fix_versions_update_arg(&values), Some(vec![]));
    }

    #[test]
    fn parse_fix_versions_update_arg_values_pass_through() {
        let values = vec!["1.2.0".to_string(), "1.3.0".to_string()];
        assert_eq!(
            parse_fix_versions_update_arg(&values),
            Some(vec!["1.2.0", "1.3.0"])
        );
    }

    #[test]
    fn parse_labels_update_arg_empty_is_none() {
        assert!(parse_labels_update_arg(&[]).is_none());
    }

    #[test]
    fn parse_labels_update_arg_none_sentinel_clears() {
        let values = vec!["none".to_string()];
        assert_eq!(parse_labels_update_arg(&values), Some(vec![]));
    }

    #[test]
    fn parse_labels_update_arg_values_pass_through() {
        let values = vec!["backend".to_string(), "urgent".to_string()];
        assert_eq!(
            parse_labels_update_arg(&values),
            Some(vec!["backend", "urgent"])
        );
    }

    #[test]
    fn vec_to_opt_refs_empty_is_none() {
        let values: Vec<String> = vec![];
        assert!(vec_to_opt_refs(&values).is_none());
    }

    #[test]
    fn vec_to_opt_refs_passes_through_values() {
        let values = vec!["a".to_string(), "b".to_string()];
        assert_eq!(vec_to_opt_refs(&values), Some(vec!["a", "b"]));
    }

    #[test]
    fn schema_does_not_advertise_nonexistent_token_flag() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let _config_dir = unset_config_dir_env();
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
        let _env = ProcessEnvLock::acquire().unwrap();
        let _config_dir = unset_config_dir_env();
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
        // email is not required when using PAT auth, so required_fields only
        // lists the fields that are always mandatory.
        assert_eq!(
            auth["required_fields"],
            serde_json::json!(["host", "token"])
        );
        assert!(
            auth["email_note"].as_str().is_some(),
            "schema must explain when email is required"
        );

        let auth_env = auth["env"].as_array().unwrap();
        assert!(
            auth_env.iter().all(|entry| entry["required"] == false),
            "individual env vars are optional auth sources, not mandatory on their own"
        );
    }

    #[test]
    fn schema_config_init_uses_platform_specific_bootstrap_guidance() {
        let _env = ProcessEnvLock::acquire().unwrap();
        let _config_dir = unset_config_dir_env();
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

    #[test]
    fn parse_field_number_value() {
        let (key, val) = parse_field("customfield_10106=8").unwrap();
        assert_eq!(key, "customfield_10106");
        assert_eq!(val, serde_json::json!(8));
        assert!(val.is_number());
    }

    #[test]
    fn parse_field_float_value() {
        let (_key, val) = parse_field("customfield_10106=3.5").unwrap();
        assert_eq!(val, serde_json::json!(3.5));
    }

    #[test]
    fn parse_field_bool_value() {
        let (_, val) = parse_field("customfield_foo=true").unwrap();
        assert_eq!(val, serde_json::json!(true));
        let (_, val2) = parse_field("customfield_foo=false").unwrap();
        assert_eq!(val2, serde_json::json!(false));
    }

    #[test]
    fn parse_field_string_value() {
        let (key, val) = parse_field("customfield_10014=PROJ-1").unwrap();
        assert_eq!(key, "customfield_10014");
        assert_eq!(val, serde_json::json!("PROJ-1"));
        assert!(val.is_string());
    }

    #[test]
    fn parse_field_json_object_value() {
        let (_, val) = parse_field(r#"customfield_10080={"id":"10000"}"#).unwrap();
        assert_eq!(val["id"], "10000");
    }

    #[test]
    fn parse_field_json_array_value() {
        let (_, val) = parse_field(r#"labels=["backend","urgent"]"#).unwrap();
        assert_eq!(val[0], "backend");
        assert_eq!(val[1], "urgent");
    }

    #[test]
    fn parse_field_plain_string_with_spaces() {
        // A value that is not valid JSON falls back to a plain string
        let (_, val) = parse_field("summary=hello world").unwrap();
        assert_eq!(val, serde_json::json!("hello world"));
    }

    #[test]
    fn parse_field_missing_equals_returns_error() {
        let err = parse_field("noequalssign").unwrap_err();
        assert!(err.contains("key=value"));
    }

    #[test]
    fn parse_field_value_with_equals_in_it() {
        // split_once ensures only the first '=' splits key from value
        let (key, val) = parse_field("customfield_10014=A=B").unwrap();
        assert_eq!(key, "customfield_10014");
        assert_eq!(val, serde_json::json!("A=B"));
    }
}
