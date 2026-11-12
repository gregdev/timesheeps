use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

// ── Raw activity (from DB) ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawActivity {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub app_name: String,
    pub window_title: String,
    /// HWND value captured at record time. Used as merge key so that sessions
    /// from the same window (different titles) merge, while different windows
    /// of the same app (e.g. two VS Code projects) stay separate.
    /// 0 = unknown (legacy data or non-Windows).
    pub window_id: u64,
    /// Full path of the executable that owned the window. Empty for rows
    /// recorded before this column existed. Used to extract app icons.
    pub exe_path: String,
}

// ── Merged/filtered activity block (sent to frontend) ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBlock {
    pub app_name: String,
    /// Representative (last-seen) title for display purposes.
    pub window_title: String,
    /// Every distinct window title observed inside this block, in order.
    /// Match rules test all of these, not just `window_title`.
    pub window_titles: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_secs: i64,
    /// Owning window handle, carried from RawActivity. Used by the second merge pass
    /// and sent to the frontend so a Window Activity row can be matched back to
    /// exactly the blocks that row aggregates. `default` keeps older payloads
    /// (and legacy rows, where it is 0) deserialisable.
    #[serde(default)]
    pub window_id: u64,
}

// ── Window summary (aggregate raw activity, no min-duration filter) ──────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSummaryItem {
    pub app_name: String,
    pub window_title: String,
    pub total_secs: i64,
    /// The window this row was grouped by, or 0 when it was grouped by title
    /// (title_split_apps) or by an extracted project key (title_group_apps).
    pub window_id: u64,
}

// ── Projects ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub parent_id: Option<i64>,
    /// Optional regex applied to window titles to pull out a sub-group key
    /// (e.g. `ELMS-(\\d+)` → `ELMS-5813`) so time inside this project can be
    /// broken down further. A rule's own pattern overrides this one.
    /// `None` (or blank) means no sub-grouping.
    pub sub_group_pattern: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProject {
    pub name: String,
    pub color: String,
    pub parent_id: Option<i64>,
    /// Missing when the client is older than the field; serde maps that to `None`.
    #[serde(default)]
    pub sub_group_pattern: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProject {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub parent_id: Option<i64>,
    #[serde(default)]
    pub sub_group_pattern: Option<String>,
}

// ── Time entries ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: i64,
    pub date: String,          // YYYY-MM-DD
    pub project_id: i64,
    pub start_minutes: i64,   // minutes from midnight
    pub end_minutes: i64,
    pub note: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTimeEntry {
    pub date: String,
    pub project_id: i64,
    pub start_minutes: i64,
    pub end_minutes: i64,
    pub note: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTimeEntry {
    pub id: i64,
    pub project_id: i64,
    pub start_minutes: i64,
    pub end_minutes: i64,
    pub note: String,
}

// ── Filter rules ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FilterRuleType {
    TitlePattern,
    AppName,
}

impl FilterRuleType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FilterRuleType::TitlePattern => "title_pattern",
            FilterRuleType::AppName => "app_name",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "title_pattern" => Some(FilterRuleType::TitlePattern),
            "app_name" => Some(FilterRuleType::AppName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRule {
    pub id: i64,
    pub rule_type: FilterRuleType,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFilterRule {
    pub rule_type: FilterRuleType,
    pub value: String,
}

// ── Settings ─────────────────────────────────────────────────────────────────

/// Serde fallback so an older frontend payload (one that predates the field)
/// cannot fail `save_settings` — and, just as importantly, cannot silently map
/// to `0`, which would disable gap-bridging for good.
fn default_suggestion_gap_secs() -> i64 {
    300
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub min_duration_secs: i64,
    pub merge_gap_secs: i64,
    /// How large a gap between two matched blocks may be and still become one
    /// suggested entry. Deliberately much larger than `merge_gap_secs`: the
    /// poller leaves one interval (~20s) between rows on every window switch,
    /// so without a tolerance every editor ↔ browser switch ends a suggestion
    /// and auto-accept turns one work stretch into a dozen entries.
    #[serde(default = "default_suggestion_gap_secs")]
    pub suggestion_gap_secs: i64,
    pub idle_timeout_secs: i64,
    pub timeline_start_hour: i64,
    pub timeline_end_hour: i64,
    pub start_on_login: bool,
    pub snap_minutes: i64,
    pub window_summary_min_secs: i64,
    /// App names (case-insensitive) that should be split by window title in the
    /// Window Activity summary instead of grouped by window ID. Useful for
    /// browsers where each tab has a distinct title but shares the same HWND.
    pub title_split_apps: Vec<String>,
    /// App names (case-insensitive) that should be grouped by a "project name"
    /// extracted from the window title, ignoring window_id. Useful for IDEs
    /// where closing/reopening gives a new HWND but the project is the same.
    /// The extraction strips the app-name suffix and any [...] bracket, then
    /// takes the last ` — ` or ` - ` segment as the group key.
    pub title_group_apps: Vec<String>,
    /// Day of week weeks start on: 0 = Sunday, 1 = Monday.
    pub week_starts_on: i64,
    /// Pay frequency: "weekly" or "fortnightly".
    pub pay_schedule_frequency: String,
    /// A known pay period start date (YYYY-MM-DD) used to anchor all pay period calculations.
    pub pay_schedule_anchor: String,
    /// Column split percentage for the Activity track in the timeline (0–100).
    /// 50 = equal split between Activity and My Time columns.
    pub timeline_col_split_pct: i64,
    /// Width of the Window Activity summary panel in pixels.
    pub layout_window_summary_width: i64,
    /// Width of the Project Time summary panel in pixels.
    pub layout_project_summary_width: i64,
    /// When true, suggested entries from project match rules are automatically
    /// converted to time entries without requiring manual acceptance.
    pub auto_accept_suggested: bool,
    /// When true, a prompt pops up once per local day at `daily_reminder_time`
    /// showing the hours tracked so far, as a nudge to fill in the timesheet.
    pub daily_reminder_enabled: bool,
    /// Local time of day (`HH:MM`, 24-hour) at which the daily reminder fires.
    pub daily_reminder_time: String,
    /// When false, the MCP server refuses every write tool, making the Claude
    /// integration read-only. Defaults to on. There is deliberately no MCP tool
    /// that can change this — Claude must not be able to unlock itself.
    pub mcp_allow_writes: bool,
    /// Minutes a normal working day is expected to account for, used only to
    /// show a shortfall on the period views (`0` disables it). Applied to
    /// Monday–Friday only, so a fortnightly total is not inflated by weekends.
    pub expected_minutes_per_day: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            min_duration_secs: 300,
            merge_gap_secs: 120,
            suggestion_gap_secs: default_suggestion_gap_secs(),
            idle_timeout_secs: 300,
            timeline_start_hour: 7,
            timeline_end_hour: 22,
            start_on_login: true,
            snap_minutes: 5,
            window_summary_min_secs: 60,
            title_split_apps: vec!["Brave".to_string(), "Chrome".to_string(), "Firefox".to_string(), "msedge".to_string(), "Opera".to_string(), "Vivaldi".to_string(), "Arc".to_string(), "Zen".to_string(), "Chromium".to_string()],
            title_group_apps: vec!["Code".to_string()],
            week_starts_on: 1,
            pay_schedule_frequency: "weekly".to_string(),
            pay_schedule_anchor: Local::now().format("%Y-%m-%d").to_string(),
            timeline_col_split_pct: 50,
            layout_window_summary_width: 220,
            layout_project_summary_width: 220,
            auto_accept_suggested: false,
            // Off by default: a reminder that appears unbidden on upgrade is
            // exactly what the toggle exists to prevent.
            daily_reminder_enabled: false,
            daily_reminder_time: "17:00".to_string(),
            mcp_allow_writes: true,
            // Off by default: a shortfall only means something once the user
            // says how long their normal day is.
            expected_minutes_per_day: 0,
        }
    }
}

// ── Project match rules ───────────────────────────────────────────────────────

/// Which activity field a condition is tested against.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchField {
    AppName,
    WindowTitle,
}

impl MatchField {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchField::AppName => "app_name",
            MatchField::WindowTitle => "window_title",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "app_name" => Some(MatchField::AppName),
            "window_title" => Some(MatchField::WindowTitle),
            _ => None,
        }
    }
}

/// How a condition's value is compared against the field.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchOperator {
    Contains,
    Equals,
    StartsWith,
    EndsWith,
}

impl MatchOperator {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchOperator::Contains => "contains",
            MatchOperator::Equals => "equals",
            MatchOperator::StartsWith => "starts_with",
            MatchOperator::EndsWith => "ends_with",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "contains" => Some(MatchOperator::Contains),
            "equals" => Some(MatchOperator::Equals),
            "starts_with" => Some(MatchOperator::StartsWith),
            "ends_with" => Some(MatchOperator::EndsWith),
            _ => None,
        }
    }

    /// Case-insensitive comparison of `value` against `candidate`.
    pub fn test(&self, candidate: &str, value: &str) -> bool {
        let candidate = candidate.to_lowercase();
        let value = value.to_lowercase();
        match self {
            MatchOperator::Contains => candidate.contains(&value),
            MatchOperator::Equals => candidate == value,
            MatchOperator::StartsWith => candidate.starts_with(&value),
            MatchOperator::EndsWith => candidate.ends_with(&value),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchCondition {
    pub field: MatchField,
    pub operator: MatchOperator,
    pub value: String,
    /// When true the condition asserts the opposite ("is not").
    pub negate: bool,
}

/// A named group of AND-ed conditions. Rules are OR-ed together across projects,
/// evaluated in explicit `position` order — first match wins.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMatchRule {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub position: i64,
    pub conditions: Vec<MatchCondition>,
    /// Optional regex that overrides the owning project's pattern for the blocks
    /// this rule claims. Purely a labelling pass — it never affects whether the
    /// rule matches, so a bad pattern cannot reclassify time.
    pub sub_group_pattern: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectMatchRule {
    pub project_id: i64,
    pub name: String,
    pub conditions: Vec<MatchCondition>,
    #[serde(default)]
    pub sub_group_pattern: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectMatchRule {
    pub id: i64,
    pub name: String,
    pub conditions: Vec<MatchCondition>,
    #[serde(default)]
    pub sub_group_pattern: Option<String>,
}

/// Per-rule effectiveness over a recent window, used to surface dead rules.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleStat {
    pub rule_id: i64,
    pub project_id: i64,
    pub hits: i64,
    pub matched_secs: i64,
    pub last_matched_at: Option<String>,
    /// Projects whose rules also matched activity covered by this rule.
    pub overlaps_with: Vec<i64>,
}

/// An application observed in recorded activity, for the value picker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownApp {
    pub app_name: String,
    pub total_secs: i64,
    pub last_seen: String,
    pub exe_path: String,
    /// Most time-consuming window titles for this app, real values to match on.
    pub sample_titles: Vec<String>,
}

// ── Suggested entries ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedEntry {
    pub project_id: i64,
    /// The rule that produced this suggestion.
    pub rule_id: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
}

// ── Search ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaySearchResult {
    pub date: String,
    pub all_blocks: Vec<ActivityBlock>,
    pub matched_blocks: Vec<ActivityBlock>,
    pub total_matched_secs: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub days: Vec<DaySearchResult>,
    pub note_matches: Vec<TimeEntry>,
}

// ── Reporting (date-range classification, used by the MCP server) ────────────

/// How classified time is bucketed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    /// One bucket per project.
    Project,
    /// One bucket per (project, sub-group) — the sub-group being whatever the
    /// ticket pattern captured, or no sub-group if there is no pattern.
    Ticket,
    /// One bucket per application, ignoring rules entirely.
    App,
}

impl GroupBy {
    pub fn as_str(&self) -> &'static str {
        match self {
            GroupBy::Project => "project",
            GroupBy::Ticket => "ticket",
            GroupBy::App => "app",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "project" => Some(GroupBy::Project),
            "ticket" | "sub_group" | "subgroup" => Some(GroupBy::Ticket),
            "app" | "application" => Some(GroupBy::App),
            _ => None,
        }
    }
}

/// Tracked vs manually logged seconds for one bucket.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifiedTotal {
    /// `None` when grouping by app, or for activity no rule claimed.
    pub project_id: Option<i64>,
    pub project_name: Option<String>,
    /// The captured sub-group key (e.g. `ELMS-5813`), when grouping by ticket.
    pub sub_group: Option<String>,
    /// Only set when grouping by app.
    pub app_name: Option<String>,
    pub tracked_secs: i64,
    /// Seconds logged manually against this project.
    ///
    /// Time entries carry only a project (no ticket or app), so when grouping by
    /// ticket this is attributed to the bucket with no sub-group, and when
    /// grouping by app it is always 0.
    pub logged_secs: i64,
    /// `tracked - logged`. Negative means more was logged than tracked.
    pub variance_secs: i64,
    /// Local date (`YYYY-MM-DD`) → tracked seconds, ascending.
    pub per_day: std::collections::BTreeMap<String, i64>,
}

/// A window title that no rule claimed, with the extra titles seen alongside it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnclassifiedGroup {
    pub app_name: String,
    /// Representative (last-seen) title of the merged blocks in this group.
    pub window_title: String,
    /// Exact tracked seconds. These sum to `ClassifiedTotals::unclassified_secs`.
    pub total_secs: i64,
    /// Other distinct titles seen inside the same blocks, to help write a rule.
    /// Informational only — **not** additive with `total_secs`.
    pub other_titles: Vec<String>,
}

/// Per-date rollup, so a range can be sliced by day without the caller having to
/// re-derive it from the buckets.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerDayTotals {
    pub tracked_secs: i64,
    /// Tracked seconds on this date that no rule claimed.
    pub unclassified_secs: i64,
    pub logged_secs: i64,
}

/// Result of classifying a date range against the project match rules.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifiedTotals {
    pub start_date: String,
    pub end_date: String,
    pub group_by: String,
    /// Tracked seconds after ignore rules, merging and `min_duration_secs` — the
    /// same number the app's timeline would show.
    pub total_tracked_secs: i64,
    pub total_logged_secs: i64,
    /// Buckets, ordered by tracked seconds descending.
    pub buckets: Vec<ClassifiedTotal>,
    /// Merged activity no rule claimed. These blocks are *excluded* from
    /// `buckets` when grouping by project or ticket (they have no project), but
    /// are still listed when grouping by app.
    pub unclassified_secs: i64,
    pub unclassified: Vec<UnclassifiedGroup>,
    /// Local date → rollup, including days with no activity at all.
    pub per_day: std::collections::BTreeMap<String, PerDayTotals>,
    /// Patterns that failed to compile. Matching was unaffected by them.
    pub warnings: Vec<SubGroupWarning>,
}

/// A sub-group pattern that could not be compiled.
///
/// Reported rather than silently dropped: a typo must be visible, but it must
/// never break classification, so the owning rule still matches normally.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubGroupWarning {
    pub rule_id: i64,
    pub project_id: i64,
    pub pattern: String,
    pub error: String,
}

// ── Period review (the "fill in my timesheet" view) ──────────────────────────

/// A stretch of a day with neither tracked activity nor a logged entry, inside
/// the day's recorded envelope.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapRange {
    /// Minutes from local midnight.
    pub start_minutes: i64,
    pub end_minutes: i64,
}

/// Matched activity on a day that no time entry covers yet — an auto-accept
/// candidate the poller has not settled, or one skipped because it overlapped.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnclaimedSuggestion {
    /// Local `YYYY-MM-DD`, so a range result stays attributable to one day.
    pub date: String,
    pub project_id: i64,
    pub rule_id: i64,
    pub start_minutes: i64,
    pub end_minutes: i64,
}

/// One day of a period review. Everything the frontend needs to decide whether
/// the day is done, without another IPC call.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayReview {
    pub date: String,
    /// Merged, filtered activity attributed to any project or not.
    pub tracked_secs: i64,
    /// Seconds of manually logged entries.
    pub logged_secs: i64,
    /// Tracked seconds no match rule claimed — the "unknown work" signal.
    pub unclassified_secs: i64,
    /// Weekday target from `Settings::expected_minutes_per_day`, or 0.
    pub expected_minutes: i64,
    pub entries: Vec<TimeEntry>,
    /// The day note, or an empty string.
    pub note: String,
    /// Matched activity not yet covered by an entry.
    pub suggestions: Vec<UnclaimedSuggestion>,
    /// Calendar meetings on this day that no entry covers yet.
    pub meetings: Vec<MeetingCandidate>,
    /// Untracked holes inside the recorded day.
    pub gaps: Vec<GapRange>,
    /// Human-readable reasons this day needs a look. Empty means "nothing
    /// obvious is missing".
    pub attention: Vec<String>,
    /// `!attention.is_empty()`, precomputed so a caller cannot forget the rule.
    pub needs_attention: bool,
}

/// A whole pay period laid out for review.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodReview {
    pub start_date: String,
    pub end_date: String,
    /// Every date in the range, in order — including days with no activity.
    pub days: Vec<DayReview>,
    pub total_tracked_secs: i64,
    pub total_logged_secs: i64,
    /// Sum of the weekday targets. 0 when `expected_minutes_per_day` is 0.
    pub total_expected_minutes: i64,
    pub total_unclassified_secs: i64,
    /// Every unclaimed suggestion in the range, flattened and in time order.
    pub suggestions: Vec<UnclaimedSuggestion>,
}

// ── Timer ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TimerStatus {
    Stopped,
    Running,
    Paused,
}

impl TimerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimerStatus::Stopped => "stopped",
            TimerStatus::Running => "running",
            TimerStatus::Paused => "paused",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerState {
    pub status: TimerStatus,
    pub project_id: Option<i64>,
    pub project_name: Option<String>,
    pub project_color: Option<String>,
    pub note: String,
    pub started_at: Option<DateTime<Utc>>,
    pub accumulated_ms: i64,
    pub paused_at: Option<DateTime<Utc>>,
    /// Total elapsed millis displayed to user (wall-clock based)
    pub elapsed_ms: i64,
}

impl Default for TimerState {
    fn default() -> Self {
        TimerState {
            status: TimerStatus::Stopped,
            project_id: None,
            project_name: None,
            project_color: None,
            note: String::new(),
            started_at: None,
            accumulated_ms: 0,
            paused_at: None,
            elapsed_ms: 0,
        }
    }
}

// ── Microsoft 365 / Calendar ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    /// Stable identity of the *series* (Graph's `iCalUId`, or a synthesised key
    /// for other sources). Every occurrence of a recurring meeting shares it, so
    /// it is what a remembered project or a dismissal is keyed on.
    #[serde(default)]
    pub i_cal_uid: String,
    pub subject: String,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub is_all_day: bool,
    pub organizer: String,
    pub location: String,
    pub is_teams_meeting: bool,
    /// `accepted` / `declined` / `tentative` / `notResponded` / `organizer`.
    /// A declined meeting is never offered as a candidate.
    #[serde(default)]
    pub response: String,
}

/// A calendar event that no time entry covers yet — the meeting half of the
/// "what is still missing" view.
///
/// Meetings are the biggest blind spot in window tracking: the time is spent in
/// a room or on a call, so nothing is recorded, and a fortnight later it is
/// simply gone from memory.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingCandidate {
    /// Series identity, used to remember a project and to dismiss the meeting.
    pub i_cal_uid: String,
    pub subject: String,
    pub start_minutes: i64,
    pub end_minutes: i64,
    pub organizer: String,
    pub location: String,
    pub is_teams_meeting: bool,
    /// The project this meeting was previously logged to, if any.
    pub project_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M365Status {
    pub connected: bool,
    pub account_name: String,
}

/// State of the device-code sign-in flow, polled by the frontend while it waits.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodePrompt {
    /// The short code the user types at `verification_uri`.
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds until the code stops being accepted.
    pub expires_in: i64,
}
