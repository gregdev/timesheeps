use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Raw activity (from DB) ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawActivity {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub app_name: String,
    pub window_title: String,
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
}

// ── Projects ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProject {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProject {
    pub id: i64,
    pub name: String,
    pub color: String,
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
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            min_duration_secs: 60,
            merge_gap_secs: 120,
            idle_timeout_secs: 300,
            timeline_start_hour: 7,
            timeline_end_hour: 22,
        }
    }
}
