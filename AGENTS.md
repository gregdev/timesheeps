# AGENTS.md — Timesheeps

Context for AI coding assistants picking up this project.

## What is this

A Tauri 2.0 desktop app that records the active window (app name + window title) every 20 seconds and displays a visual daily timeline. The user drags to create time blocks assigned to projects, then uses those as reference when manually filling out Harvest timesheets. No export, no browser extension — window title is sufficient.

## Status

Rust backend: ✅ complete and compiling on Windows (`cargo test` — 65 tests pass)
Frontend Vue/TS: ✅ complete (`pnpm run build` exits 0, `pnpm exec eslint src` clean)
Tauri config: ✅ complete, including bundling of the MCP sidecar

`pnpm tauri dev` has been run and the app starts cleanly, and the MCP server has been exercised
against a live database (reads, writes, validation failures and gate refusals) over stdio. Live
window tracking itself still has not been watched over a full working day.

## Tech stack

| Layer           | Technology                                                                                                |
| --------------- | --------------------------------------------------------------------------------------------------------- |
| Desktop shell   | Tauri 2.0                                                                                                 |
| Frontend        | Vue 3 + TypeScript + Vite                                                                                 |
| State           | Pinia                                                                                                     |
| Routing         | Vue Router 4 (`/`, `/week`, `/pay-period`, `/projects`, `/settings`, `/about`, `/search`, `/timer-popup`) |
| Validation      | Zod 3 (mirrors Rust models)                                                                               |
| Date utils      | date-fns 4                                                                                                |
| DB              | SQLite via `rusqlite` (bundled, no Tauri SQL plugin)                                                      |
| Win32           | `windows` crate 0.58 (behind `cfg(target_os = "windows")`)                                                |
| Package manager | **pnpm** (`pnpm@10.0.0` via corepack — never use npm)                                                     |

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
    bin/mcp_server.rs # MCP server over stdio (separate [[bin]] = timesheeps-mcp). Thin shim
                      # over timesheeps_lib; holds only validation + JSON shapes
    models.rs         # All serde structs + MatchField/MatchOperator/MatchCondition enums
    db.rs             # SQLite schema + migrations + all CRUD + merge_and_filter
    matcher.rs        # Rule evaluation: compile, block matching, suggestions, rule stats,
                      # and sub-group (ticket) pattern extraction
    reporting.rs      # Date-range classification for the MCP tools (project/ticket/app totals)
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
    useRegex.ts       # client-side regex check for ticket patterns (JS, so not identical to Rust)
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
- Rule indexes are created _after_ the tables are known to have their current shape

### DB merge/filter (`db::merge_and_filter`)

1. Apply ignore rules (filter_rules table) — drop matching raw events
2. Merge consecutive same-app events within `merge_gap_secs` (merge key is `window_id` when known)
3. Drop blocks shorter than `min_duration_secs`
4. Second merge pass over the surviving blocks
   Returns `Vec<ActivityBlock>` with ISO start/end timestamps. Each block carries `window_title`
   (last title seen, for display) **and** `window_titles` (every distinct title, for rule matching).

**Merge order matters.** Step 3 drops short blocks _before_ step 4 re-merges, so a block below
`min_duration_secs` is **deleted, not absorbed**: `Slack 20s | brave 10s | Slack 1m` yields the 1m
block alone (the 20s is lost), whereas `Slack 70s | brave 10s | Slack 90s` becomes one 170s block.
Interstitial apps are only bridged when _both_ same-app runs clear the minimum.

### Window Activity panel (`db::get_window_summary_for_date`)

- Aggregates **all** of the day's raw events (no min-duration filter), grouped by window title for
  `title_split_apps`, by an extracted project key for `title_group_apps`, and otherwise by `window_id`
- `WindowSummaryItem.window_id` carries that handle so a row can be matched back to blocks (0 for the
  two title-grouped kinds, and for legacy rows with no handle). `ActivityBlock.window_id` is sent to
  the frontend for the same reason — it used to be `#[serde(skip)]`, so check for it before assuming
  the frontend can see a handle
- A row's `window_title` is its _longest_ segment's title, but a merged block's `window_title` is its
  _last_ one. Matching a row to blocks on title alone therefore picks an arbitrary set. Match the way
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

### Ticket sub-group patterns (`matcher.rs` + `db.rs`)

- `projects.sub_group_pattern` is the default; `project_match_rules.sub_group_pattern` overrides it.
  Both are `TEXT NOT NULL DEFAULT ''` and surface as `Option<String>` (`''`/blank means none)
- Compiled case-insensitively via the `regex` crate. **Capture group 1 if the pattern has one,
  otherwise the whole match**, so both `ELMS-\d+` and `ELMS-(\d+)` work
- Extraction scans the same candidate titles that rule matching does (every `window_titles` entry
  plus the IDE title-group key), first capture wins
- **It is a labelling pass applied after `matcher::first_match`, never an input to it.** A bad
  pattern therefore cannot reclassify a block; it only yields no sub-group, and is reported as a
  `SubGroupWarning` (surfaced in `MatchRuleList`/`get_match_rules`)
- Resolution lives in `matcher::resolve_sub_groups(rules, projects)` → `(HashMap<rule_id, Pattern>,
Vec<SubGroupWarning>)`. `compile()` itself stays matching-only, so nothing about precedence or
  hit stats changed when this was added

### MCP server (`bin/mcp_server.rs`)

- Public API: `lib.rs` exposes `pub mod db|matcher|models|reporting`. A `[[bin]]` is a **separate
  crate**, so `pub(crate)` items are NOT reachable from it — that includes `db::merge_and_filter`,
  `extract_title_group_key` and `run_migrations`, except the `#[cfg(test)]` helper
  `db::migrated_in_memory`
- It must never migrate. On a stale schema it returns an actionable "launch Timesheeps once" error,
  because two processes racing `run_migrations` and `repair_legacy_foreign_key` is worse than a
  clear message. `db::open_at` applies WAL + `foreign_keys` + `busy_timeout=5000` and no DDL
- Classification goes through `reporting::classify_range`, which calls `db::get_activity_for_date`
  per day and then `matcher::first_match` — deliberately NOT raw `SUM(julianday(...))` SQL. The old
  MCP did that and reported sub-minute flapping the app never shows, plus negative totals from the
  bug above
- Write tools are gated by the `mcp_allow_writes` setting, re-read **per call**, with the gate list
  in `WRITE_TOOLS`. `validate_match_rules` is intentionally absent from that list: it writes nothing,
  so it stays usable when writes are off. There is no settings-write tool, so Claude cannot unlock
  itself
- Batches validate fully before writing anything, and tool failures set `isError: true` rather than
  burying a message in a successful payload

### Shipping the MCP binary

- **It needs no configuration.** Tauri bundles the extra `[[bin]]` target (`timesheeps-mcp`)
  automatically and places it beside the main executable. Verified by reading the generated
  `target/release/nsis/x64/installer.nsi`, which contains a `; Copy external binaries` block
  emitting `File /a "/oname=timesheeps-mcp.exe" "...\target\release\timesheeps-mcp.exe"` — with no
  `externalBin` in any config. The uninstaller deletes it, so upgrades replace it cleanly
- Consequently `find_mcp_binary` resolves in installed builds via its first branch, because on
  Windows `resource_dir()` *is* the executable's directory
- Do **not** add a `bundle.resources` entry for the sidecar. It was tried and reverted. It is
  redundant (the block above already ships the sidecar) and it fights the build: `tauri-build`
  **hard-fails** when a declared resource path is absent (`resource path ... doesn't exist`), and
  the sidecar does not exist until cargo has compiled it — after `build.rs` has run. Working around
  that needed an empty-directory resource plus a `beforeBundleCommand` staging script, which also
  dropped a stray 0-byte `binaries/.gitkeep` and an empty `binaries\` directory into `$INSTDIR`.
  NSIS deduplicates identical file content, so the duplicate copy cost only ~94 bytes of installer
  size — the cost was cruft, not bloat
- `externalBin` is likewise unnecessary: it demands a `-$TARGET_TRIPLE` filename (so a staging step)
  and buys nothing, since we never spawn the sidecar through the shell plugin

### Settings defaults (Settings::default in models.rs)

- `min_duration_secs`: 300
- `merge_gap_secs`: 120
- `idle_timeout_secs`: 300
- `timeline_start_hour`: 7
- `timeline_end_hour`: 22
- `snap_minutes`: 5
- `window_summary_min_secs`: 60
- `auto_accept_suggested`: false
- `mcp_allow_writes`: **true** — the MCP write gate; the persisted row usually exists, don't assume
  the default
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

- Live window tracking has not been watched over a full working day; the MCP server and the DB
  pipeline have been exercised, but not continuous recording
- **`update_activity_end` clamps `ended_at` to `started_at`**, because idle detection computes
  `now - idle_secs` and a foreground-window change while already idle used to write a row whose end
  preceded its start. Migration v3 repairs those rows. Anything summing `ended_at - started_at`
  directly should still not assume positivity
- App icons only come from activity recorded _after_ `exe_path` capture landed; older rows fall back
  to a coloured letter avatar until that app is used again
- Icon extraction is Windows-only by design (`appicon.rs` is a no-op elsewhere)
- Dark mode CSS tested visually but not on actual dark OS theme
- IPC failures are only logged to the console in most stores; the day view surfaces `loadError`
- Suggestions still respect `min_duration_secs`, so sub-minute visits produce none
- `pnpm exec eslint .` is clean across `src/**`, `mcp/**` and `scripts/`. `eslint.config.mjs`
  ignores `**/src-tauri/target/**`, whose codegen asset bundles are single-line minified JS that
  otherwise report "Invalid character" parse errors
- `pnpm tauri build` **exits 1** on the updater-signing step (`A public key has been found, but no
  private key` — needs `TAURI_SIGNING_PRIVATE_KEY`). The NSIS installer is still produced; the
  non-zero exit is not a build failure
- Running the **installed** app while the repo has a newer `SCHEMA_VERSION` downgrades the DB's
  `user_version` stamp, because `run_migrations` sets it unconditionally and never reads
  `current_schema_version` first. The MCP server then refuses every call until the app is
  reinstalled. Bump-only-in-lockstep is fine, but don't run an older build against a newer DB
- `pnpm tauri build` verified end to end: the sidecar is staged into the NSIS installer and placed
  beside `timesheeps.exe` (see "Shipping the MCP binary")
