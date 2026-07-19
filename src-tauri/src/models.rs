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
}

// ── Merged/filtered activity block (sent to frontend) ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBlock {
    pub app_name: String,
    pub window_title: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_secs: i64,
    /// Carried from RawActivity for the second merge pass; not sent to frontend.
    #[serde(skip)]
    pub window_id: u64,
}

// ── Window summary (aggregate raw activity, no min-duration filter) ──────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSummaryItem {
    pub app_name: String,
    pub window_title: String,
    pub total_secs: i64,
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProject {
    pub name: String,
    pub color: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProject {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub parent_id: Option<i64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub min_duration_secs: i64,
    pub merge_gap_secs: i64,
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
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            min_duration_secs: 300,
            merge_gap_secs: 120,
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
        }
    }
}

// ── Project match rules ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMatchRule {
    pub id: i64,
    pub project_id: i64,
    pub rule_type: FilterRuleType,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectMatchRule {
    pub project_id: i64,
    pub rule_type: FilterRuleType,
    pub value: String,
}

// ── Suggested entries ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedEntry {
    pub project_id: i64,
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
