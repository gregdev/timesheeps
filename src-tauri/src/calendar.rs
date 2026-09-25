// Microsoft 365 Calendar integration
// OAuth 2.0 PKCE flow + Microsoft Graph API calls

use crate::db;
use crate::models::{CalendarEvent, M365Status};
use anyhow::{Context, Result};
use base64::Engine;
use chrono::{DateTime, Utc};
use rand::Rng;
use reqwest::Client;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;
use tokio::sync::oneshot;
use url::Url;

// ── Constants ─────────────────────────────────────────────────────────────────

const SCOPES: &str = "Calendars.Read offline_access User.Read";
const MS_AUTHORIZE_URL: &str = "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize";
const MS_TOKEN_URL: &str = "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token";
const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

// ── PKCE helpers ──────────────────────────────────────────────────────────────

pub fn generate_pkce() -> (String, String) {
    let mut rng = rand::thread_rng();
    let verifier: String = (0..64)
        .map(|_| {
            let idx = rng.gen_range(0..62);
            if idx < 26 {
                (b'a' + idx) as char
            } else if idx < 52 {
                (b'A' + (idx - 26)) as char
            } else {
                (b'0' + (idx - 52)) as char
            }
        })
        .collect();

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);

    (verifier, challenge)
}

// ── OAuth types ───────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    #[allow(dead_code)]
    token_type: Option<String>,
}

/// Tokens returned from a successful OAuth exchange.
#[derive(Debug, Clone)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expiry: DateTime<Utc>,
}

/// Result from starting the OAuth login flow.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthUrl {
    pub url: String,
}

// ── OAuth URL builder ─────────────────────────────────────────────────────────

fn build_authorize_url(client_id: &str, redirect_uri: &str, code_challenge: &str) -> String {
    MS_AUTHORIZE_URL
        .replace("{tenant}", "common")
        + "?client_id="
        + &url::form_urlencoded::byte_serialize(client_id.as_bytes()).collect::<String>()
        + "&response_type=code"
        + "&redirect_uri="
        + &url::form_urlencoded::byte_serialize(redirect_uri.as_bytes()).collect::<String>()
        + "&response_mode=query"
        + "&scope="
        + &url::form_urlencoded::byte_serialize(SCOPES.as_bytes()).collect::<String>()
        + "&code_challenge="
        + code_challenge
        + "&code_challenge_method=S256"
}

// ── Token exchange ────────────────────────────────────────────────────────────

async fn exchange_code(
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
) -> Result<OAuthTokens> {
    let client = Client::new();
    let params = [
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("code_verifier", code_verifier),
    ];

    let resp: TokenResponse = client
        .post(MS_TOKEN_URL.replace("{tenant}", "common"))
        .form(&params)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .context("Failed to reach Microsoft token endpoint")?
        .json()
        .await
        .context("Failed to parse token response")?;

    let expiry = Utc::now() + chrono::Duration::seconds(resp.expires_in.unwrap_or(3600));

    Ok(OAuthTokens {
        access_token: resp.access_token,
        refresh_token: resp.refresh_token.unwrap_or_default(),
        expiry,
    })
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

    let expiry = Utc::now() + chrono::Duration::seconds(resp.expires_in.unwrap_or(3600));

    Ok(OAuthTokens {
        access_token: resp.access_token,
        refresh_token: resp.refresh_token.unwrap_or_else(|| refresh_token.to_string()),
        expiry,
    })
}

#[allow(dead_code)]
async fn get_valid_token(conn: &Connection) -> Result<String> {
    let client_id = db::get_setting_str(conn, "m365_client_id", "")?;
    let access_token = db::get_setting_str(conn, "m365_access_token", "")?;
    let refresh_token = db::get_setting_str(conn, "m365_refresh_token", "")?;
    let expiry_str = db::get_setting_str(conn, "m365_token_expiry", "")?;

    if client_id.is_empty() {
        anyhow::bail!("Microsoft 365 not configured: missing client_id");
    }

    // Check if access token is still valid (with 60s buffer)
    if !expiry_str.is_empty() && !access_token.is_empty() {
        if let Ok(expiry) = DateTime::parse_from_rfc3339(&expiry_str) {
            let expiry_utc = expiry.with_timezone(&Utc);
            if Utc::now() + chrono::Duration::seconds(60) < expiry_utc {
                return Ok(access_token);
            }
        }
    }

    if refresh_token.is_empty() {
        anyhow::bail!("Not authenticated — please connect Microsoft 365 in Settings");
    }

    let tokens = refresh_access_token(&client_id, &refresh_token).await?;

    db::set_setting_str(conn, "m365_access_token", &tokens.access_token)?;
    db::set_setting_str(conn, "m365_refresh_token", &tokens.refresh_token)?;
    db::set_setting_str(conn, "m365_token_expiry", &tokens.expiry.to_rfc3339())?;

    Ok(tokens.access_token)
}

// ── OAuth flow (start + callback) ─────────────────────────────────────────────

/// Start the OAuth 2.0 PKCE flow.
///
/// Returns the authorize URL (open in browser) and a oneshot receiver that
/// resolves when the callback completes with the tokens.
pub fn start_oauth_flow(
    client_id: &str,
) -> Result<(OAuthUrl, oneshot::Receiver<Result<OAuthTokens>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind local port")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://localhost:{}/callback", port);

    let (verifier, challenge) = generate_pkce();
    let auth_url = build_authorize_url(client_id, &redirect_uri, &challenge);

    let client_id_owned = client_id.to_string();
    let redirect_uri_owned = redirect_uri;

    let (tx, rx) = oneshot::channel();

    std::thread::spawn(move || {
        let result = handle_oauth_callback(listener, &client_id_owned, &redirect_uri_owned, &verifier);
        let _ = tx.send(result);
    });

    Ok((OAuthUrl { url: auth_url }, rx))
}

fn handle_oauth_callback(
    listener: TcpListener,
    client_id: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<OAuthTokens> {
    listener
        .set_nonblocking(false)
        .context("Failed to set blocking mode")?;

    let (mut stream, _addr) = listener
        .accept()
        .context("Timeout waiting for OAuth callback")?;

    stream.set_read_timeout(Some(Duration::from_secs(120))).ok();

    let mut reader = BufReader::new(
        stream.try_clone().context("Failed to clone stream")?,
    );

    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .context("Failed to read HTTP request")?;

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        send_html_response(&mut stream, false, "Invalid HTTP request");
        anyhow::bail!("Invalid HTTP request");
    }

    let full_url = format!("http://localhost{}", parts[1]);
    let parsed = Url::parse(&full_url).context("Failed to parse callback URL")?;

    // Check for OAuth error
    if let Some(error) = parsed
        .query_pairs()
        .find(|(k, _)| k == "error")
        .map(|(_, v)| v.to_string())
    {
        let desc = parsed
            .query_pairs()
            .find(|(k, _)| k == "error_description")
            .map(|(_, v)| v.to_string())
            .unwrap_or_default();
        send_html_response(&mut stream, false, &format!("Authorization denied: {} — {}", error, desc));
        anyhow::bail!("OAuth error: {} — {}", error, desc);
    }

    let code = parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .context("No authorization code in callback")?;

    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
    let result = rt.block_on(exchange_code(client_id, redirect_uri, &code, code_verifier));

    match &result {
        Ok(_) => send_html_response(&mut stream, true, "Successfully connected to Microsoft 365!"),
        Err(e) => send_html_response(&mut stream, false, &format!("Failed to exchange code: {}", e)),
    }

    result
}

fn send_html_response(stream: &mut std::net::TcpStream, success: bool, message: &str) {
    let color = if success { "#22c55e" } else { "#ef4444" };
    let title = if success { "Connected!" } else { "Error" };
    let body = format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>{title}</title>
<style>
  body {{ font-family: -apple-system, BlinkMacSystemFont, sans-serif;
         display: flex; align-items: center; justify-content: center;
         min-height: 100vh; margin: 0; background: #0f172a; color: #e2e8f0; }}
  .card {{ text-align: center; padding: 2rem; }}
  h1 {{ color: {color}; }}
  p {{ color: #94a3b8; }}
</style></head>
<body><div class="card"><h1>{title}</h1><p>{msg}</p><p>You can close this window.</p></div></body></html>"#,
        title = title,
        color = color,
        msg = message,
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body,
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
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
    subject: String,
    start: GraphDateTime,
    end: GraphDateTime,
    #[serde(rename = "isAllDay")]
    is_all_day: Option<bool>,
    organizer: Option<GraphOrganizer>,
    location: Option<GraphLocation>,
    #[serde(rename = "isOnlineMeeting")]
    is_online_meeting: Option<bool>,
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
    value: Vec<GraphEvent>,
}

/// Fetch calendar events for a given date range from Microsoft Graph.
#[allow(dead_code)]
pub async fn get_calendar_events(
    conn: &Connection,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<CalendarEvent>> {
    let has_tokens = db::get_setting_str(conn, "m365_refresh_token", "")
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    if !has_tokens {
        return Ok(Vec::new());
    }

    let token = get_valid_token(conn).await?;
    let client = Client::new();

    let url = format!(
        "{}/me/calendar/calendarView?startDateTime={}&endDateTime={}&\
         $select=subject,start,end,isAllDay,organizer,location,isOnlineMeeting&\
         $orderby=start/dateTime&$top=100",
        GRAPH_BASE_URL,
        url::form_urlencoded::byte_serialize(
            format!("{}T00:00:00", start_date).as_bytes()
        ).collect::<String>(),
        url::form_urlencoded::byte_serialize(
            format!("{}T00:00:00", end_date).as_bytes()
        ).collect::<String>(),
    );

    let resp: GraphCalendarResponse = client
        .get(&url)
        .bearer_auth(&token)
        .header("Prefer", r#"outlook.timezone="UTC""#)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .context("Failed to fetch calendar events")?
        .json()
        .await
        .context("Failed to parse calendar response")?;

    let events: Vec<CalendarEvent> = resp
        .value
        .into_iter()
        .filter_map(|evt| {
            let start_dt = parse_graph_datetime(&evt.start.date_time)?;
            let end_dt = parse_graph_datetime(&evt.end.date_time)?;
            Some(CalendarEvent {
                subject: evt.subject,
                start_at: start_dt,
                end_at: end_dt,
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
            })
        })
        .collect();

    Ok(events)
}

/// Fetch calendar events using pre-extracted token parts (no DB reference needed
/// across await points, so this future is `Send`).
pub async fn get_calendar_events_from_parts(
    client_id: &str,
    access_token: &str,
    refresh_token: &str,
    expiry_str: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<CalendarEvent>> {
    let token = get_valid_token_from_parts(client_id, access_token, refresh_token, expiry_str).await?;
    fetch_calendar_events_with_token(&token, start_date, end_date).await
}

/// Resolve a valid access token from pre-extracted parts.
async fn get_valid_token_from_parts(
    client_id: &str,
    access_token: &str,
    refresh_token: &str,
    expiry_str: &str,
) -> Result<String> {
    if client_id.is_empty() {
        anyhow::bail!("Microsoft 365 not configured: missing client_id");
    }

    // Check if access token is still valid (with 60s buffer)
    if !expiry_str.is_empty() && !access_token.is_empty() {
        if let Ok(expiry) = DateTime::parse_from_rfc3339(expiry_str) {
            let expiry_utc = expiry.with_timezone(&Utc);
            if Utc::now() + chrono::Duration::seconds(60) < expiry_utc {
                return Ok(access_token.to_string());
            }
        }
    }

    if refresh_token.is_empty() {
        anyhow::bail!("Not authenticated — please connect Microsoft 365 in Settings");
    }

    let tokens = refresh_access_token(client_id, refresh_token).await?;

    // Note: we can't persist the refreshed token here without a DB connection.
    // The refreshed token will work for this call; next call will refresh again
    // or use the old refresh_token. This is acceptable for a read operation.

    Ok(tokens.access_token)
}

/// Fetch calendar events using an already-resolved access token.
async fn fetch_calendar_events_with_token(
    token: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<CalendarEvent>> {
    let client = Client::new();

    let url = format!(
        "{}/me/calendar/calendarView?startDateTime={}&endDateTime={}&\
         $select=subject,start,end,isAllDay,organizer,location,isOnlineMeeting&\
         $orderby=start/dateTime&$top=100",
        GRAPH_BASE_URL,
        url::form_urlencoded::byte_serialize(
            format!("{}T00:00:00", start_date).as_bytes()
        ).collect::<String>(),
        url::form_urlencoded::byte_serialize(
            format!("{}T00:00:00", end_date).as_bytes()
        ).collect::<String>(),
    );

    let resp: GraphCalendarResponse = client
        .get(&url)
        .bearer_auth(token)
        .header("Prefer", r#"outlook.timezone="UTC""#)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .context("Failed to fetch calendar events")?
        .json()
        .await
        .context("Failed to parse calendar response")?;

    let events: Vec<CalendarEvent> = resp
        .value
        .into_iter()
        .filter_map(|evt| {
            let start_dt = parse_graph_datetime(&evt.start.date_time)?;
            let end_dt = parse_graph_datetime(&evt.end.date_time)?;
            Some(CalendarEvent {
                subject: evt.subject,
                start_at: start_dt,
                end_at: end_dt,
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
            })
        })
        .collect();

    Ok(events)
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
