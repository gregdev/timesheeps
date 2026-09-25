# AGENTS.md — Timesheeps

Context for AI coding assistants picking up this project.

## What is this

A Tauri 2.0 desktop app that records the active window (app name + window title) every 20 seconds and displays a visual daily timeline. The user drags to create time blocks assigned to projects, then uses those as reference when manually filling out Harvest timesheets. No export, no browser extension — window title is sufficient.

## Status

Rust backend: ✅ complete and compiling on Windows (`cargo test` — 30 tests pass)
Frontend Vue/TS: ✅ complete (`pnpm run build` exits 0)
Tauri config: ✅ complete

End-to-end live tracking (`pnpm tauri dev`) has still not been run in anger; the Rust crate is
verified by `cargo check`/`cargo test` but not yet exercised against a live desktop session.

## Tech stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri 2.0 |
| Frontend | Vue 3 + TypeScript + Vite |
| State | Pinia |
| Routing | Vue Router 4 (`/`, `/week`, `/pay-period`, `/projects`, `/settings`, `/about`, `/search`, `/timer-popup`) |
| Validation | Zod 3 (mirrors Rust models) |
| Date utils | date-fns 4 |
| DB | SQLite via `rusqlite` (bundled, no Tauri SQL plugin) |
| Win32 | `windows` crate 0.58 (behind `cfg(target_os = "windows")`) |
| Package manager | **pnpm** (`pnpm@10.0.0` via corepack — never use npm) |

## Key commands

```bash
pnpm dev              # Vite only (no Rust, IPC calls will fail — UI iteration only)
pnpm run build        # vue-tsc type check + Vite bundle
pnpm exec eslint .    # No npm script for lint; note ~36 pre-existing errors in untouched files
pnpm tauri dev        # Full app with Rust backend (Windows only for tracking to work)
pnpm tauri build      # Production binary

cd src-tauri
cargo test            # Runs the Rust suite (migrations, matcher, nl_query)
cargo check --all-targets
```

## Project structure

```
src-tauri/
  src/
    lib.rs            # App entry: DB open, tray, polling start, command registration
    main.rs           # Tauri entry point (calls lib::run)
    bin/mcp_server.rs # Standalone Rust MCP server (separate [[bin]], own SQL, shares no code)
    models.rs         # All serde structs + MatchField/MatchOperator/MatchCondition enums
    db.rs             # SQLite schema + migrations + all CRUD + merge_and_filter
    matcher.rs        # Rule evaluation: compile, block matching, suggestions, rule stats
    appicon.rs        # Windows exe icon extraction → PNG data URI (no-op elsewhere)
    activity.rs       # Background Win32 polling loop (tokio), idle detection, exe path capture
    calendar.rs       # Microsoft 365 OAuth + Graph
    nl_query.rs       # Natural-language → search-token translation (+ the only pre-existing tests)
    timer.rs          # In-memory timer
    commands/
      activity.rs           # get_activity_for_day, get_window_summary_for_day, search, deletes
      apps.rs               # get_app_icon
      calendar.rs           # M365 login/status/events/disconnect
      filter_rules.rs       # ignore rules CRUD
      permissions.rs        # screen-recording permission (macOS)
      project_match_rules.rs# rule CRUD + reorder + get_rule_stats + get_known_apps + suggestions
      projects.rs           # project CRUD + archive/unarchive/delete
      settings.rs           # settings + Claude MCP setup
      time_entries.rs       # time entry CRUD
      timer.rs              # timer commands
      window.rs             # hide_main_window, show_main_window (restore full-size app)
      mod.rs

src/
  main.ts             # createApp + Pinia + router + style.css
  App.vue             # Nav bar (Timeline/Week/Pay Period/Projects/Settings/About) + RouterView
  style.css           # CSS custom properties, dark mode, button/form element styles
  schemas/index.ts    # Zod schemas — camelCase, mirroring Rust serde(rename_all = "camelCase")
  api/index.ts        # Typed IPC wrappers using callArray/callOne helpers
  router/index.ts     # createWebHistory; /, /week, /pay-period, /projects, /settings, /about, /search, /timer-popup
  stores/
    day.ts            # selectedDate, activityBlocks, timeEntries, suggestions, loadDay, entry CRUD
    projects.ts       # projects list, load, create, update, archive, unarchive, remove
    settings.ts       # settings, filterRules, projectMatchRules, ruleStats, knownApps + helpers
    timer.ts
  composables/
    useTimeline.ts    # HOUR_HEIGHT=160, minuteToY, yToMinute, snapMinutes, isoToMinutes, formatDuration
    useFormat.ts      # shared formatMinutes/formatSeconds/formatElapsed/minutesToClock/localDateKey
    useProjectGroups.ts # rolls per-project minutes up into parent/child totals
    useAppColour.ts   # hashed colour per app name (avatar fallback)
    useAppIcon.ts     # memoised app icon fetch, shared module-level cache
    useContextMenu.ts, useEntryModal.ts, usePeriodGrid.ts
  views/
    TimelineView.vue  # DayNav + TimelineCanvas + summary bar, auto-refreshes every 30s when viewing today
    ProjectsView.vue  # Project management + match-rule editor + global rule priority list
    SettingsView.vue  # General settings form + FilterRuleList (projects live on /projects)
    WeekView.vue, PayPeriodView.vue, SearchView.vue, AboutView.vue, TimerPopupView.vue
  components/
    DayNav.vue, TimeRuler.vue, TimeBlockItem.vue, EntryTrack.vue, SuggestedBlockItem.vue
    ActivityBlockItem.vue  # Read-only colored block (hashed app color), right-click menu
    TimelineCanvas.vue     # Grid + ruler + activity track + entry track; hosts ProjectPickerModal
    ProjectPickerModal.vue # Create/edit time entry: project combo + note + time range
    ProjectList.vue        # Project tree CRUD, colour swatches, archive/restore/delete, selection
    MatchRuleEditor.vue    # Condition builder for one rule (AND-ed conditions)
    MatchRuleList.vue      # Global rule precedence list, hit counts, warnings, drag reorder
    ConditionValueInput.vue# Search-as-you-type value picker backed by real activity
    AppIcon.vue            # Icon with coloured-initial avatar fallback
    FilterRuleList.vue, WindowSummary.vue, ProjectSummary.vue, PeriodGrid.vue, PeriodNav.vue,
    PeriodSummary.vue, SearchDayTimeline.vue, SearchMatchItem.vue, ScreenRecordingBanner.vue,
    TimerWidget.vue, TooltipOverlay.vue, ContextMenu.vue, IdlePrompt.vue
```

## Important implementation details

### Timeline math
- `HOUR_HEIGHT = 160` px per hour
- `minuteToY(min) = (min - startMin) / 60 * HOUR_HEIGHT`
- Drag/resize and the hover tooltip snap to `settings.snapMinutes` (default 5)
- `isoToMinutes(iso)` converts UTC ISO string → local-time minutes via `new Date(iso).getHours() * 60 + ...`
- Day keys are always **local** dates. Use `localDateKey()` from `composables/useFormat.ts`;
  `Date#toISOString().slice(0, 10)` gives the UTC date and is wrong for evening local times.

### Rust activity polling
- Polls every `POLL_INTERVAL_SECS = 20` seconds in a `tokio::time::sleep` loop
- Win32 functions only compiled on Windows (`cfg(target_os = "windows")`), all return `None`/`0` on other platforms
- On window change: flushes current session to `activity_raw` table
- On idle (> `idle_timeout_secs`): flushes partial session, emits `idle-return` Tauri event on return
- Also persists the owning process's full exe path (`activity_raw.exe_path`), used for app icons

### DB migrations (`db::run_migrations`)
- Versioned with `PRAGMA user_version`; bump `SCHEMA_VERSION` and add a `migrate_vN` step for shape changes
- Additive columns use the swallowed-error `ALTER TABLE … ADD COLUMN` pattern (no-op if present)
- **Never `ALTER TABLE … RENAME` a table that other tables reference by foreign key.** SQLite
  rewrites the referencing keys when a table is renamed, so renaming a parent leaves children
  pointing at a name that gets dropped. Reads keep working but every write fails with
  `no such table: <old name>`. Rebuild the child table instead.
- Pre-rule-group databases (flat `project_match_rules (rule_type, value)`) are **dropped and recreated** —
  there is no backwards compatibility for flat rules
- v2 repairs databases migrated by the original v1, which renamed `project_match_rules` out from under
  `project_match_rule_conditions`; see `repair_legacy_foreign_key`, which rebuilds the child table,
  preserves its rows, and runs on every startup (no-op once clean)
- Rule indexes are created *after* the tables are known to have their current shape

### DB merge/filter (`db::merge_and_filter`)
1. Apply ignore rules (filter_rules table) — drop matching raw events
2. Merge consecutive same-app events within `merge_gap_secs` (merge key is `window_id` when known)
3. Drop blocks shorter than `min_duration_secs`
4. Second merge pass over the surviving blocks
Returns `Vec<ActivityBlock>` with ISO start/end timestamps. Each block carries `window_title`
(last title seen, for display) **and** `window_titles` (every distinct title, for rule matching).

**Merge order matters.** Step 3 drops short blocks *before* step 4 re-merges, so a block below
`min_duration_secs` is **deleted, not absorbed**: `Slack 20s | brave 10s | Slack 1m` yields the 1m
block alone (the 20s is lost), whereas `Slack 70s | brave 10s | Slack 90s` becomes one 170s block.
Interstitial apps are only bridged when *both* same-app runs clear the minimum.

### Window Activity panel (`db::get_window_summary_for_date`)
- Aggregates **all** of the day's raw events (no min-duration filter), grouped by window title for
  `title_split_apps`, by an extracted project key for `title_group_apps`, and otherwise by `window_id`
- `WindowSummaryItem.window_id` carries that handle so a row can be matched back to blocks (0 for the
  two title-grouped kinds, and for legacy rows with no handle). `ActivityBlock.window_id` is sent to
  the frontend for the same reason — it used to be `#[serde(skip)]`, so check for it before assuming
  the frontend can see a handle
- A row's `window_title` is its *longest* segment's title, but a merged block's `window_title` is its
  *last* one. Matching a row to blocks on title alone therefore picks an arbitrary set. Match the way
  the row was grouped instead — see `sessionsFor` in `src/components/WindowSummary.vue`
- "Track to project…" creates **one entry per session**, never a single entry spanning the gaps
  between sessions: a row can legitimately cover sessions hours apart, and the span between them is
  not worked time
- The picker modal takes `initialRanges: EntryRange[]` (1..N) and emits `save(projectId, ranges, …)`,
  so it can never display one range while saving a different one

### Project match rules (`matcher.rs`)
- A rule is a named group of conditions combined with **AND**; rules are **OR**-ed across projects and
  evaluated in global `position` order — first match wins
- Condition = `{field: app_name|window_title, operator: contains|equals|starts_with|ends_with, value, negate}`
- Title conditions test **every** entry of `window_titles`, plus the `title_group_apps` project key for IDEs
- Conditions with blank values are dropped at compile time, so a rule can never match everything
- `get_rule_stats` returns per-rule hits / matched minutes / overlaps, which drives the "never matches"
  and "overlaps" warnings on the Projects page

### Search (`db::search`)
- Query tokens are `word`, `app:`, `title:`, `-word`, `-app:`, `-title:`, `date:`, `after:`, `before:`
- Dates are picked from raw `activity_raw` rows, then merged blocks are re-filtered with
  `block_matches` — which therefore tests **every** entry of `window_titles`, not just the
  representative one, or a day would be selected and then silently dropped
- Note matches only ever use bare tokens (an `app:`/`title:` qualifier never searches notes)

### Formatting helpers (`composables/useFormat.ts`)
Durations arrive in three units, so format them through the shared helpers rather than inlining a
new `Math.floor(x / 60)` per component:
- `formatMinutes(min)` — timeline entries and drag previews
- `formatSeconds(secs)` — Rust-side activity totals
- `formatElapsed(ms)` — the live timer / stopwatch display
- `minutesToClock(min)`, `localDateKey(date|string)`, `cleanWindowTitle(app, title)`

### Settings defaults (Settings::default in models.rs)
- `min_duration_secs`: 300
- `merge_gap_secs`: 120
- `idle_timeout_secs`: 300
- `timeline_start_hour`: 7
- `timeline_end_hour`: 22
- `snap_minutes`: 5
- `window_summary_min_secs`: 60
These are the **code** defaults; the persisted `settings` table can hold different user-chosen values
(and often does), so don't read them back out of a database and assume the code changed.
Note: settings are read/written key-by-key in `get_settings`/`save_settings`, so adding a field means
updating four places: the struct, `Default`, `get_settings`, and `save_settings`.

### Tauri config
- Window: 1100×780, min 900×600
- Close button hides to tray (CloseRequested intercepted in lib.rs)
- Tray menu: Show / Quit
- Capabilities: `core:default`, `core:tray:default`

## Known issues / things not yet verified

- `pnpm tauri dev` not run end-to-end yet (the crate compiles and `cargo test` passes)
- App icons only come from activity recorded *after* `exe_path` capture landed; older rows fall back
  to a coloured letter avatar until that app is used again
- Icon extraction is Windows-only by design (`appicon.rs` is a no-op elsewhere)
- Dark mode CSS tested visually but not on actual dark OS theme
- IPC failures are only logged to the console in most stores; the day view surfaces `loadError`
- Suggestions still respect `min_duration_secs`, so sub-minute visits produce none
- `pnpm exec eslint .` still reports errors in `mcp/server.js` and `scripts/`; `src/**` is clean
