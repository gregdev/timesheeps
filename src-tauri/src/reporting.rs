//! Classified time reporting over a date range.
//!
//! This is the layer the MCP server exposes, and it deliberately reuses the
//! app's real pipeline — `db::get_activity_for_date` (ignore rules → merge by
//! window → drop sub-`min_duration_secs` blocks → re-merge) followed by
//! `matcher::first_match` — instead of summing raw `activity_raw` rows.
//!
//! That is a correctness requirement, not tidiness:
//!
//! * summing raw rows counts sub-minute window flapping that the app never
//!   shows, and on databases written by older builds could even go negative;
//! * raw `date(started_at, 'localtime')` SQL buckets by the UTC day of a
//!   local-time-formatted string and mishandles sessions that straddle midnight,
//!   whereas `db` resolves an explicit UTC range per local date;
//! * classification rules would otherwise have to be reimplemented in two
//!   places, and would drift.

use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};

use crate::db;
use crate::matcher;
use crate::models::{
    ClassifiedTotal, ClassifiedTotals, FilterRule, GroupBy, PerDayTotals, Settings,
    SubGroupWarning, UnclassifiedGroup, WindowSummaryItem,
};

/// Longest range a single call may classify. Guards the payload size and the
/// per-day raw-activity scans.
pub const MAX_RANGE_DAYS: i64 = 100;

/// Inclusive list of local `YYYY-MM-DD` dates between `start` and `end`.
pub fn date_range(start: &str, end: &str, max_days: i64) -> Result<Vec<String>> {
    let parse = |raw: &str, label: &str| -> Result<NaiveDate> {
        NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
            .map_err(|_| anyhow!("{} must be a YYYY-MM-DD date, got {:?}", label, raw))
    };
    let from = parse(start, "start_date")?;
    let to = parse(end, "end_date")?;

    if to < from {
        return Err(anyhow!(
            "end_date ({}) is before start_date ({})",
            end.trim(),
            start.trim()
        ));
    }

    let span = (to - from).num_days() + 1;
    if span > max_days {
        return Err(anyhow!(
            "range covers {} days, which exceeds the {} day maximum — call again with a smaller range",
            span,
            max_days
        ));
    }

    Ok((0..span)
        .map(|i| {
            (from + chrono::Duration::days(i))
                .format("%Y-%m-%d")
                .to_string()
        })
        .collect())
}

/// Bucket identity: (project, sub-group, app). Tuples order `None` first; the
/// final order is by size, so this only has to be deterministic.
type BucketKey = (Option<i64>, Option<String>, Option<String>);

/// Everything loaded once per call so the per-day loop only touches activity.
struct Context {
    settings: Settings,
    filter_rules: Vec<FilterRule>,
    compiled: Vec<matcher::CompiledRule>,
    patterns: HashMap<i64, matcher::SubGroupPattern>,
    warnings: Vec<SubGroupWarning>,
    project_name: HashMap<i64, String>,
}

impl Context {
    fn load(conn: &Connection) -> Result<Self> {
        let rules = db::get_project_match_rules(conn)?;
        let projects = db::get_projects(conn)?;
        let (patterns, warnings) = matcher::resolve_sub_groups(&rules, &projects);

        Ok(Context {
            settings: db::get_settings(conn)?,
            filter_rules: db::get_filter_rules(conn)?,
            compiled: matcher::compile(&rules),
            patterns,
            warnings,
            project_name: projects.iter().map(|p| (p.id, p.name.clone())).collect(),
        })
    }

    /// Merged, filtered blocks for one local date — exactly what the app shows.
    fn blocks_for(&self, conn: &Connection, date: &str) -> Result<Vec<crate::models::ActivityBlock>> {
        db::get_activity_for_date(conn, date, &self.settings, &self.filter_rules)
    }
}

/// Classify tracked activity across a range and compare it with logged entries.
///
/// `group_by` decides the bucket key. When grouping by project or ticket, blocks
/// no rule claimed are excluded from `buckets` and reported through
/// `unclassified` instead; when grouping by app every block is bucketed, since
/// an app grouping needs no rules.
pub fn classify_range(
    conn: &Connection,
    start: &str,
    end: &str,
    group_by: GroupBy,
) -> Result<ClassifiedTotals> {
    let dates = date_range(start, end, MAX_RANGE_DAYS)?;
    let ctx = Context::load(conn)?;

    let mut buckets: BTreeMap<BucketKey, ClassifiedTotal> = BTreeMap::new();
    let mut unclassified: BTreeMap<(String, String), (i64, Vec<String>)> = BTreeMap::new();
    let mut total_tracked_secs: i64 = 0;
    let mut total_logged_secs: i64 = 0;
    let mut unclassified_secs: i64 = 0;
    // Every date in the range gets an entry, so a zero-activity day is visible
    // rather than absent.
    let mut day_totals: BTreeMap<String, PerDayTotals> = dates
        .iter()
        .map(|d| (d.clone(), PerDayTotals::default()))
        .collect();

    for date in &dates {
        for block in ctx.blocks_for(conn, date)? {
            total_tracked_secs += block.duration_secs;
            day_totals
                .entry(date.clone())
                .or_default()
                .tracked_secs += block.duration_secs;

            let matched = matcher::first_match(&block, &ctx.compiled, &ctx.settings);
            let sub_group = matched.and_then(|rule| {
                matcher::sub_group_for_block(&block, rule.rule_id, &ctx.patterns, &ctx.settings)
            });

            if matched.is_none() {
                unclassified_secs += block.duration_secs;
                day_totals
                    .entry(date.clone())
                    .or_default()
                    .unclassified_secs += block.duration_secs;
                let entry = unclassified
                    .entry((block.app_name.clone(), block.window_title.clone()))
                    .or_insert((0, Vec::new()));
                entry.0 += block.duration_secs;
                for title in &block.window_titles {
                    if title != &block.window_title && !entry.1.contains(title) {
                        entry.1.push(title.clone());
                    }
                }
            }

            let key: BucketKey = match group_by {
                GroupBy::App => (None, None, Some(block.app_name.clone())),
                _ => match matched {
                    // Nothing to attribute it to; surfaced via `unclassified`.
                    None => continue,
                    Some(rule) => (
                        Some(rule.project_id),
                        if group_by == GroupBy::Ticket {
                            sub_group
                        } else {
                            None
                        },
                        None,
                    ),
                },
            };

            let bucket = buckets.entry(key.clone()).or_insert_with(|| ClassifiedTotal {
                project_id: key.0,
                project_name: key.0.and_then(|id| ctx.project_name.get(&id).cloned()),
                sub_group: key.1.clone(),
                app_name: key.2.clone(),
                tracked_secs: 0,
                logged_secs: 0,
                variance_secs: 0,
                per_day: BTreeMap::new(),
            });
            bucket.tracked_secs += block.duration_secs;
            *bucket.per_day.entry(date.clone()).or_insert(0) += block.duration_secs;
        }

        // Manual entries carry a project but no ticket and no app, so:
        // * grouping by project  → attributed to that project;
        // * grouping by ticket   → attributed to the no-sub-group bucket, which
        //   is itself informative ("logged to Lotteries without a ticket");
        // * grouping by app      → cannot be attributed at all.
        for entry in db::get_time_entries_for_date(conn, date)? {
            let secs = (entry.end_minutes - entry.start_minutes).max(0) * 60;
            total_logged_secs += secs;
            day_totals
                .entry(date.clone())
                .or_default()
                .logged_secs += secs;

            if group_by == GroupBy::App {
                continue;
            }

            let key: BucketKey = (Some(entry.project_id), None, None);
            let bucket = buckets.entry(key.clone()).or_insert_with(|| ClassifiedTotal {
                project_id: key.0,
                project_name: key.0.and_then(|id| ctx.project_name.get(&id).cloned()),
                sub_group: None,
                app_name: None,
                tracked_secs: 0,
                logged_secs: 0,
                variance_secs: 0,
                per_day: BTreeMap::new(),
            });
            bucket.logged_secs += secs;
            *bucket.per_day.entry(date.clone()).or_insert(0) += 0;
        }
    }

    let mut out: Vec<ClassifiedTotal> = buckets
        .into_values()
        .map(|mut b| {
            b.variance_secs = b.tracked_secs - b.logged_secs;
            b
        })
        .collect();
    out.sort_by(|a, b| {
        b.tracked_secs
            .cmp(&a.tracked_secs)
            .then_with(|| a.project_name.cmp(&b.project_name))
            .then_with(|| a.sub_group.cmp(&b.sub_group))
            .then_with(|| a.app_name.cmp(&b.app_name))
    });

    let mut unclassified_out: Vec<UnclassifiedGroup> = unclassified
        .into_iter()
        .map(|((app_name, window_title), (total_secs, other_titles))| {
            UnclassifiedGroup {
                app_name,
                window_title,
                total_secs,
                other_titles,
            }
        })
        .collect();
    unclassified_out.sort_by(|a, b| b.total_secs.cmp(&a.total_secs));

    Ok(ClassifiedTotals {
        start_date: dates.first().cloned().unwrap_or_else(|| start.to_string()),
        end_date: dates.last().cloned().unwrap_or_else(|| end.to_string()),
        group_by: group_by.as_str().to_string(),
        total_tracked_secs,
        total_logged_secs,
        buckets: out,
        unclassified_secs,
        unclassified: unclassified_out,
        per_day: day_totals,
        warnings: ctx.warnings,
    })
}

/// Unmatched merged activity across the range, grouped by app + representative
/// title — the raw material for writing new rules.
///
/// `min_secs` is applied to each group's **total across the whole range**, not
/// per day, so a title that only ever appears briefly still surfaces when it
/// recurs daily. That is exactly the "I keep seeing this and it isn't a project"
/// case that per-day filtering hides.
pub fn unclassified_titles(
    conn: &Connection,
    start: &str,
    end: &str,
    min_secs: i64,
) -> Result<Vec<UnclassifiedGroup>> {
    let dates = date_range(start, end, MAX_RANGE_DAYS)?;
    let ctx = Context::load(conn)?;

    let mut groups: BTreeMap<(String, String), (i64, Vec<String>)> = BTreeMap::new();

    for date in &dates {
        for block in ctx.blocks_for(conn, date)? {
            if matcher::first_match(&block, &ctx.compiled, &ctx.settings).is_some() {
                continue;
            }
            let entry = groups
                .entry((block.app_name.clone(), block.window_title.clone()))
                .or_insert((0, Vec::new()));
            entry.0 += block.duration_secs;
            for title in &block.window_titles {
                if title != &block.window_title && !entry.1.contains(title) {
                    entry.1.push(title.clone());
                }
            }
        }
    }

    let mut out: Vec<UnclassifiedGroup> = groups
        .into_iter()
        .filter(|(_, (secs, _))| *secs >= min_secs)
        .map(|((app_name, window_title), (total_secs, other_titles))| UnclassifiedGroup {
            app_name,
            window_title,
            total_secs,
            other_titles,
        })
        .collect();
    out.sort_by(|a, b| b.total_secs.cmp(&a.total_secs));
    Ok(out)
}

/// Window-level activity across a range.
///
/// Delegates each day to `db::get_window_summary_for_date` so the grouping
/// semantics (`title_split_apps`, `title_group_apps`, window handles) are
/// identical to the app's Window Activity panel, then re-aggregates by
/// (app, title) for the range. Window handles are per-day and so are dropped.
pub fn window_summary_range(
    conn: &Connection,
    start: &str,
    end: &str,
    min_secs: Option<i64>,
) -> Result<Vec<WindowSummaryItem>> {
    let dates = date_range(start, end, MAX_RANGE_DAYS)?;
    let settings = db::get_settings(conn)?;
    let threshold = min_secs.unwrap_or(settings.window_summary_min_secs);

    let mut totals: HashMap<(String, String), i64> = HashMap::new();
    for date in &dates {
        for item in db::get_window_summary_for_date(conn, date, &settings)? {
            *totals
                .entry((item.app_name, item.window_title))
                .or_insert(0) += item.total_secs;
        }
    }

    let mut out: Vec<WindowSummaryItem> = totals
        .into_iter()
        .filter(|(_, secs)| *secs >= threshold)
        .map(|((app_name, window_title), total_secs)| WindowSummaryItem {
            app_name,
            window_title,
            total_secs,
            window_id: 0,
        })
        .collect();
    out.sort_by(|a, b| b.total_secs.cmp(&a.total_secs));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{insert_activity, insert_project, insert_project_match_rule};
    use crate::models::{MatchCondition, MatchField, MatchOperator};
    use chrono::DateTime;
    use rusqlite::Connection;

    fn ts(s: &str) -> DateTime<chrono::Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&chrono::Utc)
    }

    fn cond(field: MatchField, value: &str) -> MatchCondition {
        MatchCondition {
            field,
            operator: MatchOperator::Contains,
            value: value.to_string(),
            negate: false,
        }
    }

    /// A migrated in-memory DB whose `min_duration_secs` is 0, so fixtures are
    /// not silently dropped by the pipeline.
    fn fresh_db() -> Connection {
        let conn = db::migrated_in_memory();
        db::set_setting_str(&conn, "min_duration_secs", "0").unwrap();
        conn
    }

    fn add(conn: &Connection, app: &str, title: &str, window_id: u64, from: &str, to: &str) {
        insert_activity(conn, app, title, window_id, "", &ts(from), &ts(to)).unwrap();
    }

    #[test]
    fn date_range_is_inclusive_and_validates() {
        let days = date_range("2026-09-01", "2026-09-03", 10).unwrap();
        assert_eq!(days, vec!["2026-09-01", "2026-09-02", "2026-09-03"]);

        assert!(date_range("2026-09-03", "2026-09-01", 10).is_err());
        assert!(date_range("nonsense", "2026-09-01", 10).is_err());
        assert!(date_range("2026-01-01", "2026-12-31", 10).is_err());
    }

    #[test]
    fn totals_span_days_and_carry_a_per_day_breakdown() {
        let conn = fresh_db();
        let project = insert_project(&conn, "Lotteries", "#6366f1", None, None).unwrap();
        insert_project_match_rule(&conn, project.id, "Jira", &[cond(MatchField::WindowTitle, "jira")], None)
            .unwrap();

        add(&conn, "brave", "[ELMS-1] Sprint - Jira", 1, "2026-09-01T09:00:00+00:00", "2026-09-01T10:00:00+00:00");
        add(&conn, "brave", "[ELMS-2] Sprint - Jira", 1, "2026-09-02T09:00:00+00:00", "2026-09-02T09:30:00+00:00");

        let out = classify_range(&conn, "2026-09-01", "2026-09-02", GroupBy::Project).unwrap();

        assert_eq!(out.total_tracked_secs, 5400);
        assert_eq!(out.buckets.len(), 1);
        assert_eq!(out.buckets[0].project_name.as_deref(), Some("Lotteries"));
        assert_eq!(out.buckets[0].tracked_secs, 5400);
        assert_eq!(out.buckets[0].per_day.len(), 2);
        assert_eq!(out.buckets[0].per_day["2026-09-01"], 3600);
        assert_eq!(out.buckets[0].per_day["2026-09-02"], 1800);
    }

    #[test]
    fn ticket_grouping_splits_one_project_into_its_tickets() {
        let conn = fresh_db();
        let project =
            insert_project(&conn, "Lotteries", "#6366f1", None, Some("ELMS-\\d+")).unwrap();
        insert_project_match_rule(&conn, project.id, "Jira", &[cond(MatchField::WindowTitle, "jira")], None)
            .unwrap();

        add(&conn, "brave", "[ELMS-1] Sprint - Jira", 1, "2026-09-01T09:00:00+00:00", "2026-09-01T10:00:00+00:00");
        add(&conn, "brave", "[ELMS-2] Sprint - Jira", 2, "2026-09-01T10:05:00+00:00", "2026-09-01T10:35:00+00:00");

        let out = classify_range(&conn, "2026-09-01", "2026-09-01", GroupBy::Ticket).unwrap();

        assert_eq!(out.buckets.len(), 2);
        let first = out.buckets.iter().find(|b| b.sub_group.as_deref() == Some("ELMS-1")).unwrap();
        let second = out.buckets.iter().find(|b| b.sub_group.as_deref() == Some("ELMS-2")).unwrap();
        assert_eq!(first.tracked_secs, 3600);
        assert_eq!(second.tracked_secs, 1800);
        assert_eq!(out.unclassified_secs, 0);
    }

    #[test]
    fn unclassified_blocks_are_excluded_from_buckets_and_listed_separately() {
        let conn = fresh_db();
        let project = insert_project(&conn, "Lotteries", "#6366f1", None, None).unwrap();
        insert_project_match_rule(&conn, project.id, "Jira", &[cond(MatchField::WindowTitle, "jira")], None)
            .unwrap();

        add(&conn, "brave", "[ELMS-1] Sprint - Jira", 1, "2026-09-01T09:00:00+00:00", "2026-09-01T10:00:00+00:00");
        add(&conn, "Code", "index.ts - second-brain - Code", 2, "2026-09-01T10:05:00+00:00", "2026-09-01T10:35:00+00:00");

        let out = classify_range(&conn, "2026-09-01", "2026-09-01", GroupBy::Project).unwrap();

        assert_eq!(out.buckets.len(), 1, "only the claimed project is a bucket");
        assert_eq!(out.unclassified_secs, 1800);
        assert_eq!(out.unclassified.len(), 1);
        assert_eq!(out.unclassified[0].app_name, "Code");
        assert_eq!(out.unclassified[0].total_secs, 1800);
        // Totals still account for everything the pipeline produced.
        assert_eq!(out.total_tracked_secs, 5400);
    }

    #[test]
    fn app_grouping_buckets_everything_and_ignores_rules() {
        let conn = fresh_db();
        add(&conn, "brave", "x - Jira", 1, "2026-09-01T09:00:00+00:00", "2026-09-01T10:00:00+00:00");
        add(&conn, "Code", "y - Code", 2, "2026-09-01T10:00:00+00:00", "2026-09-01T10:30:00+00:00");

        let out = classify_range(&conn, "2026-09-01", "2026-09-01", GroupBy::App).unwrap();
        assert_eq!(out.buckets.len(), 2);
        assert_eq!(out.buckets[0].app_name.as_deref(), Some("brave"));
        assert_eq!(out.buckets[0].tracked_secs, 3600);
    }

    #[test]
    fn logged_entries_reconcile_against_tracked_time() {
        let conn = fresh_db();
        let project = insert_project(&conn, "Lotteries", "#6366f1", None, None).unwrap();
        insert_project_match_rule(&conn, project.id, "Jira", &[cond(MatchField::WindowTitle, "jira")], None)
            .unwrap();
        add(&conn, "brave", "[ELMS-1] Sprint - Jira", 1, "2026-09-01T09:00:00+00:00", "2026-09-01T10:00:00+00:00");
        // 90 minutes logged against 60 tracked.
        db::insert_time_entry(&conn, "2026-09-01", project.id, 540, 630, "").unwrap();

        let out = classify_range(&conn, "2026-09-01", "2026-09-01", GroupBy::Project).unwrap();
        assert_eq!(out.total_logged_secs, 5400);
        assert_eq!(out.buckets[0].tracked_secs, 3600);
        assert_eq!(out.buckets[0].logged_secs, 5400);
        assert_eq!(out.buckets[0].variance_secs, -1800);
    }

    #[test]
    fn a_logged_entry_for_a_project_with_no_activity_still_reconciles() {
        let conn = fresh_db();
        let project = insert_project(&conn, "Admin", "#6366f1", None, None).unwrap();
        db::insert_time_entry(&conn, "2026-09-01", project.id, 540, 600, "").unwrap();

        let out = classify_range(&conn, "2026-09-01", "2026-09-01", GroupBy::Project).unwrap();
        assert_eq!(out.buckets.len(), 1);
        assert_eq!(out.buckets[0].project_name.as_deref(), Some("Admin"));
        assert_eq!(out.buckets[0].tracked_secs, 0);
        assert_eq!(out.buckets[0].logged_secs, 3600);
        assert_eq!(out.buckets[0].variance_secs, -3600);
    }

    #[test]
    fn a_rule_pattern_overrides_the_project_pattern_end_to_end() {
        let conn = fresh_db();
        let project = insert_project(&conn, "Lotteries", "#6366f1", None, Some("PROJ-\\d+")).unwrap();
        insert_project_match_rule(
            &conn,
            project.id,
            "Jira",
            &[cond(MatchField::WindowTitle, "jira")],
            Some("ELMS-\\d+"),
        )
        .unwrap();
        add(&conn, "brave", "[ELMS-7] Sprint - Jira", 1, "2026-09-01T09:00:00+00:00", "2026-09-01T09:30:00+00:00");

        let out = classify_range(&conn, "2026-09-01", "2026-09-01", GroupBy::Ticket).unwrap();
        assert_eq!(out.buckets[0].sub_group.as_deref(), Some("ELMS-7"));
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn an_invalid_pattern_warns_without_losing_classification() {
        let conn = fresh_db();
        let project =
            insert_project(&conn, "Lotteries", "#6366f1", None, Some("ELMS-(\\d+")).unwrap();
        insert_project_match_rule(&conn, project.id, "Jira", &[cond(MatchField::WindowTitle, "jira")], None)
            .unwrap();
        add(&conn, "brave", "[ELMS-1] Sprint - Jira", 1, "2026-09-01T09:00:00+00:00", "2026-09-01T09:30:00+00:00");

        let out = classify_range(&conn, "2026-09-01", "2026-09-01", GroupBy::Ticket).unwrap();
        assert_eq!(out.warnings.len(), 1);
        // The project still got its time; only the ticket breakdown is missing.
        assert_eq!(out.buckets.len(), 1);
        assert_eq!(out.buckets[0].project_name.as_deref(), Some("Lotteries"));
        assert_eq!(out.buckets[0].sub_group, None);
        assert_eq!(out.buckets[0].tracked_secs, 1800);
    }

    #[test]
    fn negative_duration_rows_never_reach_the_totals() {
        let conn = fresh_db();
        // Bypassing the write-path clamp, as an old build would have.
        conn.execute(
            "INSERT INTO activity_raw
                (started_at, ended_at, app_name, window_title, window_id, exe_path)
             VALUES ('2026-09-01T09:00:00+00:00', '2026-09-01T08:59:00+00:00',
                     'brave', 'x - Jira', 1, '')",
            [],
        )
        .unwrap();

        let out = classify_range(&conn, "2026-09-01", "2026-09-01", GroupBy::App).unwrap();
        assert!(out.total_tracked_secs >= 0, "no negative durations may leak out");
        assert!(out.buckets.iter().all(|b| b.tracked_secs >= 0));
    }

    #[test]
    fn unclassified_titles_thresholds_on_the_range_total() {
        let conn = fresh_db();
        // 30s a day for three days: 90s total, under the per-day filter the app
        // uses but over this threshold.
        for (i, date) in ["2026-09-01", "2026-09-02", "2026-09-03"].iter().enumerate() {
            let id = (i + 1) as u64;
            add(
                &conn,
                "brave",
                "Klaviyo - Brave",
                id,
                &format!("{date}T09:00:00+00:00"),
                &format!("{date}T09:00:30+00:00"),
            );
        }

        let groups = unclassified_titles(&conn, "2026-09-01", "2026-09-03", 60).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].window_title, "Klaviyo - Brave");
        assert_eq!(groups[0].total_secs, 90);

        let none = unclassified_titles(&conn, "2026-09-01", "2026-09-03", 120).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn unclassified_groups_sum_to_the_unclassified_total() {
        let conn = fresh_db();
        add(&conn, "brave", "a - Brave", 1, "2026-09-01T09:00:00+00:00", "2026-09-01T09:10:00+00:00");
        add(&conn, "Code", "b - Code", 2, "2026-09-01T09:10:00+00:00", "2026-09-01T09:25:00+00:00");

        let out = classify_range(&conn, "2026-09-01", "2026-09-01", GroupBy::Project).unwrap();
        let summed: i64 = out.unclassified.iter().map(|g| g.total_secs).sum();
        assert_eq!(summed, out.unclassified_secs);
    }

    #[test]
    fn window_summary_range_reaggregates_across_days() {
        let conn = fresh_db();
        add(&conn, "brave", "Klaviyo - Brave", 1, "2026-09-01T09:00:00+00:00", "2026-09-01T09:05:00+00:00");
        add(&conn, "brave", "Klaviyo - Brave", 9, "2026-09-02T09:00:00+00:00", "2026-09-02T09:05:00+00:00");

        // 5s per day, 10s across the range — below the app's 60s per-day floor.
        let out = window_summary_range(&conn, "2026-09-01", "2026-09-02", None).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].total_secs, 600);
        assert_eq!(out[0].window_id, 0, "a handle means nothing across days");
    }
}
