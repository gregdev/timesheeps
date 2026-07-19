//! Natural-language → keyword query translator.
//!
//! Converts user-friendly phrases like "what did I do yesterday" into the
//! existing keyword-search syntax ("date:2026-06-24") so the rest of the
//! search pipeline works unchanged.
//!
//! Dates are resolved via `chrono-english` (day names, "last monday", etc.)
//! with simple arithmetic for the most common cases ("yesterday", "today").
//!
//! App names are extracted from patterns like "on Figma", "using VS Code".

use chrono::{Datelike, Duration, Local, NaiveDate};
use chrono_english::{parse_date_string, Dialect};

// ── Word lists ──────────────────────────────────────────────────────────────

/// Words filtered out as semantically empty in a time-tracking query.
const NOISE: &[&str] = &[
    "a", "am", "an", "and", "are", "at", "be", "been", "by", "can", "did",
    "do", "does", "doing", "for", "from", "has", "have", "how", "i", "in",
    "is", "it", "its", "me", "much", "my", "of", "on", "or", "show",
    "spent", "that", "the", "these", "this", "those", "time", "to", "up",
    "was", "were", "what", "with", "worked", "working",
];

/// Prepositions that introduce an app name: "on Figma", "using VS Code", etc.
const APP_PREPS: &[&str] = &["in", "on", "using", "with"];

/// Words that start a multi-word date expression.
const DATE_STARTERS: &[&str] = &["last", "next", "past", "this"];

/// Single-word date expressions handled directly (trivial arithmetic).
const DATE_SINGLES: &[&str] = &["today", "tomorrow", "yesterday"];

/// Day-of-week names.
const DAYS: &[&str] = &[
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
];

/// Month names / three-letter abbreviations → (name, number).
const MONTHS: &[(&str, u32)] = &[
    ("jan", 1), ("january", 1),
    ("feb", 2), ("february", 2),
    ("mar", 3), ("march", 3),
    ("apr", 4), ("april", 4),
    ("may", 5),
    ("jun", 6), ("june", 6),
    ("jul", 7), ("july", 7),
    ("aug", 8), ("august", 8),
    ("sep", 9), ("september", 9),
    ("oct", 10), ("october", 10),
    ("nov", 11), ("november", 11),
    ("dec", 12), ("december", 12),
];

// ── Public API ──────────────────────────────────────────────────────────────

/// Attempts to convert a natural-language query into keyword-search syntax.
///
/// Returns `Some("keyword string")` if the input was recognised as NL and
/// successfully translated.  Returns `None` if the input already uses keyword
/// syntax (contains `:`) or couldn't be parsed — callers should fall back to
/// the normal `parse_search_query` path.
pub fn parse_nl(query: &str) -> Option<String> {
    if !looks_like_nl(query) {
        return None;
    }

    let lower = query.to_lowercase();
    let mut words: Vec<&str> = lower.split_whitespace().collect();
    let now = Local::now();

    let date_token = extract_date(&mut words, &now);
    let app_tokens = extract_apps(&mut words);
    let remaining = filter_noise(&words);

    if date_token.is_none() && app_tokens.is_empty() && remaining.is_empty() {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();
    if let Some(dt) = date_token {
        parts.push(dt);
    }
    parts.extend(app_tokens);
    if !remaining.is_empty() {
        parts.push(remaining);
    }

    Some(parts.join(" "))
}

// ── Guards ──────────────────────────────────────────────────────────────────

fn looks_like_nl(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Presence of ':' anywhere → already keyword syntax; let the existing
    // parser handle it.
    !trimmed.contains(':')
}

fn is_noise(w: &str) -> bool {
    NOISE.contains(&w)
        || DATE_STARTERS.contains(&w)
        || APP_PREPS.contains(&w)
        || DATE_SINGLES.contains(&w)
        || DAYS.contains(&w)
        || MONTHS.iter().any(|(n, _)| *n == w)
}

// ── Date extraction ─────────────────────────────────────────────────────────

/// Scan `words` for the first date expression, remove its constituent words,
/// and return the equivalent keyword-style token(s).
///
/// Returns a single string that may contain multiple space-separated tokens
/// (e.g. `"after:2026-06-16 before:2026-06-22"`).
fn extract_date(words: &mut Vec<&str>, now: &chrono::DateTime<Local>) -> Option<String> {
    let today = now.date_naive();

    let mut i = 0;
    while i < words.len() {
        // -- single word --------------------------------------------------
        if let Some(result) = try_single_date(words[i], now) {
            words.remove(i);
            return Some(result);
        }

        // -- three-word phrase ("last 7 days", "past 2 weeks") -------------
        // Must come BEFORE two-word: "last 3 days" → 3-word wins over
        // the ambiguous "last 3" that chrono-english would misinterpret.
        if i + 2 < words.len() {
            if let Some(result) =
                try_three_word_date(words[i], words[i + 1], words[i + 2], &today)
            {
                words.remove(i);
                words.remove(i);
                words.remove(i);
                return Some(result);
            }
        }

        // -- two-word phrase ----------------------------------------------
        if i + 1 < words.len() {
            let phrase = format!("{} {}", words[i], words[i + 1]);
            if let Some(result) =
                try_two_word_date(&phrase, words[i], words[i + 1], now)
            {
                words.remove(i);
                words.remove(i); // index shifts after first remove
                return Some(result);
            }
        }

        i += 1;
    }

    None
}

fn try_single_date(word: &str, now: &chrono::DateTime<Local>) -> Option<String> {
    let today = now.date_naive();

    match word {
        "yesterday" => {
            let d = today - Duration::days(1);
            Some(format!("date:{}", d.format("%Y-%m-%d")))
        }
        "today" => Some(format!("date:{}", today.format("%Y-%m-%d"))),
        "tomorrow" => {
            let d = today + Duration::days(1);
            Some(format!("date:{}", d.format("%Y-%m-%d")))
        }
        day if DAYS.contains(&day) => {
            // "friday" → chrono-english resolves to the coming Friday
            parse_date_string(day, *now, Dialect::Us)
                .ok()
                .map(|dt| format!("date:{}", dt.date_naive().format("%Y-%m-%d")))
        }
        month_str => {
            // "january", "feb", etc. → entire month range
            MONTHS
                .iter()
                .find(|(n, _)| *n == month_str)
                .and_then(|(_, m)| month_range(today.year(), *m))
        }
    }
}

fn try_two_word_date(
    phrase: &str,
    first: &str,
    second: &str,
    now: &chrono::DateTime<Local>,
) -> Option<String> {
    let today = now.date_naive();

    // "last/this/next week" and "last/this/next month" — chrono-english
    // doesn't handle these, so we compute them directly.
    if DATE_STARTERS.contains(&first) {
        match (first, second) {
            ("last", "week") => {
                let d = today - Duration::weeks(1);
                return week_range(d);
            }
            ("this", "week") => return week_range(today),
            ("next", "week") => {
                let d = today + Duration::weeks(1);
                return week_range(d);
            }
            ("last", "month") => {
                let (y, m) = prev_month(today.year(), today.month());
                return month_range(y, m);
            }
            ("this", "month") => return month_range(today.year(), today.month()),
            ("next", "month") => {
                let (y, m) = next_month(today.year(), today.month());
                return month_range(y, m);
            }
            _ => { /* fall through to chrono-english for day names */ }
        }
    }

    // "last/this/next <dayname>" — chrono-english handles these.
    if DATE_STARTERS.contains(&first) && first != "past" {
        if let Ok(dt) = parse_date_string(phrase, *now, Dialect::Us) {
            let d = dt.date_naive();
            return Some(format!(
                "date:{}",
                d.format("%Y-%m-%d")
            ));
        }
    }

    None
}

fn try_three_word_date(
    first: &str,
    second: &str,
    third: &str,
    today: &NaiveDate,
) -> Option<String> {
    // "last N days", "past 2 weeks", "last 3 months"
    if (first == "last" || first == "past") && second.parse::<i64>().is_ok() {
        let n: i64 = second.parse().unwrap();

        return match third {
            "day" | "days" => {
                let d = *today - Duration::days(n);
                Some(format!("after:{}", d.format("%Y-%m-%d")))
            }
            "week" | "weeks" => {
                let d = *today - Duration::weeks(n as i64);
                Some(format!("after:{}", d.format("%Y-%m-%d")))
            }
            "month" | "months" => {
                let mut year = today.year();
                let mut month = today.month() as i32 - n as i32;
                while month <= 0 {
                    year -= 1;
                    month += 12;
                }
                NaiveDate::from_ymd_opt(year, month as u32, 1)
                    .map(|d| format!("after:{}", d.format("%Y-%m-%d")))
            }
            _ => None,
        };
    }

    None
}

// ── Date helpers ────────────────────────────────────────────────────────────

/// Monday–Sunday range containing `d`.
fn week_range(d: NaiveDate) -> Option<String> {
    let wd = d.weekday().num_days_from_monday();
    let mon = d - Duration::days(wd as i64);
    let sun = mon + Duration::days(6);
    Some(format!(
        "after:{} before:{}",
        mon.format("%Y-%m-%d"),
        sun.format("%Y-%m-%d"),
    ))
}

/// Full month range for the given year/month.
fn month_range(year: i32, month: u32) -> Option<String> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let last = last_day_of_month(year, month)?;
    Some(format!(
        "after:{} before:{}",
        first.format("%Y-%m-%d"),
        last.format("%Y-%m-%d"),
    ))
}

fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?.pred_opt()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?.pred_opt()
    }
}

fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

// ── App extraction ──────────────────────────────────────────────────────────

/// Scan for "on X", "using X", "in X", "with X" patterns and produce `app:X`
/// tokens.  Matched words are removed from `words`.
fn extract_apps(words: &mut Vec<&str>) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if APP_PREPS.contains(&words[i]) && i + 1 < words.len() {
            let candidate = words[i + 1];
            // Only treat as app name if the candidate isn't a known noise /
            // date word.
            if !is_noise(candidate) {
                tokens.push(format!("app:{}", candidate));
                words.remove(i); // preposition
                words.remove(i); // app name (shifted after first remove)
                continue;
            }
        }
        i += 1;
    }
    tokens
}

// ── Noise filtering ─────────────────────────────────────────────────────────

/// Keep only words that aren't noise / date / preposition cruft.
/// Returns them space-joined (plain terms match app_name OR window_title).
fn filter_noise(words: &[&str]) -> String {
    let meaningful: Vec<&str> = words.iter().filter(|w| !is_noise(*w)).copied().collect();
    meaningful.join(" ")
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_falls_through() {
        assert_eq!(parse_nl("app:figma"), None);
        assert_eq!(parse_nl("date:2026-06-24"), None);
        assert_eq!(parse_nl("after:2026-01-01 before:2026-01-31"), None);
        assert_eq!(parse_nl("app:code title:budget"), None);
    }

    #[test]
    fn empty_and_whitespace() {
        assert_eq!(parse_nl(""), None);
        assert_eq!(parse_nl("   "), None);
    }

    #[test]
    fn yesterday_today_tomorrow() {
        let today = Local::now().date_naive();
        let yest = today - Duration::days(1);
        let tomo = today + Duration::days(1);

        let r = parse_nl("yesterday").unwrap();
        assert!(r.contains(&format!("date:{}", yest.format("%Y-%m-%d"))));

        let r = parse_nl("today").unwrap();
        assert!(r.contains(&format!("date:{}", today.format("%Y-%m-%d"))));

        let r = parse_nl("tomorrow").unwrap();
        assert!(r.contains(&format!("date:{}", tomo.format("%Y-%m-%d"))));
    }

    #[test]
    fn week_and_month_ranges() {
        // "last week" → after:/before: range (not just a date token)
        let r = parse_nl("last week").unwrap();
        assert!(r.contains("after:"), "expected after: in: {r}");
        assert!(r.contains("before:"), "expected before: in: {r}");

        let r = parse_nl("this month").unwrap();
        assert!(r.contains("after:"), "expected after: in: {r}");
        assert!(r.contains("before:"), "expected before: in: {r}");
    }

    #[test]
    fn app_extraction() {
        let r = parse_nl("on figma yesterday").unwrap();
        assert!(r.contains("app:figma"), "expected app:figma in: {r}");
        assert!(r.contains("date:"), "expected date: in: {r}");
    }

    #[test]
    fn noise_stripping() {
        // "what did I do yesterday" → just "date:..."
        let r = parse_nl("what did I do yesterday").unwrap();
        assert!(r.starts_with("date:"), "expected only date token, got: {r}");
        // Should not contain noise words
        assert!(!r.contains("what"));
        assert!(!r.contains("did"));
    }

    #[test]
    fn last_n_days() {
        let r = parse_nl("last 3 days").unwrap();
        assert!(r.contains("after:"), "expected after: in: {r}");
    }

    #[test]
    fn plain_term_fallback() {
        // "budget" is not a date or app → should become a plain term
        let r = parse_nl("budget").unwrap();
        assert_eq!(r, "budget");
    }
}
