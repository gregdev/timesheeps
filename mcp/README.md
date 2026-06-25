# Timesheeps MCP Server

Lets you ask Claude things like:

- **"What did I work on today?"**
- **"How much time did I spend in VS Code this week?"**
- **"Summarise my Thursday"**

Claude will call this server, which reads your local Timesheeps SQLite database and returns real activity + time-entry data.

---

## 1. Install Claude Desktop

Download from <https://claude.ai/download> and install it.

---

## 2. Install dependencies (one-time)

From this folder (`mcp/`):

```
npm install
```

---

## 3. Register the MCP server with Claude Desktop

Open (or create) the Claude Desktop config file:

```
C:\Users\greg2\AppData\Roaming\Claude\claude_desktop_config.json
```

Add (or merge) this JSON:

```json
{
  "mcpServers": {
    "timesheeps": {
      "command": "C:\\Program Files\\nodejs\\node.exe",
      "args": ["C:\\Users\\greg2\\projects\\timesheeps\\mcp\\server.js"]
    }
  }
}
```

If the file doesn't exist yet, create it with exactly that content.
If it already has other servers, add `"timesheeps": { ... }` inside the existing `"mcpServers"` object.

---

## 4. Restart Claude Desktop

Fully quit and reopen Claude. You should see a hammer icon (🔨) near the chat input indicating MCP tools are active.

---

## Available tools (Claude calls these automatically)

| Tool                   | What it does                                                                |
| ---------------------- | --------------------------------------------------------------------------- |
| `get_day_summary`      | Activity + time entries for a day — best for "what did I work on" questions |
| `get_activity_summary` | Raw app/window usage breakdown                                              |
| `get_time_entries`     | Manually logged time blocks with project names                              |
| `get_projects`         | List of active projects                                                     |

All tools accept an optional `date` parameter (YYYY-MM-DD). Without it they default to today.

---

## Database path

The server reads:

```
C:\Users\greg2\AppData\Roaming\app.timesheeps.Timesheeps\timesheeps.db
```

Override with the `TIMESHEEPS_DB` environment variable if needed.

---

## Troubleshooting

**"DB file not found"** — Timesheeps hasn't run yet (the DB is created on first launch).

**No hammer icon in Claude** — Check that the paths in `claude_desktop_config.json` are correct and that you fully restarted Claude (not just closed the window — use Quit from the system tray).
