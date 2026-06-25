#!/usr/bin/env node
// Timesheeps MCP Server
// Exposes activity and time-entry data from the Timesheeps SQLite database
// to Claude (or any MCP-compatible client) via stdio.
//
// DB location (Tauri app data dir on Windows):
//   %APPDATA%\app.timesheeps.Timesheeps\timesheeps.db
// Override by setting the TIMESHEEPS_DB environment variable.

'use strict'

const path = require('path')
const os = require('os')
const Database = require('better-sqlite3')
const { Server } = require('@modelcontextprotocol/sdk/server/index.js')
const { StdioServerTransport } = require('@modelcontextprotocol/sdk/server/stdio.js')
const {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} = require('@modelcontextprotocol/sdk/types.js')

// ── Database path ─────────────────────────────────────────────────────────────

const DB_PATH =
  process.env.TIMESHEEPS_DB ||
  path.join(
    process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming'),
    'app.timesheeps.Timesheeps',
    'timesheeps.db',
  )

function openDb() {
  return new Database(DB_PATH, { readonly: true, fileMustExist: true })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function todayLocalDate() {
  const now = new Date()
  return [
    now.getFullYear(),
    String(now.getMonth() + 1).padStart(2, '0'),
    String(now.getDate()).padStart(2, '0'),
  ].join('-')
}

function minutesToTime(mins) {
  const h = Math.floor(mins / 60)
  const m = mins % 60
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
}

function formatDuration(secs) {
  if (secs < 60) {
    return `${secs}s`
  }

  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)

  if (h === 0) {
    return `${m}m`
  }

  return m === 0 ? `${h}h` : `${h}h ${m}m`
}

// ── Queries ───────────────────────────────────────────────────────────────────

// Returns total time spent per app (and window title) for a local calendar day.
function getActivitySummary(date) {
  const db = openDb()

  try {
    // Aggregate by app_name + window_title for the given local date.
    // activity_raw stores UTC ISO timestamps; SQLite's 'localtime' modifier
    // converts them to the host's local timezone for date bucketing.
    const rows = db
      .prepare(
        `SELECT
           app_name,
           window_title,
           SUM(CAST((julianday(ended_at) - julianday(started_at)) * 86400 AS INTEGER)) AS total_secs
         FROM activity_raw
         WHERE date(started_at, 'localtime') = ?
         GROUP BY app_name, window_title
         ORDER BY total_secs DESC`,
      )
      .all(date)

    // Roll up to app level for a high-level summary
    const byApp = {}

    for (const r of rows) {
      if (!byApp[r.app_name]) {
        byApp[r.app_name] = { app_name: r.app_name, total_secs: 0, windows: [] }
      }

      byApp[r.app_name].total_secs += r.total_secs
      byApp[r.app_name].windows.push({
        title: r.window_title,
        duration: formatDuration(r.total_secs),
        total_secs: r.total_secs,
      })
    }

    return Object.values(byApp)
      .sort((a, b) => b.total_secs - a.total_secs)
      .map((a) => ({ ...a, duration: formatDuration(a.total_secs) }))
  } finally {
    db.close()
  }
}

// Returns manually logged time entries for a date.
function getTimeEntries(date) {
  const db = openDb()

  try {
    const rows = db
      .prepare(
        `SELECT te.id,
                te.date,
                te.start_minutes,
                te.end_minutes,
                te.note,
                p.name  AS project_name,
                p.color AS project_color
         FROM time_entries te
         JOIN projects p ON p.id = te.project_id
         WHERE te.date = ?
         ORDER BY te.start_minutes`,
      )
      .all(date)

    return rows.map((r) => ({
      id: r.id,
      date: r.date,
      project: r.project_name,
      color: r.project_color,
      start: minutesToTime(r.start_minutes),
      end: minutesToTime(r.end_minutes),
      duration: formatDuration((r.end_minutes - r.start_minutes) * 60),
      duration_mins: r.end_minutes - r.start_minutes,
      note: r.note,
    }))
  } finally {
    db.close()
  }
}

// Returns all (non-archived) projects.
function getProjects() {
  const db = openDb()

  try {
    return db
      .prepare(
        `SELECT id, name, color, parent_id
         FROM projects
         WHERE archived_at IS NULL
         ORDER BY name`,
      )
      .all()
  } finally {
    db.close()
  }
}

// Returns activity summary + time entries for a date in one call.
function getDaySummary(date) {
  const activity = getActivitySummary(date)
  const entries = getTimeEntries(date)

  const totalTrackedSecs = activity.reduce((s, a) => s + a.total_secs, 0)
  const totalLoggedMins = entries.reduce((s, e) => s + e.duration_mins, 0)

  return {
    date,
    total_tracked: formatDuration(totalTrackedSecs),
    total_logged: formatDuration(totalLoggedMins * 60),
    activity_by_app: activity,
    time_entries: entries,
  }
}

// ── MCP server ────────────────────────────────────────────────────────────────

const server = new Server({ name: 'timesheeps', version: '1.0.0' }, { capabilities: { tools: {} } })

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: 'get_day_summary',
      description:
        'Get a combined summary of computer activity and logged time entries for a day. ' +
        'Use this to answer questions like "what did I work on today?" or "how did I spend Tuesday?". ' +
        'Returns both raw app usage (from window tracking) and any manually logged Harvest-style time entries.',
      inputSchema: {
        type: 'object',
        properties: {
          date: {
            type: 'string',
            description: 'Date in YYYY-MM-DD format. Defaults to today if omitted.',
          },
        },
      },
    },
    {
      name: 'get_activity_summary',
      description:
        'Get a breakdown of time spent per application and window title on a given day. ' +
        'Useful for seeing which apps and projects dominated a day.',
      inputSchema: {
        type: 'object',
        properties: {
          date: {
            type: 'string',
            description: 'Date in YYYY-MM-DD format. Defaults to today if omitted.',
          },
        },
      },
    },
    {
      name: 'get_time_entries',
      description:
        'Get manually logged time entries (project + note + duration) for a given day. ' +
        'These are the blocks the user has explicitly created in the Timesheeps timeline.',
      inputSchema: {
        type: 'object',
        properties: {
          date: {
            type: 'string',
            description: 'Date in YYYY-MM-DD format. Defaults to today if omitted.',
          },
        },
      },
    },
    {
      name: 'get_projects',
      description: 'List all active (non-archived) projects defined in Timesheeps.',
      inputSchema: {
        type: 'object',
        properties: {},
      },
    },
  ],
}))

server.setRequestHandler(CallToolRequestSchema, async (req) => {
  const { name, arguments: args } = req.params

  try {
    let result
    const date = args?.date || todayLocalDate()

    if (name === 'get_day_summary') {
      result = getDaySummary(date)
    } else if (name === 'get_activity_summary') {
      result = { date, activity_by_app: getActivitySummary(date) }
    } else if (name === 'get_time_entries') {
      result = { date, entries: getTimeEntries(date) }
    } else if (name === 'get_projects') {
      result = { projects: getProjects() }
    } else {
      throw new Error(`Unknown tool: ${name}`)
    }

    return {
      content: [{ type: 'text', text: JSON.stringify(result, null, 2) }],
    }
  } catch (err) {
    return {
      content: [{ type: 'text', text: `Error: ${err.message}` }],
      isError: true,
    }
  }
})

// ── Entry point ───────────────────────────────────────────────────────────────

async function main() {
  const transport = new StdioServerTransport()
  await server.connect(transport)
}

main().catch((err) => {
  process.stderr.write(`Fatal: ${err.message}\n`)
  process.exit(1)
})
