mod agent;
pub mod browser;
mod google;
mod safety;
mod tools;
mod types;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

use safety::scope::{ScopeConfig, ScopeGuard};
use safety::undo::UndoStack;
use safety::plan::PlanApprovalState;

#[derive(serde::Serialize)]
struct GoogleConnectionStatus {
    configured: bool,
    connected: bool,
}

#[derive(serde::Deserialize)]
struct WorkspaceAssistantRequest {
    query: String,
    snapshot: google::WorkspaceSnapshot,
}

#[derive(serde::Deserialize)]
struct GcpServiceAccountKey {
    client_email: String,
    private_key: String,
    token_uri: Option<String>,
}

#[derive(serde::Serialize)]
struct GcpJwtClaims {
    iss: String,
    scope: String,
    aud: String,
    exp: usize,
    iat: usize,
}

fn read_runtime_env(name: &str) -> Option<String> {
    if let Ok(v) = std::env::var(name) {
        let trimmed = v.trim().to_string();
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

fn load_gcp_service_key() -> Result<GcpServiceAccountKey, String> {
    let configured_path = read_runtime_env("GCP_SERVICE_ACCOUNT_KEY_PATH")
        .or_else(|| read_runtime_env("GOOGLE_APPLICATION_CREDENTIALS"))
        .unwrap_or_else(|| "keys.json".to_string());

    let path = std::path::PathBuf::from(&configured_path);
    let candidates = if path.is_absolute() {
        vec![path]
    } else {
        vec![
            std::path::PathBuf::from(&configured_path),
            std::path::PathBuf::from("..").join(&configured_path),
        ]
    };

    let selected = candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| format!("Service account key file not found: {}", configured_path))?;

    let raw = std::fs::read_to_string(&selected)
        .map_err(|e| format!("Failed to read service key at {}: {}", selected.display(), e))?;
    serde_json::from_str::<GcpServiceAccountKey>(&raw)
        .map_err(|e| format!("Invalid service account JSON: {}", e))
}

async fn fetch_gcp_access_token(sa: &GcpServiceAccountKey) -> Result<String, String> {
    let token_uri = sa
        .token_uri
        .clone()
        .unwrap_or_else(|| "https://oauth2.googleapis.com/token".to_string());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as usize;

    let claims = GcpJwtClaims {
        iss: sa.client_email.clone(),
        scope: "https://www.googleapis.com/auth/cloud-platform".to_string(),
        aud: token_uri.clone(),
        exp: now + 3600,
        iat: now,
    };

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
        .map_err(|e| format!("Invalid RSA private key in service account JSON: {}", e))?;
    let assertion = jsonwebtoken::encode(&header, &claims, &key)
        .map_err(|e| format!("JWT signing failed: {}", e))?;

    let client = reqwest::Client::new();
    let response = client
        .post(&token_uri)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", assertion.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("OAuth token request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("OAuth token request failed ({}): {}", status, body));
    }

    let token_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Invalid OAuth token response: {}", e))?;

    token_json["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "OAuth response missing access_token".to_string())
}

#[tauri::command]
async fn transcribe_audio_command(audio_base64: String) -> Result<serde_json::Value, String> {
    if audio_base64.trim().is_empty() {
        return Ok(serde_json::json!({
            "transcript": "",
            "error": "Empty audio payload",
        }));
    }

    let key = load_gcp_service_key()?;
    let access_token = fetch_gcp_access_token(&key).await?;

    let language_code = read_runtime_env("GCP_STT_LANGUAGE_CODE").unwrap_or_else(|| "en-US".to_string());
    let model = read_runtime_env("GCP_STT_MODEL").unwrap_or_else(|| "latest_short".to_string());

    let body = serde_json::json!({
        "config": {
            "encoding": "LINEAR16",
            "sampleRateHertz": 16000,
            "languageCode": language_code,
            "model": model,
            "enableAutomaticPunctuation": true,
        },
        "audio": {
            "content": audio_base64,
        }
    });

    let client = reqwest::Client::new();
    let response = client
        .post("https://speech.googleapis.com/v1/speech:recognize")
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Speech API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_body = response.text().await.unwrap_or_default();
        return Ok(serde_json::json!({
            "transcript": "",
            "error": format!("Speech API error ({}): {}", status, err_body),
        }));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Invalid Speech API response: {}", e))?;

    let transcript = payload["results"]
        .as_array()
        .map(|results| {
            results
                .iter()
                .filter_map(|r| r["alternatives"].as_array())
                .filter_map(|alts| alts.first())
                .filter_map(|alt| alt["transcript"].as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
        .trim()
        .to_string();

    Ok(serde_json::json!({
        "transcript": transcript,
        "error": serde_json::Value::Null,
    }))
}

pub struct InterruptState(pub AtomicBool);

#[tauri::command]
async fn interrupt_agent(state: State<'_, InterruptState>) -> Result<(), String> {
    state.0.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
async fn run_agent_command(
    prompt: String,
    history: Vec<serde_json::Value>,
    window: tauri::Window,
    state: State<'_, InterruptState>,
    scope_guard: State<'_, ScopeGuard>,
    undo_stack: State<'_, UndoStack>,
    plan_state: State<'_, PlanApprovalState>,
    browser_state: State<'_, browser::BrowserState>,
) -> Result<(), String> {
    state.0.store(false, Ordering::SeqCst);
    agent::run_agent(prompt, history, window, state, scope_guard, undo_stack, plan_state, browser_state).await
}

// ─── Safety commands ───

#[tauri::command]
async fn get_scope_config(scope_guard: State<'_, ScopeGuard>) -> Result<ScopeConfig, String> {
    scope_guard.get_config()
}

#[tauri::command]
async fn set_scope_config(
    config: ScopeConfig,
    scope_guard: State<'_, ScopeGuard>,
) -> Result<(), String> {
    scope_guard.update_config(config)
}

#[tauri::command]
async fn get_undo_stack(
    undo_stack: State<'_, UndoStack>,
) -> Result<Vec<safety::undo::UndoEntry>, String> {
    Ok(undo_stack.get_entries())
}

#[tauri::command]
async fn undo_last(undo_stack: State<'_, UndoStack>) -> Result<String, String> {
    let entry = undo_stack
        .pop_last()
        .ok_or_else(|| "No operations to undo".to_string())?;
    safety::undo::apply_undo(&entry)
}

#[tauri::command]
async fn undo_all(undo_stack: State<'_, UndoStack>) -> Result<Vec<String>, String> {
    let entries = undo_stack.get_entries();
    if entries.is_empty() {
        return Err("Nothing to undo — no operations were recorded for the last run.".to_string());
    }
    let mut results = Vec::new();
    // Undo in reverse order (newest first)
    for entry in entries.iter().rev() {
        let msg = match safety::undo::apply_undo(entry) {
            Ok(m) => m,
            Err(e) => format!("Could not undo '{}': {}", entry.description, e),
        };
        results.push(msg);
        undo_stack.remove_by_id(&entry.id);
    }
    Ok(results)
}

#[tauri::command]
async fn undo_specific(id: String, undo_stack: State<'_, UndoStack>) -> Result<String, String> {
    let entry = undo_stack
        .remove_by_id(&id)
        .ok_or_else(|| format!("No undo entry found with id: {}", id))?;
    safety::undo::apply_undo(&entry)
}

#[tauri::command]
async fn approve_plan(
    plan_id: String,
    plan_state: State<'_, PlanApprovalState>,
) -> Result<(), String> {
    plan_state.approve(&plan_id)
}

#[tauri::command]
async fn reject_plan(
    plan_id: String,
    _reason: Option<String>,
    plan_state: State<'_, PlanApprovalState>,
) -> Result<(), String> {
    plan_state.reject(&plan_id)
}

#[tauri::command]
async fn select_directory_command() -> Result<Option<String>, String> {
    tools::select_directory_dialog()
}

#[tauri::command]
async fn attach_files_command() -> Result<Vec<tools::AttachmentInfo>, String> {
    tools::attach_files_dialog()
}

#[tauri::command]
async fn take_screenshot_command() -> Result<Option<tools::AttachmentInfo>, String> {
    tools::take_screenshot_interactive()
}

#[tauri::command]
async fn open_external_command(url: String) -> Result<(), String> {
    tools::open_external_url(&url)
}

#[tauri::command]
async fn open_in_terminal_command(path: String) -> Result<(), String> {
    tools::open_in_terminal(&path)
}

#[tauri::command]
async fn google_connection_status() -> Result<GoogleConnectionStatus, String> {
    let config = google::load_config()?;
    Ok(GoogleConnectionStatus {
        configured: !config.client_id.trim().is_empty() && !config.client_secret.trim().is_empty(),
        connected: google::is_connected(),
    })
}

#[tauri::command]
async fn sign_in_google() -> Result<google::GoogleProfile, String> {
    google::sign_in().await
}

#[tauri::command]
async fn disconnect_google() -> Result<(), String> {
    google::clear_tokens()
}

#[tauri::command]
async fn get_workspace_snapshot() -> Result<google::WorkspaceSnapshot, String> {
    google::snapshot().await
}

#[tauri::command]
async fn ask_workspace_assistant(
    request: WorkspaceAssistantRequest,
) -> Result<String, String> {
    let api_key = env!("GEMINI_API_KEY");
    if api_key.trim().is_empty() {
        return Err("GEMINI_API_KEY is missing. Add it to src-tauri/.env and rebuild the app.".to_string());
    }

    if request.query.trim().is_empty() {
        return Err("Query cannot be empty.".to_string());
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent?key={}",
        api_key
    );

    let snapshot_json = serde_json::to_string_pretty(&request.snapshot)
        .map_err(|e| format!("Failed to serialize workspace snapshot: {}", e))?;

    let prompt = format!(
        "You are Imprint, a stateful student assistant.\n\
         Use the connected Gmail, Google Classroom, and Calendar context to answer the user's request.\n\
         Prioritize clarity, actionability, and urgency.\n\
         If asked what to focus on, rank tasks by urgency and student impact.\n\
         If asked to draft or summarize, ground the response in the workspace data only.\n\
         Keep the answer concise and structured.\n\n\
         Workspace snapshot:\n{}\n\n\
         User query:\n{}",
        snapshot_json,
        request.query.trim()
    );

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .json(&serde_json::json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "temperature": 0.3
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Gemini request failed: {}", e))?;

    let status = response.status();
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

    if !status.is_success() {
        let msg = payload["error"]["message"]
            .as_str()
            .unwrap_or("Gemini returned an error");
        return Err(format!("Gemini error ({}): {}", status, msg));
    }

    let text = payload["candidates"][0]["content"]["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if text.is_empty() {
        return Err("Gemini returned an empty response.".to_string());
    }

    Ok(text)
}

use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

#[cfg(target_os = "windows")]
fn is_toggle_shortcut(shortcut: &tauri_plugin_global_shortcut::Shortcut) -> bool {
    shortcut.matches(Modifiers::ALT | Modifiers::SHIFT, Code::Space)
}

#[cfg(not(target_os = "windows"))]
fn is_toggle_shortcut(shortcut: &tauri_plugin_global_shortcut::Shortcut) -> bool {
    shortcut.matches(Modifiers::ALT, Code::Space)
}

fn anchor_window_bottom_center(window: &tauri::WebviewWindow) {
    // Keep a small safety inset from bottom so the pill sits above taskbar/dock.
    #[cfg(target_os = "windows")]
    let bottom_inset: i32 = 48;
    #[cfg(target_os = "macos")]
    let bottom_inset: i32 = 32;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let bottom_inset: i32 = 32;

    let Ok(size) = window.outer_size() else { return };
    let Ok(Some(monitor)) = window.current_monitor() else { return };

    let mon_size = monitor.size();
    let mon_pos = monitor.position();

    let x = mon_pos.x + ((mon_size.width as i32 - size.width as i32) / 2);
    let mut y = mon_pos.y + (mon_size.height as i32 - size.height as i32 - bottom_inset);
    if y < mon_pos.y {
        y = mon_pos.y;
    }

    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(InterruptState(AtomicBool::new(false)))
        .manage(ScopeGuard::from_saved())
        .manage(UndoStack::new(50))
        .manage(PlanApprovalState::new())
        .manage(browser::new_state())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts([
                    #[cfg(target_os = "windows")]
                    "alt+shift+space",
                    #[cfg(not(target_os = "windows"))]
                    "alt+space",
                ])
                .unwrap()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed && is_toggle_shortcut(&shortcut) {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                anchor_window_bottom_center(&window);
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(),
        )
            .invoke_handler(tauri::generate_handler![
                run_agent_command,
                interrupt_agent,
                select_directory_command,
                attach_files_command,
                take_screenshot_command,
                transcribe_audio_command,
                open_external_command,
                open_in_terminal_command,
                google_connection_status,
                sign_in_google,
                disconnect_google,
                get_workspace_snapshot,
                ask_workspace_assistant,
                get_scope_config,
                set_scope_config,
                get_undo_stack,
                undo_last,
                undo_specific,
                approve_plan,
                reject_plan,
                undo_all,
                browser::commands::browser_connect,
                browser::commands::browser_navigate_command,
                browser::commands::browser_screenshot_command,
                browser::commands::browser_disconnect,
                browser::commands::browser_setup_chrome,
            ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
