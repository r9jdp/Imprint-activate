use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

const CALLBACK_PORT: u16 = 8765;
const CALLBACK_PATH: &str = "/oauth2/callback";

const SCOPES: &[&str] = &[
    "openid",
    "email",
    "profile",
    "https://www.googleapis.com/auth/gmail.readonly",
    "https://www.googleapis.com/auth/classroom.courses.readonly",
    "https://www.googleapis.com/auth/classroom.coursework.me.readonly",
    "https://www.googleapis.com/auth/calendar.readonly",
    "https://www.googleapis.com/auth/drive.file",
    "https://www.googleapis.com/auth/documents",
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleOauthConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleTokenStore {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    pub scope: String,
    pub token_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleProfile {
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GmailMessage {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub snippet: String,
    pub internal_date: Option<String>,
    pub web_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClassroomAssignment {
    pub id: String,
    pub course_id: String,
    pub course_name: String,
    pub course_link: Option<String>,
    pub title: String,
    pub state: String,
    pub due_date: Option<String>,
    pub due_time: Option<String>,
    pub alternate_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarEvent {
    pub id: String,
    pub summary: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub html_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceSnapshot {
    pub profile: Option<GoogleProfile>,
    pub gmail: Vec<GmailMessage>,
    pub classroom: Vec<ClassroomAssignment>,
    pub calendar: Vec<CalendarEvent>,
}

fn storage_dir() -> Result<PathBuf, String> {
    let dir = dirs::home_dir()
        .ok_or_else(|| "Could not resolve home directory".to_string())?
        .join(".imprint")
        .join("student-agent");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn config_path() -> Result<PathBuf, String> {
    Ok(storage_dir()?.join("google_tokens.json"))
}

fn token_path() -> Result<PathBuf, String> {
    config_path()
}

fn read_runtime_env(name: &str) -> Option<String> {
    if let Ok(value) = std::env::var(name) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    for path in [".env", "../.env"] {
        if let Ok(contents) = std::fs::read_to_string(path) {
            for line in contents.lines() {
                let line = line.trim();
                if line.starts_with('#') || !line.contains('=') {
                    continue;
                }
                if let Some(rest) = line.strip_prefix(&format!("{}=", name)) {
                    let value = rest.trim().trim_matches('"').trim_matches('\'');
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn load_config() -> Result<GoogleOauthConfig, String> {
    Ok(GoogleOauthConfig {
        client_id: read_runtime_env("GOOGLE_CLIENT_ID")
            .or_else(|| read_runtime_env("GOOGLE_OAUTH_CLIENT_ID"))
            .unwrap_or_default(),
        client_secret: read_runtime_env("GOOGLE_CLIENT_SECRET")
            .or_else(|| read_runtime_env("GOOGLE_OAUTH_CLIENT_SECRET"))
            .unwrap_or_default(),
    })
}

fn load_tokens() -> Result<GoogleTokenStore, String> {
    let path = token_path()?;
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_tokens(tokens: &GoogleTokenStore) -> Result<(), String> {
    let path = token_path()?;
    let raw = serde_json::to_string_pretty(tokens).map_err(|e| e.to_string())?;
    std::fs::write(path, raw).map_err(|e| e.to_string())
}

pub fn clear_tokens() -> Result<(), String> {
    let path = token_path()?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn is_connected() -> bool {
    token_path().map(|p| p.exists()).unwrap_or(false)
}

fn now_unix() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs())
}

fn build_pkce() -> (String, String, String) {
    let state = uuid::Uuid::new_v4().to_string();
    let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());
    (state, verifier, challenge)
}

fn auth_url(config: &GoogleOauthConfig, state: &str, challenge: &str) -> String {
    let redirect_uri = format!("http://127.0.0.1:{}{}", CALLBACK_PORT, CALLBACK_PATH);
    let scope = SCOPES.join(" ");
    Url::parse_with_params(
        "https://accounts.google.com/o/oauth2/v2/auth",
        &[
            ("client_id", config.client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", scope.as_str()),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
        ],
    )
    .expect("oauth url")
    .to_string()
}

fn wait_for_code(expected_state: &str) -> Result<String, String> {
    let listener = TcpListener::bind(("127.0.0.1", CALLBACK_PORT))
        .map_err(|e| format!("Could not bind local OAuth callback port {}: {}", CALLBACK_PORT, e))?;

    let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
    let mut buffer = [0_u8; 4096];
    let size = stream.read(&mut buffer).map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buffer[..size]).to_string();
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| "Invalid OAuth callback request".to_string())?;

    let path = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "Missing OAuth callback path".to_string())?;

    let callback_url = format!("http://127.0.0.1:{}{}", CALLBACK_PORT, path);
    let parsed = Url::parse(&callback_url).map_err(|e| e.to_string())?;
    let params: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();

    let response = if let Some(err) = params.get("error") {
        format!("OAuth failed: {}", err)
    } else if params.get("state").map(String::as_str) != Some(expected_state) {
        "OAuth state mismatch".to_string()
    } else {
        "Google account connected. You can close this window.".to_string()
    };

    let body = format!(
        "<html><body style=\"font-family: sans-serif; padding: 32px;\"><h2>{}</h2></body></html>",
        response
    );
    let http = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(http.as_bytes()).map_err(|e| e.to_string())?;

    if let Some(err) = params.get("error") {
        return Err(format!("Google OAuth failed: {}", err));
    }
    if params.get("state").map(String::as_str) != Some(expected_state) {
        return Err("OAuth state mismatch".to_string());
    }
    params
        .get("code")
        .cloned()
        .ok_or_else(|| "Missing OAuth authorization code".to_string())
}

async fn exchange_code(
    client: &Client,
    config: &GoogleOauthConfig,
    code: &str,
    verifier: &str,
) -> Result<GoogleTokenStore, String> {
    let redirect_uri = format!("http://127.0.0.1:{}{}", CALLBACK_PORT, CALLBACK_PATH);
    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("code", code),
            ("code_verifier", verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("Token exchange failed: {}", e))?;

    let status = response.status();
    let payload: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Token exchange failed ({}): {}", status, payload));
    }

    let expires_in = payload["expires_in"].as_u64().unwrap_or(3600);
    Ok(GoogleTokenStore {
        access_token: payload["access_token"].as_str().unwrap_or_default().to_string(),
        refresh_token: payload["refresh_token"].as_str().unwrap_or_default().to_string(),
        expires_at: now_unix()? + expires_in.saturating_sub(30),
        scope: payload["scope"].as_str().unwrap_or_default().to_string(),
        token_type: payload["token_type"].as_str().unwrap_or("Bearer").to_string(),
    })
}

async fn refresh_tokens(client: &Client, config: &GoogleOauthConfig, tokens: &GoogleTokenStore) -> Result<GoogleTokenStore, String> {
    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("refresh_token", tokens.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| format!("Token refresh failed: {}", e))?;

    let status = response.status();
    let payload: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Token refresh failed ({}): {}", status, payload));
    }

    let expires_in = payload["expires_in"].as_u64().unwrap_or(3600);
    Ok(GoogleTokenStore {
        access_token: payload["access_token"].as_str().unwrap_or_default().to_string(),
        refresh_token: tokens.refresh_token.clone(),
        expires_at: now_unix()? + expires_in.saturating_sub(30),
        scope: payload["scope"]
            .as_str()
            .unwrap_or(tokens.scope.as_str())
            .to_string(),
        token_type: payload["token_type"]
            .as_str()
            .unwrap_or(tokens.token_type.as_str())
            .to_string(),
    })
}

async fn access_token(client: &Client) -> Result<String, String> {
    let config = load_config()?;
    if config.client_id.trim().is_empty() || config.client_secret.trim().is_empty() {
        return Err("Google OAuth is not configured. Add client ID and client secret first.".to_string());
    }

    let tokens = load_tokens()?;
    let now = now_unix()?;
    let final_tokens = if tokens.expires_at <= now {
        let refreshed = refresh_tokens(client, &config, &tokens).await?;
        save_tokens(&refreshed)?;
        refreshed
    } else {
        tokens
    };
    Ok(final_tokens.access_token)
}

pub async fn sign_in() -> Result<GoogleProfile, String> {
    let config = load_config()?;
    if config.client_id.trim().is_empty() || config.client_secret.trim().is_empty() {
        return Err("Google OAuth is not configured. Add client ID and client secret first.".to_string());
    }

    let client = Client::new();
    let (state, verifier, challenge) = build_pkce();
    let url = auth_url(&config, &state, &challenge);
    crate::tools::open_external_url(&url)?;
    let code = wait_for_code(&state)?;
    let tokens = exchange_code(&client, &config, &code, &verifier).await?;
    save_tokens(&tokens)?;
    fetch_profile(&client, &tokens.access_token).await
}

pub async fn fetch_profile(client: &Client, access_token: &str) -> Result<GoogleProfile, String> {
    let response = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Failed to load Google profile: {}", e))?;

    let status = response.status();
    let payload: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Failed to load Google profile ({}): {}", status, payload));
    }

    Ok(GoogleProfile {
        email: payload["email"].as_str().unwrap_or_default().to_string(),
        name: payload["name"].as_str().unwrap_or_default().to_string(),
        picture: payload["picture"].as_str().map(str::to_string),
    })
}

pub async fn snapshot() -> Result<WorkspaceSnapshot, String> {
    let client = Client::new();
    let access = access_token(&client).await?;
    let profile = fetch_profile(&client, &access).await.ok();
    let gmail = fetch_gmail(&client, &access).await?;
    let classroom = fetch_classroom(&client, &access).await?;
    let calendar = fetch_calendar(&client, &access).await?;
    Ok(WorkspaceSnapshot {
        profile,
        gmail,
        classroom,
        calendar,
    })
}

async fn fetch_gmail(client: &Client, access_token: &str) -> Result<Vec<GmailMessage>, String> {
    let response = client
        .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
        .bearer_auth(access_token)
        .query(&[("labelIds", "INBOX"), ("maxResults", "30")])
        .send()
        .await
        .map_err(|e| format!("Failed to load Gmail messages: {}", e))?;
    let status = response.status();
    let payload: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Failed to load Gmail messages ({}): {}", status, payload));
    }

    let ids = payload["messages"].as_array().cloned().unwrap_or_default();
    let mut results = Vec::new();
    for message in ids.into_iter().take(30) {
        let Some(id) = message["id"].as_str() else { continue };
        let detail = client
            .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{}", id))
            .bearer_auth(access_token)
            .query(&[
                ("format", "metadata"),
                ("metadataHeaders", "From"),
                ("metadataHeaders", "Subject"),
                ("metadataHeaders", "Date"),
            ])
            .send()
            .await
            .map_err(|e| format!("Failed to load Gmail message detail: {}", e))?;
        let detail_status = detail.status();
        let detail_payload: Value = detail.json().await.map_err(|e| e.to_string())?;
        if !detail_status.is_success() {
            continue;
        }
        let headers = detail_payload["payload"]["headers"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let header_value = |name: &str| -> String {
            headers
                .iter()
                .find(|h| h["name"].as_str() == Some(name))
                .and_then(|h| h["value"].as_str())
                .unwrap_or_default()
                .to_string()
        };
        results.push(GmailMessage {
            id: id.to_string(),
            from: header_value("From"),
            subject: header_value("Subject"),
            snippet: detail_payload["snippet"].as_str().unwrap_or_default().to_string(),
            internal_date: detail_payload["internalDate"].as_str().map(str::to_string),
            web_link: Some(format!("https://mail.google.com/mail/u/0/#inbox/{}", id)),
        });
    }
    Ok(results)
}

async fn fetch_classroom(client: &Client, access_token: &str) -> Result<Vec<ClassroomAssignment>, String> {
    let response = client
        .get("https://classroom.googleapis.com/v1/courses")
        .bearer_auth(access_token)
        .query(&[("courseStates", "ACTIVE"), ("pageSize", "6")])
        .send()
        .await
        .map_err(|e| format!("Failed to load Classroom courses: {}", e))?;
    let status = response.status();
    let payload: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Failed to load Classroom courses ({}): {}", status, payload));
    }

    let courses = payload["courses"].as_array().cloned().unwrap_or_default();
    let mut assignments = Vec::new();
    for course in courses.into_iter().take(4) {
        let Some(course_id) = course["id"].as_str() else { continue };
        let course_name = course["name"].as_str().unwrap_or("Untitled course").to_string();
        let detail = client
            .get(format!(
                "https://classroom.googleapis.com/v1/courses/{}/courseWork",
                course_id
            ))
            .bearer_auth(access_token)
            .query(&[("pageSize", "5"), ("orderBy", "dueDate desc")])
            .send()
            .await
            .map_err(|e| format!("Failed to load Classroom coursework: {}", e))?;
        let detail_status = detail.status();
        let detail_payload: Value = detail.json().await.map_err(|e| e.to_string())?;
        if !detail_status.is_success() {
            continue;
        }

        for item in detail_payload["courseWork"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .take(3)
        {
            assignments.push(ClassroomAssignment {
                id: item["id"].as_str().unwrap_or_default().to_string(),
                course_id: course_id.to_string(),
                course_name: course_name.clone(),
                course_link: course["alternateLink"].as_str().map(str::to_string),
                title: item["title"].as_str().unwrap_or("Untitled coursework").to_string(),
                state: item["state"].as_str().unwrap_or("UNKNOWN").to_string(),
                due_date: item["dueDate"].as_object().map(|o| {
                    format!(
                        "{}-{:02}-{:02}",
                        o.get("year").and_then(|v| v.as_i64()).unwrap_or_default(),
                        o.get("month").and_then(|v| v.as_u64()).unwrap_or_default(),
                        o.get("day").and_then(|v| v.as_u64()).unwrap_or_default()
                    )
                }),
                due_time: item["dueTime"].as_object().map(|o| {
                    format!(
                        "{:02}:{:02}",
                        o.get("hours").and_then(|v| v.as_u64()).unwrap_or_default(),
                        o.get("minutes").and_then(|v| v.as_u64()).unwrap_or_default()
                    )
                }),
                alternate_link: item["alternateLink"].as_str().map(str::to_string),
            });
        }
    }
    Ok(assignments)
}

async fn fetch_calendar(client: &Client, access_token: &str) -> Result<Vec<CalendarEvent>, String> {
    let time_min = chrono::Utc::now().to_rfc3339();
    let response = client
        .get("https://www.googleapis.com/calendar/v3/calendars/primary/events")
        .bearer_auth(access_token)
        .query(&[
            ("maxResults", "6"),
            ("orderBy", "startTime"),
            ("singleEvents", "true"),
            ("timeMin", time_min.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("Failed to load Calendar events: {}", e))?;
    let status = response.status();
    let payload: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Failed to load Calendar events ({}): {}", status, payload));
    }

    let events = payload["items"].as_array().cloned().unwrap_or_default();
    Ok(events
        .into_iter()
        .take(6)
        .map(|event| CalendarEvent {
            id: event["id"].as_str().unwrap_or_default().to_string(),
            summary: event["summary"].as_str().unwrap_or("Untitled event").to_string(),
            start: event["start"]["dateTime"]
                .as_str()
                .or_else(|| event["start"]["date"].as_str())
                .map(str::to_string),
            end: event["end"]["dateTime"]
                .as_str()
                .or_else(|| event["end"]["date"].as_str())
                .map(str::to_string),
            html_link: event["htmlLink"].as_str().map(str::to_string),
        })
        .collect())
}
