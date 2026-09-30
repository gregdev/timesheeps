# Timesheeps MCP Server

Lets you ask Claude Desktop things like:

- **"How much time did I spend on each project over the last fortnight?"**
- **"Break my Lotteries time down by Jira ticket."**
- **"What did I work on today?"**
- **"Set up match rules for these clients."**

Claude calls this server, which reads your local Timesheeps SQLite database and returns real
activity, classified by your own match rules.

The server is `timesheeps-mcp`, a small Rust binary built from this repository
(`src-tauri/src/bin/mcp_server.rs`). There is no Node.js component and nothing to `npm install`.

---

## 1. Set it up from the app

Open Timesheeps → **Settings → Claude AI → Set up Claude MCP**.

This writes the server into your Claude Desktop config for you (it finds both the traditional
`%APPDATA%\Claude` install and the Microsoft Store sandboxed one). Then **fully quit and reopen
Claude** — not just the window, use Quit from the tray.

You also need developer mode on in Claude: **Help → Troubleshoot → Enable Developer Mode**.

You should see a hammer icon (🔨) near the chat input once it is working.

## 2. Or configure it by hand

Add this to `claude_desktop_config.json`
(`%APPDATA%\Claude\claude_desktop_config.json`, or under
`%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude\`):

```json
{
  "mcpServers": {
    "timesheeps": {
      "command": "C:\\Users\\<you>\\AppData\\Local\\Timesheeps\\timesheeps-mcp.exe"
    }
  }
}
```

That is where the installer puts it, next to `timesheeps.exe`. If you only ever run
`pnpm tauri dev`, point it at `src-tauri\target\debug\timesheeps-mcp.exe` instead.

---

## Tools

### Reading

| Tool                       | What it does                                                                                                                                                                                 |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `get_classified_totals`    | **The main one.** Time grouped by project (or by ticket, or by app) across a date range, with a per-day breakdown and a tracked-vs-logged comparison. Also reports activity no rule claimed. |
| `get_day_summary_range`    | Per-day totals across a range, with an optional per-day project split, entries and title list.                                                                                               |
| `get_day_summary`          | Everything about one day.                                                                                                                                                                    |
| `get_activity_summary`     | Per-app / per-window-title time for one day.                                                                                                                                                 |
| `get_window_summary_range` | Window-level activity across a range, reaggregated by app and title.                                                                                                                         |
| `unclassified_titles`      | Titles that matched no rule, ranked by time — the raw material for new rules.                                                                                                                |
| `get_match_rules`          | Every rule, its conditions, precedence position and effective ticket pattern.                                                                                                                |
| `get_projects`             | Active projects (with ticket patterns) and archived ones.                                                                                                                                    |
| `get_known_apps`           | Apps and sample titles actually seen recently, for writing conditions against real values.                                                                                                   |
| `get_time_entries`         | Manually logged entries for one day.                                                                                                                                                         |
| `get_settings`             | Tracking settings, including whether writes are allowed.                                                                                                                                     |

### Writing

Disabled unless **Settings → Claude AI → "Allow Claude to change projects, rules and time
entries"** is on. The setting is re-read on every call, and Claude cannot change it.

| Tool                                                             | What it does                                                                                                                                         |
| ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `create_projects` / `update_project`                             | Create and edit projects, including their default ticket pattern.                                                                                    |
| `archive_project` / `unarchive_project` / `delete_project`       | Deletes need `confirm: true` and report everything they cascaded to.                                                                                 |
| `create_match_rules` / `update_match_rule` / `delete_match_rule` | Rule CRUD. Updates return before/after so a change can be undone.                                                                                    |
| `reorder_match_rules`                                            | Set the global precedence order.                                                                                                                     |
| `validate_match_rules`                                           | Dry run. Compiles the rules and reports what each would claim over a range, and what existing rules already take. **Not gated** — it writes nothing. |
| `log_time_entries` / `update_time_entry` / `delete_time_entry`   | Time entries, with `dry_run` support and overlap rejection.                                                                                          |

Every write is validated before anything is committed: hex colours, non-blank condition values,
compilable regexes, existing project ids, and non-overlapping time ranges. A batch is rejected whole
rather than half-applied.

---

## Ticket patterns

To split a project's time by ticket (e.g. Jira's `ELMS-5813`), give the project — or an individual
rule — a `sub_group_pattern`: a case-insensitive regex applied to window titles.

```
ELMS-\d+
```

If the pattern has a capture group, group 1 is used; otherwise the whole match is. So both
`ELMS-\d+` and `ELMS-(\d+)` work. A rule's pattern overrides its project's.

You can just ask Claude:

> "Split my Lotteries time by ELMS ticket number"

Patterns affect **only** the breakdown. They never change which project a block is attributed to, so
a bad regex cannot reclassify your time — it just yields no sub-groups, and the app shows a warning.
Both the app and the MCP validate the regex when it is saved.

---

## Database path

```
C:\Users\<you>\AppData\Roaming\app.timesheeps.Timesheeps\timesheeps.db
```

Override with the `TIMESHEEPS_DB` environment variable (useful for pointing at a copy).

The server opens the database read-write (SQLite in WAL mode, with a busy timeout) so the app can
keep running alongside it. It never runs schema migrations: if the database is older than the server
expects, every tool returns a message telling you to launch Timesheeps once.

---

## Troubleshooting

**"Timesheeps database not found"** — Timesheeps hasn't run yet; it creates the database on first
launch.

**"…is at schema version N but this MCP server expects M. Launch Timesheeps once"** — open the app
so it can migrate, then retry.

**No hammer icon in Claude** — check the path in `claude_desktop_config.json`, and fully restart
Claude (Quit from the tray, not just closing the window).

**Claude says writes are disabled** — turn on **Settings → Claude AI → "Allow Claude to change
projects, rules and time entries"**.

**Numbers disagree with the app** — they shouldn't; both use the same merge and matching pipeline.
Check what `get_settings` reports for `minDurationSecs` and `mergeGapSecs`, since blocks below the
minimum duration are dropped by design.
