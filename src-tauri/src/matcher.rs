//! Rule evaluation for project auto-match rules.
//!
//! Model: a rule is a named group of conditions that are **AND**-ed together.
//! Rules are **OR**-ed across projects and evaluated in explicit `position`
//! order — the first rule that matches a block wins.
//!
//! Every title observed inside a merged block is tested, not just the
//! representative `window_title`. This matters because blocks are merged by
//! HWND, so a long browsing or editor session collapses into one block whose
//! single title would otherwise hide everything else that happened in it.

use chrono::{DateTime, Utc};

use crate::db;
use crate::models::{
    ActivityBlock, MatchField, MatchOperator, Project, ProjectMatchRule, RuleStat, Settings,
    SubGroupWarning, SuggestedEntry,
};

/// A rule with its conditions prepared for evaluation.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub rule_id: i64,
    pub project_id: i64,
    pub position: i64,
    conditions: Vec<CompiledCondition>,
}

#[derive(Debug, Clone)]
struct CompiledCondition {
    field: MatchField,
    operator: MatchOperator,
    value: String,
    negate: bool,
}

/// One rule winning one block.
#[derive(Debug, Clone)]
pub struct RuleMatch {
    pub rule_id: i64,
    pub project_id: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
}

/// Prepare rules for evaluation: drop conditions with blank values (an empty
/// `contains` matches everything, which is never what the user meant) and sort
/// into precedence order. A rule left with no conditions never matches.
pub fn compile(rules: &[ProjectMatchRule]) -> Vec<CompiledRule> {
    let mut compiled: Vec<CompiledRule> = rules
        .iter()
        .map(|rule| {
            let conditions: Vec<CompiledCondition> = rule
                .conditions
                .iter()
                .filter(|c| !c.value.trim().is_empty())
                .map(|c| CompiledCondition {
                    field: c.field,
                    operator: c.operator,
                    value: c.value.clone(),
                    negate: c.negate,
                })
                .collect();
            CompiledRule {
                rule_id: rule.id,
                project_id: rule.project_id,
                position: rule.position,
                conditions,
            }
        })
        .filter(|r| !r.conditions.is_empty())
        .collect();
    compiled.sort_by_key(|r| (r.position, r.rule_id));
    compiled
}

/// Every string a title-based condition should be tested against.
///
/// Includes all titles seen inside the block, plus — for apps configured in
/// `title_group_apps` (IDEs) — the project name extracted from the title, so
/// rules can be written against "buyflow" rather than a whole file path.
fn candidate_titles(block: &ActivityBlock, settings: &Settings) -> Vec<String> {
    let mut out = block.window_titles.clone();

    if !out.iter().any(|t| t == &block.window_title) {
        out.push(block.window_title.clone());
    }

    let grouped = settings
        .title_group_apps
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&block.app_name));
    if grouped {
        if let Some(key) = db::extract_title_group_key(&block.app_name, &block.window_title) {
            if !out.iter().any(|t| t == &key) {
                out.push(key);
            }
        }
    }

    out
}

/// Evaluate one condition. A negated condition asserts that *no* candidate
/// matches, so "title is not X" holds for a block where every title avoids X.
fn condition_holds(cond: &CompiledCondition, block: &ActivityBlock, titles: &[String]) -> bool {
    let any_match = match cond.field {
        MatchField::AppName => cond.operator.test(&block.app_name, &cond.value),
        MatchField::WindowTitle => titles
            .iter()
            .any(|t| cond.operator.test(t, &cond.value)),
    };

    if cond.negate {
        !any_match
    } else {
        any_match
    }
}

/// All rules that match `block`, in precedence order.
pub fn block_matching_rules<'a>(
    block: &ActivityBlock,
    rules: &'a [CompiledRule],
    settings: &Settings,
) -> Vec<&'a CompiledRule> {
    let titles = candidate_titles(block, settings);
    rules
        .iter()
        .filter(|rule| {
            rule.conditions
                .iter()
                .all(|cond| condition_holds(cond, block, &titles))
        })
        .collect()
}

/// The winning rule for a block: first match in precedence order.
pub fn first_match<'a>(
    block: &ActivityBlock,
    rules: &'a [CompiledRule],
    settings: &Settings,
) -> Option<&'a CompiledRule> {
    let titles = candidate_titles(block, settings);
    rules.iter().find(|rule| {
        rule.conditions
            .iter()
            .all(|cond| condition_holds(cond, block, &titles))
    })
}

/// One `RuleMatch` per block that a rule claimed.
pub fn compute_matches(
    blocks: &[ActivityBlock],
    rules: &[CompiledRule],
    settings: &Settings,
) -> Vec<RuleMatch> {
    blocks
        .iter()
        .filter_map(|block| {
            first_match(block, rules, settings).map(|rule| RuleMatch {
                rule_id: rule.rule_id,
                project_id: rule.project_id,
                started_at: block.started_at,
                ended_at: block.ended_at,
            })
        })
        .collect()
}

/// Suggestions for the timeline: consecutive matches for the same project are
/// merged into a single span.
pub fn compute_suggestions(
    blocks: &[ActivityBlock],
    rules: &[CompiledRule],
    settings: &Settings,
) -> Vec<SuggestedEntry> {
    let matches = compute_matches(blocks, rules, settings);
    let mut merged: Vec<SuggestedEntry> = Vec::new();

    for m in matches {
        let last = merged.last_mut();
        match last {
            Some(last) if last.project_id == m.project_id && m.started_at <= last.ended_at => {
                if m.ended_at > last.ended_at {
                    last.ended_at = m.ended_at;
                }
            }
            _ => merged.push(SuggestedEntry {
                project_id: m.project_id,
                rule_id: m.rule_id,
                started_at: m.started_at,
                ended_at: m.ended_at,
            }),
        }
    }

    merged
}

/// Per-rule effectiveness across a window of days, plus simple overlap
/// detection: if two rules from *different* projects both match the same block,
/// each rule records the other project.
pub fn compute_rule_stats(
    days: &[(String, Vec<ActivityBlock>)],
    rules: &[CompiledRule],
    settings: &Settings,
) -> Vec<RuleStat> {
    let mut stats: Vec<RuleStat> = rules
        .iter()
        .map(|rule| RuleStat {
            rule_id: rule.rule_id,
            project_id: rule.project_id,
            hits: 0,
            matched_secs: 0,
            last_matched_at: None,
            overlaps_with: Vec::new(),
        })
        .collect();

    let index_of: std::collections::HashMap<i64, usize> = stats
        .iter()
        .enumerate()
        .map(|(i, s)| (s.rule_id, i))
        .collect();

    for (_, blocks) in days {
        for block in blocks {
            let matched = block_matching_rules(block, rules, settings);

            for (rank, rule) in matched.iter().enumerate() {
                let Some(&idx) = index_of.get(&rule.rule_id) else {
                    continue;
                };

                // Only the winning rule is credited with the time.
                if rank == 0 {
                    stats[idx].hits += 1;
                    stats[idx].matched_secs += block.duration_secs;
                    let ended = block.ended_at.to_rfc3339();
                    let is_newer = stats[idx]
                        .last_matched_at
                        .as_ref()
                        .map(|prev| ended > *prev)
                        .unwrap_or(true);
                    if is_newer {
                        stats[idx].last_matched_at = Some(ended);
                    }
                }

                // A block claimed by two rules from different projects is a
                // conflict; record it on both sides.
                for other in matched.iter() {
                    if other.project_id != rule.project_id
                        && !stats[idx].overlaps_with.contains(&other.project_id)
                    {
                        stats[idx].overlaps_with.push(other.project_id);
                    }
                }
            }
        }
    }

    for stat in &mut stats {
        stat.overlaps_with.sort_unstable();
        stat.overlaps_with.dedup();
    }
    stats
}

// ── Sub-group (ticket) extraction ─────────────────────────────────────────────
//
// This is a *labelling* pass that runs after a rule has already been chosen by
// `first_match`. It deliberately has no influence on which project a block
// belongs to, so a mistyped pattern can never reclassify work — the worst case
// is a missing ticket breakdown.

/// A compiled sub-group pattern.
#[derive(Debug, Clone)]
pub struct SubGroupPattern {
    regex: regex::Regex,
    /// True when the pattern declares at least one capture group.
    /// `captures_len()` counts group 0, so anything above 1 is a real group.
    uses_group_one: bool,
}

impl SubGroupPattern {
    /// Compile a pattern.
    ///
    /// Matching is case-insensitive: window titles vary in casing far more than
    /// ticket keys do. The error is returned rather than swallowed so a typo is
    /// visible in the UI and to the MCP caller.
    pub fn compile(pattern: &str) -> Result<Self, String> {
        let regex = regex::RegexBuilder::new(pattern.trim())
            .case_insensitive(true)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            uses_group_one: regex.captures_len() > 1,
            regex,
        })
    }

    /// The first capture across `titles`, or `None` if nothing matches.
    ///
    /// Group 1 is used when the pattern declares a group (so `ELMS-(\d+)`
    /// yields the bare number when the caller wants it), otherwise the whole
    /// match — which lets `ELMS-\d+` work without the user adding parentheses.
    pub fn extract(&self, titles: &[String]) -> Option<String> {
        for title in titles {
            let Some(caps) = self.regex.captures(title) else {
                continue;
            };
            let matched = if self.uses_group_one {
                caps.get(1).or_else(|| caps.get(0))
            } else {
                caps.get(0)
            };
            if let Some(m) = matched {
                let text = m.as_str().trim();
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            }
        }
        None
    }
}

/// Effective sub-group pattern per rule id.
///
/// A rule's own pattern overrides the owning project's. Rules with no pattern at
/// either level are absent from the map. Compile failures come back as warnings
/// and the rule simply yields no sub-group.
pub fn resolve_sub_groups(
    rules: &[ProjectMatchRule],
    projects: &[Project],
) -> (std::collections::HashMap<i64, SubGroupPattern>, Vec<SubGroupWarning>) {
    let project_pattern: std::collections::HashMap<i64, &str> = projects
        .iter()
        .filter_map(|p| p.sub_group_pattern.as_deref().map(|pat| (p.id, pat)))
        .collect();

    let mut compiled = std::collections::HashMap::new();
    let mut warnings = Vec::new();

    for rule in rules {
        let pattern = rule
            .sub_group_pattern
            .as_deref()
            .or_else(|| project_pattern.get(&rule.project_id).copied());
        let Some(pattern) = pattern else { continue };
        if pattern.trim().is_empty() {
            continue;
        }
        match SubGroupPattern::compile(pattern) {
            Ok(c) => {
                compiled.insert(rule.id, c);
            }
            Err(error) => warnings.push(SubGroupWarning {
                rule_id: rule.id,
                project_id: rule.project_id,
                pattern: pattern.to_string(),
                error,
            }),
        }
    }

    (compiled, warnings)
}

/// The sub-group key for a block already claimed by `rule_id`.
///
/// Scans the same candidate titles that rule matching uses (every title seen in
/// the merged block, plus the IDE project key), so a ticket glimpsed in a tab
/// that has since closed is still attributed.
pub fn sub_group_for_block(
    block: &ActivityBlock,
    rule_id: i64,
    patterns: &std::collections::HashMap<i64, SubGroupPattern>,
    settings: &Settings,
) -> Option<String> {
    let pattern = patterns.get(&rule_id)?;
    pattern.extract(&candidate_titles(block, settings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MatchCondition;
    use chrono::TimeZone;

    fn block(app: &str, titles: &[&str]) -> ActivityBlock {
        let start = Utc.with_ymd_and_hms(2026, 9, 23, 9, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 9, 23, 10, 0, 0).unwrap();
        ActivityBlock {
            app_name: app.to_string(),
            window_title: titles.last().unwrap().to_string(),
            window_titles: titles.iter().map(|t| t.to_string()).collect(),
            started_at: start,
            ended_at: end,
            duration_secs: 3600,
            window_id: 1,
        }
    }

    fn rule(id: i64, project_id: i64, position: i64, conditions: Vec<MatchCondition>) -> ProjectMatchRule {
        ProjectMatchRule {
            id,
            project_id,
            name: format!("rule {id}"),
            position,
            conditions,
            sub_group_pattern: None,
        }
    }

    fn cond(field: MatchField, operator: MatchOperator, value: &str, negate: bool) -> MatchCondition {
        MatchCondition {
            field,
            operator,
            value: value.to_string(),
            negate,
        }
    }

    fn settings() -> Settings {
        let mut s = Settings::default();
        s.title_group_apps.clear();
        s
    }

    #[test]
    fn matches_titles_other_than_the_representative_one() {
        let b = block("brave", &["New tab", "[ELMS-5815] Waitlist - Jira", "Overview — Bitbucket"]);
        let rules = compile(&[rule(
            1,
            7,
            0,
            vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "jira", false)],
        )]);
        assert_eq!(compute_matches(&[b], &rules, &settings()).len(), 1);
    }

    #[test]
    fn and_semantics_require_every_condition() {
        let b = block("brave", &["Lottery Buyflow - Brave"]);
        let both = compile(&[rule(
            1,
            7,
            0,
            vec![
                cond(MatchField::WindowTitle, MatchOperator::Contains, "buyflow", false),
                cond(MatchField::WindowTitle, MatchOperator::Contains, "lottery", true),
            ],
        )]);
        assert!(compute_matches(&[b.clone()], &both, &settings()).is_empty());

        let just_buyflow = compile(&[rule(
            1,
            7,
            0,
            vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "buyflow", false)],
        )]);
        assert_eq!(compute_matches(&[b], &just_buyflow, &settings()).len(), 1);
    }

    #[test]
    fn negation_scans_every_title_in_the_block() {
        // "buyflow" appears in one title, so "is not buyflow" must fail.
        let b = block("Code", &["index.ts - buyflow - Code", "notes.md - seima - Code"]);
        let exclude = compile(&[rule(
            1,
            7,
            0,
            vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "buyflow", true)],
        )]);
        assert!(compute_matches(&[b], &exclude, &settings()).is_empty());
    }

    #[test]
    fn operators_behave_as_expected() {
        let b = block("olk", &["Inbox - Greg Smith - Outlook"]);
        let cases = vec![
            (MatchOperator::Contains, "greg smith", true),
            (MatchOperator::Equals, "Inbox - Greg Smith - Outlook", true),
            (MatchOperator::Equals, "Inbox", false),
            (MatchOperator::StartsWith, "inbox", true),
            (MatchOperator::EndsWith, "outlook", true),
            (MatchOperator::EndsWith, "inbox", false),
        ];
        for (op, value, expected) in cases {
            let rules = compile(&[rule(
                1,
                7,
                0,
                vec![cond(MatchField::WindowTitle, op, value, false)],
            )]);
            assert_eq!(
                !compute_matches(&[b.clone()], &rules, &settings()).is_empty(),
                expected,
                "{op:?} {value:?}"
            );
        }
    }

    #[test]
    fn app_name_condition_uses_exact_field() {
        let b = block("olk", &["Inbox"]);
        let hit = compile(&[rule(
            1,
            7,
            0,
            vec![cond(MatchField::AppName, MatchOperator::Equals, "OLK", false)],
        )]);
        assert_eq!(compute_matches(&[b.clone()], &hit, &settings()).len(), 1);

        let miss = compile(&[rule(
            1,
            7,
            0,
            vec![cond(MatchField::AppName, MatchOperator::Contains, "Outlook", false)],
        )]);
        assert!(compute_matches(&[b], &miss, &settings()).is_empty());
    }

    #[test]
    fn first_rule_in_position_order_wins() {
        let b = block("brave", &["soa-elms / buyflow — Bitbucket - Brave"]);
        let rules = compile(&[
            rule(1, 10, 5, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "bitbucket", false)]),
            rule(2, 20, 1, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "buyflow", false)]),
        ]);
        let matches = compute_matches(&[b.clone()], &rules, &settings());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].project_id, 20, "lower position wins");

        // Reordering flips the winner.
        let rules = compile(&[
            rule(1, 10, 0, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "bitbucket", false)]),
            rule(2, 20, 9, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "buyflow", false)]),
        ]);
        assert_eq!(compute_matches(&[b], &rules, &settings())[0].project_id, 10);
    }

    #[test]
    fn blank_values_and_empty_rules_never_match_everything() {
        let b = block("brave", &["anything at all"]);
        let blank = compile(&[rule(
            1,
            7,
            0,
            vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "   ", false)],
        )]);
        assert!(blank.is_empty(), "blank conditions are dropped at compile time");
        assert!(compute_matches(&[b.clone()], &blank, &settings()).is_empty());

        let none = compile(&[rule(1, 7, 0, vec![])]);
        assert!(compute_matches(&[b], &none, &settings()).is_empty());
    }

    #[test]
    fn group_app_titles_are_offered_as_a_candidate() {
        let b = block("Code", &["wrangler.jsonc - soa-buyflow [WSL: Ubuntu] - Visual Studio Code"]);
        let mut s = settings();
        s.title_group_apps = vec!["Code".to_string()];

        let rules = compile(&[rule(
            1,
            7,
            0,
            vec![cond(MatchField::WindowTitle, MatchOperator::Equals, "soa-buyflow", false)],
        )]);
        assert_eq!(compute_matches(&[b], &rules, &s).len(), 1);
    }

    #[test]
    fn consecutive_same_project_matches_merge() {
        let mk = |h: u32| ActivityBlock {
            app_name: "Code".to_string(),
            window_title: "buyflow - Code".to_string(),
            window_titles: vec!["buyflow - Code".to_string()],
            started_at: Utc.with_ymd_and_hms(2026, 9, 23, h, 0, 0).unwrap(),
            ended_at: Utc.with_ymd_and_hms(2026, 9, 23, h + 1, 0, 0).unwrap(),
            duration_secs: 3600,
            window_id: 1,
        };
        let rules = compile(&[rule(
            1,
            7,
            0,
            vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "buyflow", false)],
        )]);
        let suggestions = compute_suggestions(&[mk(9), mk(10)], &rules, &settings());
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].rule_id, 1);
        assert_eq!(suggestions[0].ended_at, Utc.with_ymd_and_hms(2026, 9, 23, 11, 0, 0).unwrap());
    }

    #[test]
    fn stats_flag_zero_hit_rules_and_overlaps() {
        let b = block("Code", &["index.ts - buyflow - Code"]);
        let rules = compile(&[
            rule(1, 10, 0, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "buyflow", false)]),
            rule(2, 20, 1, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "index.ts", false)]),
            rule(3, 30, 2, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "nowhere", false)]),
        ]);
        let stats = compute_rule_stats(&[("2026-09-23".to_string(), vec![b])], &rules, &settings());

        let winner = stats.iter().find(|s| s.rule_id == 1).unwrap();
        assert_eq!(winner.hits, 1);
        assert_eq!(winner.matched_secs, 3600);
        assert!(winner.last_matched_at.is_some());
        assert_eq!(winner.overlaps_with, vec![20]);

        let loser = stats.iter().find(|s| s.rule_id == 2).unwrap();
        assert_eq!(loser.hits, 0);
        assert_eq!(loser.overlaps_with, vec![10]);

        let dead = stats.iter().find(|s| s.rule_id == 3).unwrap();
        assert_eq!(dead.hits, 0);
        assert!(dead.overlaps_with.is_empty());
    }

    // ── Sub-group (ticket) extraction ─────────────────────────────────────────

    fn project(id: i64, pattern: Option<&str>) -> Project {
        Project {
            id,
            name: format!("project {id}"),
            color: "#6366f1".to_string(),
            archived_at: None,
            parent_id: None,
            sub_group_pattern: pattern.map(str::to_string),
        }
    }

    fn with_pattern(
        mut r: ProjectMatchRule,
        pattern: &str,
    ) -> ProjectMatchRule {
        r.sub_group_pattern = Some(pattern.to_string());
        r
    }

    fn titles(list: &[&str]) -> Vec<String> {
        list.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn a_pattern_without_a_group_returns_the_whole_match() {
        let p = SubGroupPattern::compile("ELMS-\\d+").unwrap();
        assert_eq!(
            p.extract(&titles(&["[ELMS-5813] Waitlist - Jira"])),
            Some("ELMS-5813".to_string())
        );
    }

    #[test]
    fn a_pattern_with_a_group_returns_group_one() {
        let p = SubGroupPattern::compile("ELMS-(\\d+)").unwrap();
        assert_eq!(
            p.extract(&titles(&["[ELMS-5813] Waitlist - Jira"])),
            Some("5813".to_string())
        );
    }

    #[test]
    fn extraction_is_case_insensitive() {
        let p = SubGroupPattern::compile("ELMS-\\d+").unwrap();
        assert_eq!(
            p.extract(&titles(&["elms-5813 - Jira"])),
            Some("elms-5813".to_string())
        );
    }

    #[test]
    fn extraction_scans_every_title_in_the_block() {
        let p = SubGroupPattern::compile("ELMS-\\d+").unwrap();
        // The ticket is only in an earlier title; the representative one is a tab.
        assert_eq!(
            p.extract(&titles(&["[ELMS-5815] Waitlist - Jira", "New tab"])),
            Some("ELMS-5815".to_string())
        );
        assert_eq!(p.extract(&titles(&["New tab", "Inbox"])), None);
    }

    #[test]
    fn a_rules_pattern_overrides_its_projects() {
        let rules = [with_pattern(
            rule(1, 7, 0, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "jira", false)]),
            "ELMS-\\d+",
        )];
        let projects = [project(7, Some("PROJ-\\d+"))];

        let (patterns, warnings) = resolve_sub_groups(&rules, &projects);
        assert!(warnings.is_empty());
        assert_eq!(
            patterns[&1].extract(&titles(&["[ELMS-9] x - Jira"])),
            Some("ELMS-9".to_string())
        );
    }

    #[test]
    fn a_rule_without_a_pattern_falls_back_to_its_projects() {
        let rules = [rule(
            1,
            7,
            0,
            vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "jira", false)],
        )];
        let projects = [project(7, Some("ELMS-\\d+"))];

        let (patterns, warnings) = resolve_sub_groups(&rules, &projects);
        assert!(warnings.is_empty());
        assert_eq!(
            patterns[&1].extract(&titles(&["[ELMS-9] x - Jira"])),
            Some("ELMS-9".to_string())
        );
    }

    #[test]
    fn a_project_without_a_pattern_yields_no_sub_group() {
        let rules = [rule(
            1,
            7,
            0,
            vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "jira", false)],
        )];
        let (patterns, warnings) = resolve_sub_groups(&rules, &[project(7, None)]);
        assert!(patterns.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn an_invalid_pattern_warns_and_never_breaks_matching() {
        // Unbalanced bracket: a plausible typo. The rule must still classify the
        // block; only the ticket breakdown is lost, and the reason is reported.
        let b = block("brave", &["[ELMS-5813] Waitlist - Jira"]);
        let rules = [with_pattern(
            rule(1, 7, 0, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "jira", false)]),
            "ELMS-(\\d+",
        )];

        let (patterns, warnings) = resolve_sub_groups(&rules, &[]);
        assert!(patterns.is_empty());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].rule_id, 1);
        assert_eq!(warnings[0].pattern, "ELMS-(\\d+");

        let compiled = compile(&rules);
        assert_eq!(compute_matches(&[b], &compiled, &settings()).len(), 1);
    }

    #[test]
    fn sub_group_for_block_reads_the_candidate_titles() {
        let b = block("brave", &["[ELMS-5815] Waitlist - Jira", "New tab"]);
        let rules = [with_pattern(
            rule(1, 7, 0, vec![cond(MatchField::WindowTitle, MatchOperator::Contains, "jira", false)]),
            "ELMS-\\d+",
        )];
        let (patterns, _) = resolve_sub_groups(&rules, &[]);

        assert_eq!(
            sub_group_for_block(&b, 1, &patterns, &settings()),
            Some("ELMS-5815".to_string())
        );
        // A rule with no resolved pattern yields nothing rather than panicking.
        assert_eq!(sub_group_for_block(&b, 999, &patterns, &settings()), None);
    }
}
