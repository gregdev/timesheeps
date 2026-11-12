// Microsoft 365 Calendar integration
//
// Sign-in uses the **device code** flow rather than an authorization-code flow
// with a localhost redirect.
//
// That is a deliberate reversal. The old flow needed the user to register an
// Azure AD application, configure a `http://localhost` redirect URI, and paste
// the client id into Settings — three things to get wrong before seeing a single
// meeting, and the localhost callback does not work at all on machines where an
// endpoint agent blocks loopback listeners.
//
// Device code needs none of that: the app shows a short code, the user types it
// at microsoft.com/devicelogin on any device, and the app polls until the tenant
// approves. The only app-registration setting that matters is "allow public
// client flows", and the client id is a public identifier, not a secret, so it
// can ship with the binary.

use crate::db;
use crate::models::{CalendarEvent, DeviceCodePrompt, M365Status};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use rusqlite::Connection;
use std::time::Duration;

// ── Constants ─────────────────────────────────────────────────────────────────

const SCOPES: &str = "Calendars.Read offline_access User.Read";
const MS_DEVICECODE_URL: &str = "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/devicecode";
const MS_TOKEN_URL: &str = "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token";
const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

/// Overrides the built-in client id, for a tenant that insists on its own app
/// registration. Public client ids are not secrets, so shipping a default is
/// normal — leaving this empty just means the user has to paste one once.
pub const CLIENT_ID_OVERRIDE_ENV: &str = "TIMESHEEPS_M365_CLIENT_ID";

// ── OAuth types ───────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    #[allow(dead_code)]
    token_type: Option<String>,
    /// Device-code polling reports progress through this field rather than an
    /// HTTP status: `authorization_pending`, `slow_down`, `expired_token`,
    /// `authorization_declined`.
    error: Option<String>,
    error_description: Option<String>,
}

/// Tokens returned from a successful OAuth exchange.
#[derive(Debug, Clone)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expiry: DateTime<Utc>,
}

/// Start a device-code sign-in. The prompt is shown to the user; `device_code`
/// is the long-lived secret that `/token` is polled with.
pub async fn start_device_code(client_id: &str) -> Result<(String, DeviceCodePrompt)> {
    let params = [("client_id", client_id), ("scope", SCOPES)];

    let body: serde_json::Value = Client::new()
        .post(MS_DEVICECODE_URL.replace("{tenant}", "common"))
        .form(&params)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .context("Failed to reach Microsoft sign-in")?
        .json()
        .await
        .context("Failed to parse device code response")?;

    if let Some(err) = body["error"].as_str() {
        let desc = body["error_description"].as_str().unwrap_or_default();
        anyhow::bail!("Microsoft refused to start sign-in: {err} — {desc}");
    }

    let device_code = body["device_code"]
        .as_str()
        .context("Microsoft did not return a device code")?
        .to_string();
    let user_code = body["user_code"]
        .as_str()
        .context("Microsoft did not return a user code")?
        .to_string();
    let verification_uri = body["verification_uri"]
        .as_str()
        .unwrap_or("https://microsoft.com/devicelogin")
        .to_string();
    let expires_in = body["expires_in"].as_i64().unwrap_or(900);
    let interval = body["interval"].as_i64().unwrap_or(5).max(1);

    // The interval is carried in-band so the poller knows how often to ask.
    let prompt = DeviceCodePrompt {
        user_code,
        verification_uri,
        expires_in,
    };

    Ok((format!("{device_code}\u{1}{interval}"), prompt))
}

/// Poll `/token` until the user finishes signing in, then return the tokens.
///
/// Every failure mode is reported explicitly: the frontend has no way to guess
/// why nothing happened if this returns a bare error.
pub async fn poll_device_code(client_id: &str, device_code: &str) -> Result<OAuthTokens> {
    let (code, interval) = device_code
        .split_once('\u{1}')
        .map(|(c, i)| (c.to_string(), i.parse::<i64>().unwrap_or(5).max(1)))
        .unwrap_or_else(|| (device_code.to_string(), 5));

    let client = Client::new();
    // A code lives for 15 minutes; the extra minute absorbs network slop.
    let deadline = Utc::now() + chrono::Duration::seconds(16 * 60);
    let mut wait = interval;

    loop {
        if Utc::now() > deadline {
            anyhow::bail!("The sign-in code expired before it was used. Try connecting again.");
        }

        tokio::time::sleep(Duration::from_secs(wait as u64)).await;

        let params = [
            ("client_id", client_id),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", code.as_str()),
        ];

        let resp: TokenResponse = client
            .post(MS_TOKEN_URL.replace("{tenant}", "common"))
            .form(&params)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .context("Failed to reach Microsoft token endpoint")?
            .json()
            .await
            .context("Failed to parse the token response")?;

        if let Some(access_token) = resp.access_token {
            return Ok(OAuthTokens {
                access_token,
                refresh_token: resp.refresh_token.unwrap_or_default(),
                expiry: Utc::now() + chrono::Duration::seconds(resp.expires_in.unwrap_or(3600)),
            });
        }

        match resp.error.as_deref() {
            // Still waiting — by far the common case.
            Some("authorization_pending") => {}
            // Microsoft asked us to back off: it tolerates one early poll, not
            // a loop that ignores it.
            Some("slow_down") => wait += 5,
            Some("expired_token") => {
                anyhow::bail!("The sign-in code expired before it was used. Try connecting again.");
            }
            Some("authorization_declined") => {
                anyhow::bail!("Sign-in was declined.");
            }
            Some(other) => {
                let desc = resp.error_description.unwrap_or_default();
                anyhow::bail!("Microsoft sign-in failed: {other} — {desc}");
            }
            // No token and no error should not happen; treat it as transient but
            // do not spin at full speed.
            None => wait = (wait + 1).max(interval),
        }
    }
}

async fn refresh_access_token(client_id: &str, refresh_token: &str) -> Result<OAuthTokens> {
    let client = Client::new();
    let params = [
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];

    let resp: TokenResponse = client
        .post(MS_TOKEN_URL.replace("{tenant}", "common"))
        .form(&params)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .context("Failed to refresh token")?
        .json()
        .await
        .context("Failed to parse refresh token response")?;

    let access_token = resp
        .access_token
        .context("Microsoft returned no access token when refreshing")?;
    let expiry = Utc::now() + chrono::Duration::seconds(resp.expires_in.unwrap_or(3600));

    Ok(OAuthTokens {
        access_token,
        refresh_token: resp.refresh_token.unwrap_or_else(|| refresh_token.to_string()),
        expiry,
    })
}

/// Store OAuth tokens in the database and return the account display name.
pub fn store_tokens(conn: &Connection, tokens: &OAuthTokens) -> Result<String> {
    let rt = tokio::runtime::Runtime::new()?;
    let account_name = rt.block_on(async {
        let client = Client::new();
        let resp: serde_json::Value = client
            .get(format!("{}/me", GRAPH_BASE_URL))
            .bearer_auth(&tokens.access_token)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .context("Failed to get user info")?
            .json()
            .await
            .context("Failed to parse user info")?;

        Ok::<String, anyhow::Error>(
            resp["displayName"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string(),
        )
    })?;

    db::set_setting_str(conn, "m365_access_token", &tokens.access_token)?;
    db::set_setting_str(conn, "m365_refresh_token", &tokens.refresh_token)?;
    db::set_setting_str(conn, "m365_token_expiry", &tokens.expiry.to_rfc3339())?;
    db::set_setting_str(conn, "m365_account_name", &account_name)?;

    Ok(account_name)
}

// ── Calendar events ───────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct GraphEvent {
    /// Stable per *series* (every occurrence of a recurring meeting shares it),
    /// so this is the key a remembered project or a dismissal hangs off.
    /// `id` is per-occurrence and therefore useless for that purpose.
    #[serde(rename = "iCalUId", default)]
    i_cal_uid: String,
    #[serde(default)]
    subject: String,
    start: Option<GraphDateTime>,
    end: Option<GraphDateTime>,
    #[serde(rename = "isAllDay", default)]
    is_all_day: Option<bool>,
    #[serde(rename = "isCancelled", default)]
    is_cancelled: Option<bool>,
    organizer: Option<GraphOrganizer>,
    location: Option<GraphLocation>,
    #[serde(rename = "isOnlineMeeting", default)]
    is_online_meeting: Option<bool>,
    #[serde(rename = "responseStatus", default)]
    response_status: Option<GraphResponseStatus>,
}

#[derive(Debug, serde::Deserialize)]
struct GraphDateTime {
    #[serde(rename = "dateTime")]
    date_time: String,
    #[serde(rename = "timeZone")]
    #[allow(dead_code)]
    time_zone: String,
}

#[derive(Debug, serde::Deserialize)]
struct GraphResponseStatus {
    #[serde(default)]
    response: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GraphOrganizer {
    #[serde(rename = "emailAddress")]
    email_address: Option<GraphEmailAddress>,
}

#[derive(Debug, serde::Deserialize)]
struct GraphEmailAddress {
    name: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GraphLocation {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GraphCalendarResponse {
    #[serde(default)]
    value: Vec<GraphEvent>,
    /// Present when the window holds more events than one page. Ignoring it
    /// silently truncated a busy fortnight.
    #[serde(rename = "@odata.nextLink", default)]
    next_link: Option<String>,
}

/// Everything `fetch_events` needs, read out of the database in one go so the
/// `await` points never hold a connection (and the future stays `Send`).
pub struct CalendarPrereq {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expiry_str: String,
}

impl CalendarPrereq {
    /// `None` when the calendar is not connected.
    pub fn load(conn: &Connection) -> Result<Option<Self>> {
        let refresh_token = db::get_setting_str(conn, "m365_refresh_token", "")?;
        if refresh_token.is_empty() {
            return Ok(None);
        }

        Ok(Some(CalendarPrereq {
            client_id: db::get_setting_str(conn, "m365_client_id", "")?,
            access_token: db::get_setting_str(conn, "m365_access_token", "")?,
            refresh_token,
            expiry_str: db::get_setting_str(conn, "m365_token_expiry", "")?,
        }))
    }
}

/// A calendar fetch, plus any refreshed tokens the caller must persist.
///
/// Persisting matters: Microsoft rotates refresh tokens, and a rotated token
/// that is never written back leaves the stored one to be invalidated. The old
/// code refreshed on every call and deliberately dropped the result, which works
/// until the rotation invalidates the stored copy and the user is silently
/// logged out.
pub struct CalendarFetch {
    pub events: Vec<CalendarEvent>,
    pub refreshed: Option<OAuthTokens>,
}

fn local_date_of(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string()
}

fn local_midnight_utc(date: &chrono::NaiveDate) -> DateTime<Utc> {
    date.and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

/// Fetch every event overlapping the local days `[start_date, end_date]`.
///
/// The window is converted to explicit UTC instants. Sending a bare
/// `2026-11-12T00:00:00` while asking Graph to answer in UTC made it read as
/// *UTC* midnight — eleven hours adrift for anyone in UTC+11, which silently
/// dropped the morning's meetings from the day the user was looking at.
pub async fn fetch_events(
    pre: &CalendarPrereq,
    start_date: &str,
    end_date: &str,
) -> Result<CalendarFetch> {
    let (token, refreshed) = resolve_token(pre).await?;

    let from = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
        .context("start_date must be YYYY-MM-DD")?;
    let to = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
        .context("end_date must be YYYY-MM-DD")?;

    let start_utc = local_midnight_utc(&from);
    // The Graph `calendarView` window is half-open, so the last included day is
    // reached by asking for midnight *after* it.
    let end_utc = local_midnight_utc(&(to + chrono::Duration::days(1)));

    let mut url = format!(
        "{}/me/calendar/calendarView?startDateTime={}&endDateTime={}&\
         $select=iCalUId,subject,start,end,isAllDay,isCancelled,organizer,location,\
         isOnlineMeeting,responseStatus&$orderby=start/dateTime&$top=200",
        GRAPH_BASE_URL,
        url::form_urlencoded::byte_serialize(start_utc.to_rfc3339().as_bytes())
            .collect::<String>(),
        url::form_urlencoded::byte_serialize(end_utc.to_rfc3339().as_bytes()).collect::<String>(),
    );

    let client = Client::new();
    let mut events: Vec<CalendarEvent> = Vec::new();
    // Five pages is 1000 events for a month; anything beyond that is a broken
    // calendar, not a working day.
    for _ in 0..5 {
        let resp: GraphCalendarResponse = client
            .get(&url)
            .bearer_auth(&token)
            .header("Prefer", r#"outlook.timezone="UTC""#)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .context("Failed to fetch calendar events")?
            .json()
            .await
            .context("Failed to parse calendar response")?;

        events.extend(resp.value.into_iter().filter_map(to_calendar_event));

        match resp.next_link {
            Some(next) => url = next,
            None => break,
        }
    }

    Ok(CalendarFetch { events, refreshed })
}

fn to_calendar_event(evt: GraphEvent) -> Option<CalendarEvent> {
    // A cancelled occurrence carries no useful span and must never be offered
    // as a candidate.
    if evt.is_cancelled.unwrap_or(false) {
        return None;
    }

    let start_at = parse_graph_datetime(&evt.start?.date_time)?;
    let end_at = parse_graph_datetime(&evt.end?.date_time)?;
    if end_at <= start_at {
        return None;
    }

    Some(CalendarEvent {
        i_cal_uid: evt.i_cal_uid,
        subject: evt.subject,
        start_at,
        end_at,
        is_all_day: evt.is_all_day.unwrap_or(false),
        organizer: evt
            .organizer
            .and_then(|o| o.email_address)
            .and_then(|e| e.name)
            .unwrap_or_default(),
        location: evt
            .location
            .and_then(|l| l.display_name)
            .unwrap_or_default(),
        is_teams_meeting: evt.is_online_meeting.unwrap_or(false),
        response: evt
            .response_status
            .and_then(|r| r.response)
            .unwrap_or_default(),
    })
}

/// Resolve a usable access token, returning any rotated tokens for the caller to
/// persist.
async fn resolve_token(pre: &CalendarPrereq) -> Result<(String, Option<OAuthTokens>)> {
    if pre.client_id.is_empty() {
        anyhow::bail!("Microsoft 365 not configured: missing client_id");
    }

    // 60s of slack, so a token cannot expire between this check and the request.
    if !pre.expiry_str.is_empty() && !pre.access_token.is_empty() {
        if let Ok(expiry) = DateTime::parse_from_rfc3339(&pre.expiry_str) {
            if Utc::now() + chrono::Duration::seconds(60) < expiry.with_timezone(&Utc) {
                return Ok((pre.access_token.clone(), None));
            }
        }
    }

    let tokens = refresh_access_token(&pre.client_id, &pre.refresh_token).await?;
    Ok((tokens.access_token.clone(), Some(tokens)))
}

/// Write fetched events into the local cache, keyed by local date.
pub fn store_events(
    conn: &Connection,
    events: &[CalendarEvent],
    start_date: &str,
    end_date: &str,
) -> Result<()> {
    let rows: Vec<db::StoredCalendarEvent> = events
        .iter()
        .map(|e| db::StoredCalendarEvent {
            i_cal_uid: if e.i_cal_uid.is_empty() {
                // Fall back to a content key so legacy/synthetic sources still
                // have a stable series identity to hang a project on.
                format!("{}|{}", e.subject, e.start_at.date_naive())
            } else {
                e.i_cal_uid.clone()
            },
            started_at: e.start_at.to_rfc3339(),
            ended_at: e.end_at.to_rfc3339(),
            event_date: local_date_of(e.start_at),
            subject: e.subject.clone(),
            is_all_day: e.is_all_day,
            organizer: e.organizer.clone(),
            location: e.location.clone(),
            is_teams: e.is_teams_meeting,
            response: e.response.clone(),
        })
        .collect();

    db::replace_calendar_events(conn, start_date, end_date, &rows)
}

pub fn persist_refreshed(conn: &Connection, tokens: &OAuthTokens) -> Result<()> {
    db::set_setting_str(conn, "m365_access_token", &tokens.access_token)?;
    db::set_setting_str(conn, "m365_refresh_token", &tokens.refresh_token)?;
    db::set_setting_str(conn, "m365_token_expiry", &tokens.expiry.to_rfc3339())?;
    Ok(())
}

/// Days either side of today kept in the cache. A fortnight back covers the
/// timesheet the user is actually filling in; a fortnight forward means a
/// meeting booked this morning is already on the timeline.
pub const SYNC_BACK_DAYS: i64 = 14;
pub const SYNC_FORWARD_DAYS: i64 = 14;

/// A completed sync: the events, the window they cover, and any rotated tokens
/// the caller must persist.
pub struct SyncWindow {
    pub events: Vec<CalendarEvent>,
    pub refreshed: Option<OAuthTokens>,
    pub start_date: String,
    pub end_date: String,
}

/// Refresh the cache around today. Split out from the scheduler loop so the
/// "Refresh now" button and the background tick share one code path.
pub async fn sync_around_today(pre: &CalendarPrereq) -> Result<SyncWindow> {
    let today = chrono::Local::now().date_naive();
    let start_date = (today - chrono::Duration::days(SYNC_BACK_DAYS))
        .format("%Y-%m-%d")
        .to_string();
    let end_date = (today + chrono::Duration::days(SYNC_FORWARD_DAYS))
        .format("%Y-%m-%d")
        .to_string();

    let fetch = fetch_events(pre, &start_date, &end_date).await?;

    Ok(SyncWindow {
        events: fetch.events,
        refreshed: fetch.refreshed,
        start_date,
        end_date,
    })
}

fn parse_graph_datetime(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    let cleaned = s.trim_end_matches('Z');
    let with_tz = format!("{}Z", cleaned);
    if let Ok(dt) = DateTime::parse_from_rfc3339(&with_tz) {
        return Some(dt.with_timezone(&Utc));
    }
    None
}

// ── Connection status ─────────────────────────────────────────────────────────

/// Get the current Microsoft 365 connection status.
pub fn get_status(conn: &Connection) -> Result<M365Status> {
    let account_name = db::get_setting_str(conn, "m365_account_name", "")?;
    let has_refresh = db::get_setting_str(conn, "m365_refresh_token", "")
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    Ok(M365Status {
        connected: has_refresh,
        account_name: if has_refresh { account_name } else { String::new() },
    })
}

/// Disconnect (clear all stored tokens).
pub fn disconnect(conn: &Connection) -> Result<()> {
    db::set_setting_str(conn, "m365_access_token", "")?;
    db::set_setting_str(conn, "m365_refresh_token", "")?;
    db::set_setting_str(conn, "m365_token_expiry", "")?;
    db::set_setting_str(conn, "m365_account_name", "")?;
    Ok(())
}

// ── Background sync ───────────────────────────────────────────────────────────

/// How often the cache is refreshed. Meetings are booked and moved a few times a
/// day; a quarter of an hour is far more often than that changes, and the request
/// is a single Graph page.
const SYNC_TICK_SECS: u64 = 15 * 60;

/// Keep the local calendar cache warm.
///
/// This exists so the review view and the MCP can read meetings synchronously
/// from SQLite. Fetching per day, on view, meant the numbers depended on which
/// page the user happened to open — and a meeting was invisible to both the
/// period review and Claude.
///
/// The first pass runs immediately: waiting a full tick would leave a freshly
/// launched app with an empty calendar for a quarter of an hour, and this app is
/// usually launched once and left running for weeks.
pub fn start_sync(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        // A short delay lets the window finish coming up before any network work.
        tokio::time::sleep(Duration::from_secs(10)).await;

        loop {
            if let Err(e) = sync_once(&app).await {
                // Not fatal: an expired token or a sleeping laptop should not
                // spam anything. The next tick retries.
                eprintln!("[timesheeps] calendar sync failed: {e}");
            }
            tokio::time::sleep(Duration::from_secs(SYNC_TICK_SECS)).await;
        }
    });
}

/// One sync pass. Public so the "Refresh now" command shares the exact same path.
pub async fn sync_once(app: &tauri::AppHandle) -> Result<usize> {
    use tauri::Manager;

    let prereq = {
        let state = app.state::<crate::AppState>();
        let conn = state
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("database lock poisoned: {e}"))?;
        CalendarPrereq::load(&conn)?
    };

    let Some(prereq) = prereq else {
        // Not connected: nothing to do, and no error to report.
        return Ok(0);
    };

    // Network first, with no database lock held.
    let window = sync_around_today(&prereq).await?;
    let count = window.events.len();

    {
        let state = app.state::<crate::AppState>();
        let conn = state
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("database lock poisoned: {e}"))?;
        store_events(&conn, &window.events, &window.start_date, &window.end_date)?;
        if let Some(tokens) = &window.refreshed {
            persist_refreshed(&conn, tokens)?;
        }
    }

    Ok(count)
}
