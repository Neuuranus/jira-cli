---
name: jira
description: "Manage Jira issues, projects, sprints, and boards using the `jira` command-line tool. Supports all issues subcommands including list, create, update, transition, comment, link, log-work, bulk operations, and attachments."
---
# Jira CLI Specialist

This skill provides instructions for using the `jira` CLI tool to interact with Jira. The binary is at `~/.local/bin/jira`. Configuration is at `~/.config/jira/config.toml` (PAT auth, Jira Data Center).

**Agent note:** Run `jira schema` at the start of a session to get a complete machine-readable description of all commands, flags, and JSON output shapes.

## Output behavior

- JSON is emitted automatically when stdout is not a TTY (e.g., when called from an agent)
- Force JSON with `--json` on any command
- `--quiet` suppresses counts and status messages, leaving only data on stdout
- Set `JIRA_READ_ONLY=1` to block all write operations (safe read-only mode)

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

---

## Listing issues

```sh
# List all issues (sorted by updated DESC)
jira issues list

# Filter by project, status, assignee, type, sprint
jira issues list --project MYAPP --status "In Progress"
jira issues list --project MYAPP --type Bug --assignee me
jira issues list --sprint active          # open sprints
jira issues list --sprint "Sprint 14"     # named sprint

# Filter by components, labels, fix-versions (repeatable flags)
jira issues list --component Backend --component API
jira issues list --label urgent --label backend
jira issues list --fix-version 1.2.0 --fix-version 1.3.0

# Filter by saved Jira filter ID or name
jira issues list --filter "My Filter"

# Extra JQL appended to other filters
jira issues list --project MYAPP --jql 'priority = High'

# Pagination
jira issues list --limit 50 --offset 100
jira issues list --all                    # fetch every page automatically

# Issues assigned to the current user
jira issues mine
jira issues mine --project MYAPP --status "To Do"
```

## Viewing an issue

```sh
jira issues show MYAPP-123
jira issues show MYAPP-123 --open         # open in browser
```

JSON output includes: key, id, url, summary, status, type, priority, assignee, reporter, labels, components, fixVersions, affectedVersions, description, created, updated, comments, issueLinks.

## Creating issues

```sh
# Minimal
jira issues create --project MYAPP --summary "Fix login bug" --type Bug

# Full options
jira issues create --project MYAPP \
  --summary "Add dark mode" \
  --type Story \
  --description "Users want a dark mode option." \
  --priority High \
  --assignee me \
  --label ui --label frontend \
  --component "Web App" \
  --fix-version 2.0.0 \
  --sprint active

# Subtask (child of another issue)
jira issues create --project MYAPP --summary "Write unit tests" \
  --type Sub-task --parent MYAPP-42

# Custom fields
jira issues create --project MYAPP --summary "Example" --type Story \
  --field customfield_10016=5
```

Returns JSON: `{ "key": "MYAPP-123", "id": "...", "url": "..." }`

## Updating issues

```sh
jira issues update MYAPP-123 --summary "Updated title"
jira issues update MYAPP-123 --priority Low --assignee me
jira issues update MYAPP-123 --assignee none           # unassign
jira issues update MYAPP-123 --description "New desc"
jira issues update MYAPP-123 --label urgent --label backend
jira issues update MYAPP-123 --component Backend
jira issues update MYAPP-123 --fix-version 1.3.0

# Custom fields
jira issues update MYAPP-123 --field customfield_10016=8
```

Returns JSON: `{ "key": "MYAPP-123", "updated": true }`

## Transitioning issues

```sh
# List available transitions for an issue
jira issues list-transitions MYAPP-123

# Transition by name or transition ID
jira issues transition MYAPP-123 --to "In Review"
jira issues transition MYAPP-123 --to "Done"
```

`list-transitions` JSON: array of `{ "id": "...", "name": "...", "to": { "name": "..." } }`

## Assigning issues

```sh
jira issues assign MYAPP-123 --assignee me
jira issues assign MYAPP-123 --assignee user@example.com
jira issues assign MYAPP-123 --assignee none           # unassign
```

## Comments

```sh
# List comments
jira issues comments MYAPP-123

# Add a comment
jira issues comment MYAPP-123 --body "Deployed to staging."
```

`comments` JSON: `{ "issue": "...", "total": N, "comments": [{ "id", "author", "body", "created", "updated" }] }`

## Logging work

```sh
jira issues log-work MYAPP-123 --time-spent 2h
jira issues log-work MYAPP-123 --time-spent 30m --comment "Fixed the flaky test"
jira issues log-work MYAPP-123 --time-spent 1h30m --started "2024-01-15T09:00:00.000+0000"
```

Returns JSON: `{ "id", "issue", "timeSpent", "timeSpentSeconds", "author", "started", "created" }`

## Issue links

```sh
# List available link types
jira issues link-types

# Link two issues
jira issues link MYAPP-123 --to MYAPP-456 --type "Blocks"
jira issues link MYAPP-123 --to MYAPP-789 --type "relates to"

# Remove a link (use link ID from show/link-types output)
jira issues unlink <link-id>
```

`link-types` JSON: array of `{ "id", "name", "inward", "outward" }`

## Moving to a sprint

```sh
jira issues move MYAPP-123 --sprint active
jira issues move MYAPP-123 --sprint "Sprint 14"
```

Returns JSON: `{ "issue", "sprintId", "sprintName" }`

## Attachments

```sh
# List attachments on an issue
jira issues attach list MYAPP-123

# Download all attachments to a directory
jira issues attach download MYAPP-123 --dir ./attachments

# Download a specific attachment by ID
jira issues attach download MYAPP-123 --id <attachment-id> --dir ./attachments

# Upload one or more files as attachments
jira issues attach upload MYAPP-123 ./report.pdf
jira issues attach upload MYAPP-123 ./a.txt ./b.png
```

`attach list` JSON: `{ "issue", "total", "attachments": [{ "id", "filename", "mimeType", "size", "created", "author", "content" }] }`

`attach upload` JSON: `{ "issue", "uploaded": N, "attachments": [{ "id", "filename", "mimeType", "size", "created", "author", "content" }] }`

## Bulk operations

Always use `--dry-run` first to preview what will be affected.

```sh
# Bulk transition: move all matching issues to a new status
jira issues bulk-transition \
  --jql 'project = MYAPP AND status = "To Do"' \
  --to "In Progress" \
  --dry-run                              # preview only

jira issues bulk-transition \
  --jql 'project = MYAPP AND status = "To Do"' \
  --to "In Progress"

# Bulk assign: assign all matching issues to a user
jira issues bulk-assign \
  --jql 'project = MYAPP AND sprint in openSprints()' \
  --assignee me \
  --dry-run

jira issues bulk-assign \
  --jql 'project = MYAPP AND sprint in openSprints()' \
  --assignee me
```

Bulk JSON response: `{ "dryRun", "total", "succeeded", "failed", "issues": [{ "key", "ok", ... }] }`

---

## JQL search

```sh
jira search 'project = MYAPP AND sprint in openSprints() ORDER BY priority'
jira search 'assignee = currentUser() AND status != Done' --limit 20
jira search 'project = MYAPP' --all      # fetch every page
```

## Projects

```sh
jira projects list
jira projects show MYAPP
jira projects components MYAPP           # list project components
jira projects versions MYAPP             # list project versions
```

## Boards and sprints

```sh
jira boards list
jira sprints list
jira sprints list --board "MYAPP board"
```

## Users and fields

```sh
jira users search --query "alice"
jira fields list
jira fields list --custom                # custom fields only
```

## Current user

```sh
jira myself
```

## Configuration and diagnostics

```sh
jira config show            # show resolved credentials (token masked)
jira schema                 # dump all commands and JSON shapes as machine-readable JSON
jira schema | jq '.commands[] | select(.name == "issues list")'
```

## Common patterns for agents

```sh
# Find open bugs in a project assigned to me
jira issues mine --project MYAPP --type Bug --status "In Progress"

# Get full detail of an issue as JSON
jira issues show MYAPP-123 --json

# Check what status transitions are available before transitioning
jira issues list-transitions MYAPP-123 --json

# Find all issues blocking a release
jira search 'project = MYAPP AND fixVersion = "2.0.0" AND status != Done'

# Look up custom field IDs before using --field
jira fields list --custom --json
```
