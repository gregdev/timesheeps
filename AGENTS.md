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
    reminder.rs       # Daily timesheet reminder: scheduler, prompt payload, native toast
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

- `HOUR_HEIGHT = 160` px per hour is the **100% zoom** value; the live value is the shared
  `hourHeight` ref in `composables/useTimeline.ts` (module-level, so the ruler and both tracks
  cannot disagree). Ctrl+wheel over the timeline body zooms about the pointer, clamped to
  60–480 px/hour and persisted in `localStorage` (view state — deliberately not a `Settings`
  field). Because `hourHeight` is a ref, `minuteToY`/`yToMinute`/`totalHeight` are only reactive
  when **read inside a computed**; a plain function call in a template expression still tracks,
  but anything cached by hand must read `hourHeight.value`
- `minuteToY(min) = (min - startMin) / 60 * hourHeight`
- `timelineStartHour`/`timelineEndHour` are a **preference, not a bound**: `startMin`/`endMin`
  widen to fit the day's entries, suggestions and activity blocks (rounded to whole hours, clamped
  to 0:00–24:00) so recorded work is never invisible. Widening only ever adds hours. `isExpanded`
  drives the explanation in the ruler tooltip; `preferredStartMin`/`preferredEndMin` are the raw
  Settings values. Only the day timeline reads this geometry — `useTimeline()` elsewhere is used
  purely for `formatDuration`/`minutesToTime`/`isoToMinutes`
- The zoom control (`TimelineZoomControl`) lives in `TimelineView`'s `.day-actions` toolbar beside
  Copy, **not** in the column headers: the 44 px ruler gutter is narrower than the control, so
  anything placed there overlaps the "Activity" title. It is always visible and reads as a
  percentage of the 160 px/hour default; clicking the value resets to 100%
- Drag/resize and the hover tooltip snap to `settings.snapMinutes` (default 5)
- `isoToMinutes(iso)` converts UTC ISO string → local-time minutes via `new Date(iso).getHours() * 60 + ...`
- Day keys are always **local** dates. Use `localDateKey()` from `composables/useFormat.ts`;
  `Date#toISOString().slice(0, 10)` gives the UTC date and is wrong for evening local times
- Short entries have a **minimum rendered height of 8px** (`TimeBlockItem`), and the resize
  handles shrink to `height/4` so a small block keeps a clickable body. Two entries a few minutes
  apart still overlap visually at low zoom — that is what zoom is for; a click there lands on
  whichever block is later in DOM order

### Entry selection and merging

- `ctrl/cmd+click` toggles one entry, `shift+click` selects the range from the anchor, plain click
  selects just that entry, `Escape` clears. State lives in
  `composables/useEntrySelection.ts` (module-level), pruned whenever the entry list changes and
  cleared on a date change
- **A single click never opens the editor** — that is a double-click, so a stray click cannot
  interrupt what the user was doing. The `dblclick` handler sits on the outer `.time-block` (not
  `.block-body`) because a 2-minute entry is all resize handle and has no reliable body target
- The entry hover tooltip (`data-tooltip`) carries project name, time window + duration, the note
  when present, and the gesture hints. The block body only prints those lines above certain
  heights, so a short entry would otherwise be unidentifiable without opening the editor
- The click handler lives on `.block-body`, but `TimeBlockItem` routes **handle** presses through
  the same `select` event, so a 2-minute block (all handle, no body) is still selectable
- Editing a single entry is: double-click the block, or right-click → "Edit entry…"
- `Del` with a selection opens the delete confirmation (`ConfirmDialog`). The handler skips
  `INPUT`/`TEXTAREA`/`contenteditable` targets and refuses to stack on an open modal, so it cannot
  fire while typing in the timer note or the picker's project search
- Deletion confirms through `components/ConfirmDialog.vue`, not `window.confirm`: the native box
  cannot be themed and blocks the whole webview. It mirrors `ProjectPickerModal`'s styling; keep the
  two in step. **Focus starts on Cancel** and Enter is left to the focused button, so a reflexive
  Enter cancels rather than deleting. Its Escape listener is registered in the capture phase so it
  closes the dialog instead of reaching the timeline's clear-selection handler
- Pressing empty track clears the selection — it also starts a create-drag, and a stale
  "3 selected" in the header would keep offering Merge
- Bulk **Merge** (header button) folds the selection into one entry spanning earliest start to
  latest end. `dayStore.mergeBlocker` refuses mixed projects and any span that would cover an
  entry left out of the selection; the header then disables the button with that reason as its
  tooltip. Gaps inside the span become counted time — that is the point of the action, not a bug
- `Merge with previous entry` (right-click) is the single-pair version and is stricter: adjacent
  entries only, same project, non-overlapping
- **Undo/redo** (`dayStore.undo` / `redo`, Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y, plus toolbar buttons) is
  snapshot-based, not command-based: a day holds a few dozen entries, and a composite like Merge
  (one update plus N deletes) then undoes as a single step with no per-operation inverse to write.
  `withHistory()` records the pre-state only _after_ a successful mutation that actually changed the
  list, so a refused merge or an unchanged update leaves no step. `restore()` matches entries by id
  first and falls back to a content key, because undoing a delete re-creates the row with a **new
  backend id**; it then re-reads the day so ids stay authoritative. Stacks are per-day and cleared
  whenever `selectedDate` changes — restoring a snapshot captured on another date would write rows
  onto a day they never belonged to. Note that auto-accepted suggestions are not recorded, so undo
  can remove one; the suggestion then reappears, because suggestions are recomputed from activity
- Right-clicking an entry that is part of a multi-selection opens the **bulk** menu instead
  (Merge N / Delete N / Clear selection). Merge is rendered disabled when `mergeBlocker` refuses,
  rather than being offered and rejected. Bulk actions are _emitted_ to `TimelineCanvas`, which owns
  the confirm dialogs and the inline error, so the two merge paths cannot drift- Right-clicking an entry that is _not_ already selected highlights it (and anchors it) so it is
  clear which entry the menu acts on, and the highlight is dropped when the menu closes. That is
  tracked by a plain `contextMenuSelection` flag in `EntryTrack` — never a ref — so a deliberate
  multi-selection survives the bulk menu. The clearing watcher uses `flush: 'sync'` because the menu
  closes on mousedown and the next event may already be another entry's contextmenu
- `.col-headers` has a **fixed 32px height**, not content-driven: the 19.4px selection buttons are
  taller than the bare "My Time" label, so the row used to grow ~4px on selection and shove the
  timeline body down under the cursor. `.col-header` is a flex row with `padding: 0 8px` — vertical
  centring replaces the old 6px padding so the fixed height and the `border-left` still fill the row- `TimeBlockItem.startDrag` returns early for `e.button !== 0`. Without that guard a right-click's
  mousedown started a "move" drag whose mouseup was read as a click, opening the editor _and_ the
  context menu at the same time.

### Day navigation (`DayNav.vue`)

- A **Back to Today** button appears only when `dayStore.isViewingToday` is false, calling
  `dayStore.goToday`. On today the date label carries a "Today" badge instead
- The relative distance ("6 days ago") is a separate `.relative-badge` between the date picker and
  the button — never inside the button label. "Back to Today (6 days ago)" reads as though _today_
  were six days ago; on its own the badge clearly describes the date on screen

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

### Daily timesheet reminder (`reminder.rs` + `src/components/DailyReminder.vue`)

- `reminder::start(app)` spawns a tokio loop ticking every `TICK_SECS` (30s), re-reading
  `db::get_settings` each tick so enabling/disabling/retiming needs no restart — same shape as
  `activity::start_polling`
- Fires when `daily_reminder_enabled` and local time ≥ `daily_reminder_time` and the day has not
  already been claimed. Persisted time is stored in the `settings` table under
  `daily_reminder_last_fired`; it is **bookkeeping, deliberately not a `Settings` field**, so the
  auto-saving settings form can never round-trip and clobber it
- A malformed `daily_reminder_time` yields no popup at all (never a fallback time)
- The payload is built **before** the day is claimed, so a failure retries next tick instead of
  silently skipping; the claim is written **before** emitting, so a tick can never double-fire
- Totals come from `reporting::classify_range(…, GroupBy::Project)` — the same ignore-rule →
  merge → min-duration → match-rule pipeline the timeline uses, never raw `SUM` over
  `activity_raw`. It reports tracked secs, logged entry secs, and per-project rows
- Delivery is a `daily-reminder` Tauri event consumed in `App.vue` (mirroring `idle-return` /
  `IdlePrompt`), which renders the `DailyReminder` modal
- A native toast (via `tauri-plugin-notification`) is sent **only when the window had to be
  brought forward** — `reveal_window` returns that boolean. When the app is already on screen the
  modal is the whole message; when it was hidden in the tray the toast is the record in the Action
  Center. A toast failure is logged, never propagated, because permission can legitimately be
  refused and the prompt has already been shown
- **Windows toasts only carry the app's name and icon in an installed build.** Under
  `pnpm tauri dev` they are attributed to Windows PowerShell — that is the plugin, not a bug. The
  plugin needs Rust ≥ 1.90. `notification:default` is in `capabilities/default.json` only so a
  _frontend_ caller would work too; Rust's `app.notification()` is not ACL-gated, and no
  `@tauri-apps/plugin-notification` JS dependency is installed
- `reveal_window` restores the 1100×780 window only when it is hidden or still at the tray
  popup's 280 px width, so a reminder while the user is working does not move their layout
- Navigation back off `/timer-popup` happens in `App.vue`, not Rust: a `location.replace` eval
  would reload the webview and race the event listener
- Enabling the reminder after today's time has passed fires it on the next tick (the day is
  unclaimed), which is intentional feedback but means toggling it in Settings can pop it up
- `daily_reminder_enabled` defaults to **false**: a reminder appearing unbidden on upgrade is what
  the toggle exists to prevent

### Auto-accepting suggestions (`src/stores/day.ts`)

- The poller keeps a row **open** while a window stays focused, moving `ended_at` forward every
  20s. An active session's suggestion therefore always ends "now"
- A suggestion is only auto-accepted once it has been quiet for `AUTO_ACCEPT_SETTLE_MINUTES` (2).
  Accepting it earlier freezes the sliver recorded so far into an entry, and every later refresh
  then rejects the grown suggestion as an overlap — that is what produced 1-minute entries for
  20-minute sessions
- `loadDay` can run twice concurrently (the 30s interval plus the `activity-updated` event).
  `autoAccepting` is a plain, non-reactive guard: without it, overlapping passes both see the same
  suggestion as unclaimed and create a duplicate entry for it
- Manual cleanup is a right-click on an entry → "Merge with previous entry"
  (`dayStore.mergeWithPrevious`). It only merges **adjacent** entries of the same project (nothing
  between them, non-overlapping), keeps the earlier entry's id and start, extends its end, and keeps
  the earlier note unless it is blank. The menu item is only offered when the pair qualifies, and the
  store re-checks before writing because the entries can change while the menu is open

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
- `compute_suggestions` bridges same-project matches across gaps of up to `suggestion_gap_secs`
  (default 300). Merging only on **contact** was wrong: the poller leaves ~20 s between rows on
  every window switch, so an editor ↔ browser session produced one suggestion — and, with
  auto-accept on, one entry — per switch. A block claimed by a _different_ project still splits the
  run; bridging it would bill that project's minutes to this one's span

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
  Windows `resource_dir()` _is_ the executable's directory
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
- `suggestion_gap_secs`: 300 — how far apart two same-project matches may be and still become one
  suggested entry. Separate from `merge_gap_secs`, which is about merging _raw rows_ of the same window
- `idle_timeout_secs`: 300
- `timeline_start_hour`: 7
- `timeline_end_hour`: 22
- `snap_minutes`: 5
- `window_summary_min_secs`: 60
- `auto_accept_suggested`: false
- `daily_reminder_enabled`: false
- `daily_reminder_time`: "17:00"
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
- Entries damaged by the old bottom-edge resize clamp (`end_minutes < start_minutes`) are now
  rejected at the write path (`db::insert/update_time_entry` bail on an inverted range), and the
  2026-10-28 rows `1682`/`1686` were repaired by hand to `22:17–22:22` and `22:35–22:40`
  (end times taken from the underlying raw activity). Any _other_ DB may still contain some
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
