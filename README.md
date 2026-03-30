# jira

An agent-friendly Jira CLI. Designed to be driven by AI agents and scripts, not just humans.

- **Auto-JSON** when stdout is not a TTY — pipe it anywhere, get structured data
- **`jira schema`** dumps every command, flag, and JSON shape as machine-readable JSON for agent introspection
- **Structured exit codes** — agents can branch on auth failures, rate limits, not-found, and input errors without parsing text
- **Clean stdout/stderr split** — data on stdout, messages on stderr, `--quiet` suppresses all non-data output

```
$ jira issues list --project MYAPP --status "In Progress" --json
{
  "total": 3,
  "startAt": 0,
  "maxResults": 50,
  "issues": [...]
}
```

## Installation

```sh
cargo install jira-cli
```

Or build from source:

```sh
git clone https://github.com/rvben/jira-cli
cd jira-cli
cargo install --path .
```

## Configuration

Create the config file reported by `jira init`, `jira config init`, or
`jira config show`.

Default locations:

- Unix-like systems: `~/.config/jira/config.toml`
- Unix-like systems with `XDG_CONFIG_HOME` set: `$XDG_CONFIG_HOME/jira/config.toml`
- Windows: `%APPDATA%\jira\config.toml`

Example:

```toml
[default]
host  = "mycompany.atlassian.net"
email = "me@example.com"
token = "your-api-token"
```

Run `jira config show` to confirm the resolved path and active credentials.

Get your API token at: https://id.atlassian.com/manage-profile/security/api-tokens

```sh
# Unix-like systems only
chmod 600 ~/.config/jira/config.toml
```

On Windows, keep the file in your per-user `%APPDATA%` directory rather than a
shared folder.

Or use environment variables:

```sh
export JIRA_HOST=mycompany.atlassian.net
export JIRA_EMAIL=me@example.com
export JIRA_TOKEN=your-api-token
```

Multiple profiles are supported:

```toml
[profiles.work]
host  = "work.atlassian.net"
email = "me@work.com"
token = "work-token"
```

Select with `--profile work` or `JIRA_PROFILE=work`.

## Usage

```sh
# List issues
jira issues list
jira issues list --project MYAPP --status "In Progress" --assignee me
jira issues list --sprint active

# Show an issue
jira issues show MYAPP-123

# Create an issue
jira issues create --project MYAPP --summary "Fix login bug" --issue-type Bug --priority High

# Transition an issue
jira issues list-transitions MYAPP-123
jira issues transition MYAPP-123 --to "In Review"

# Comment
jira issues comment MYAPP-123 --body "Deployed to staging."

# Assign
jira issues assign MYAPP-123 --assignee me

# Search with JQL
jira search 'project = MYAPP AND sprint in openSprints() ORDER BY priority'

# Projects
jira projects list
jira projects show MYAPP

# Current user
jira myself

# Config
jira init
jira config show
jira config init
```

## Agent use

`jira schema` returns a complete description of all commands, flags, arguments, JSON output shapes, and exit codes. AI agents should call this once at the start of a session.

```sh
jira schema | jq '.commands[] | .name'
```

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Unexpected error |
| 2 | Bad input or config error |
| 3 | Authentication failed |
| 4 | Resource not found |
| 5 | Jira API error |
| 6 | Rate limited |

## Output flags

| Flag | Effect |
|------|--------|
| `--json` | Force JSON output (auto when stdout is not a TTY) |
| `--quiet` | Suppress counts, confirmations, and status messages |

## License

MIT
