# AGENTS.md — Timesheeps

Context for AI coding assistants picking up this project.

## What is this

A Tauri 2.0 desktop app that records the active window (app name + window title) every 5 seconds and displays a visual daily timeline. The user drags to create time blocks assigned to projects, then uses those as reference when manually filling out Harvest timesheets. No export, no browser extension — window title is sufficient.

## Status

**The app is fully implemented and the frontend build passes (`pnpm run build` exits 0).**

Rust backend: ✅ complete  
Frontend Vue/TS: ✅ complete  
Tauri config: ✅ complete  
Rust compilation on Windows: ❓ not yet verified (developed in WSL)

The next step is opening the project natively on Windows and running `pnpm tauri dev` to verify the Rust build and test live activity tracking.

## Tech stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri 2.0 |
| Frontend | Vue 3 + TypeScript + Vite |
| State | Pinia |
| Routing | Vue Router 4 (two routes: `/` and `/settings`) |
| Validation | Zod 3 (mirrors Rust models) |
| Date utils | date-fns 4 |
| DB | SQLite via `rusqlite` (bundled, no Tauri SQL plugin) |
| Win32 | `windows` crate 0.58 (behind `cfg(target_os = "windows")`) |
| Package manager | **pnpm** (`pnpm@10.0.0` via corepack — never use npm) |

## Key commands

```bash
pnpm dev              # Vite only (no Rust, IPC calls will fail — UI iteration only)
pnpm run build        # vue-tsc type check + Vite bundle
pnpm tauri dev        # Full app with Rust backend (Windows only for tracking to work)
pnpm tauri build      # Production binary
```

## Project structure

```
src-tauri/
  src/
    lib.rs            # App entry: DB open, tray, polling start, command registration
    main.rs           # Tauri entry point (calls lib::run)
    models.rs         # All serde structs (RawActivity, ActivityBlock, Project, TimeEntry, FilterRule, Settings)
    db.rs             # SQLite schema + all CRUD + merge_and_filter algorithm
    activity.rs       # Background Win32 polling loop (tokio), idle detection
    commands/
      activity.rs     # get_activity_for_day
      projects.rs     # get_projects, create_project, update_project, archive_project
      time_entries.rs # get_time_entries_for_day, create_time_entry, update_time_entry, delete_time_entry
      filter_rules.rs # get_filter_rules, create_filter_rule, delete_filter_rule
      settings.rs     # get_settings, save_settings
      mod.rs

src/
  main.ts             # createApp + Pinia + router + style.css
  App.vue             # Nav bar + RouterView + IdlePrompt listener
  style.css           # CSS custom properties (--bg, --surface, --primary, etc.), dark mode, utility classes
  schemas/index.ts    # Zod schemas — all in camelCase matching Rust's serde(rename_all = "camelCase")
  api/index.ts        # Typed IPC wrappers using callArray/callOne helpers
  router/index.ts     # createWebHistory, routes: / → TimelineView, /settings → SettingsView
  stores/
    day.ts            # selectedDate, activityBlocks, timeEntries, loadDay, createEntry, updateEntry, deleteEntry
    projects.ts       # projects list, load, create, update, archive
    settings.ts       # settings object + filterRules, load, save, createRule, deleteRule
  composables/
    useTimeline.ts    # HOUR_HEIGHT=80, minuteToY, yToMinute, snapMinutes, isoToMinutes, formatDuration
  views/
    TimelineView.vue  # DayNav + TimelineCanvas + summary bar, auto-refreshes every 30s when viewing today
    SettingsView.vue  # General settings form + ProjectList + FilterRuleList
  components/
    DayNav.vue             # Prev/Next/Today navigation
    TimeRuler.vue          # Hour labels (44px wide)
    ActivityBlockItem.vue  # Read-only colored block (hashed app color)
    TimeBlockItem.vue      # Draggable/resizable time entry block
    EntryTrack.vue         # Drag-to-create new entries, hosts TimeBlockItems
    TimelineCanvas.vue     # Full layout: grid + ruler + activity track + entry track; hosts ProjectPickerModal
    ProjectPickerModal.vue # Create/edit time entry: project select + note + time range
    ProjectList.vue        # Full CRUD for projects with color swatches + archive
    FilterRuleList.vue     # Add/remove ignore rules (title_pattern or app_name)
    IdlePrompt.vue         # Fixed bottom-right toast when user returns from idle
```

## Important implementation details

### Timeline math
- `HOUR_HEIGHT = 80` px per hour
- `minuteToY(min) = (min - startMin) / 60 * HOUR_HEIGHT`
- All snap to 5-minute increments
- `isoToMinutes(iso)` converts UTC ISO string → local-time minutes via `new Date(iso).getHours() * 60 + ...`

### Rust activity polling
- Polls every 5 seconds in a `tokio::time::sleep` loop
- Win32 functions only compiled on Windows (`cfg(target_os = "windows")`), all return `None`/`0` on other platforms
- On window change: flushes current session to `activity_raw` table
- On idle (> `idle_timeout_secs`): flushes partial session, emits `idle-return` Tauri event on return

### DB merge/filter (`db::merge_and_filter`)
1. Apply ignore rules (filter_rules table) — drop matching raw events
2. Merge consecutive same-app events within `merge_gap_secs`
3. Drop blocks shorter than `min_duration_secs`
Returns `Vec<ActivityBlock>` with ISO start/end timestamps

### Settings defaults (Settings::default in models.rs)
- `min_duration_secs`: 60
- `merge_gap_secs`: 120
- `idle_timeout_secs`: 300
- `timeline_start_hour`: 7
- `timeline_end_hour`: 22

### Tauri config
- Window: 1100×780, min 900×600
- Close button hides to tray (CloseRequested intercepted in lib.rs)
- Tray menu: Show / Quit
- Capabilities: `core:default`, `core:tray:default`

## Known issues / things not yet verified

- Rust build on Windows not tested — may need `windows` crate feature flags tweaked
- `pnpm tauri dev` not run end-to-end yet
- Dark mode CSS tested visually but not on actual dark OS theme
- No error handling in frontend if IPC commands fail (errors are silently swallowed in stores)
