use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::thread;
use std::time::Duration;

use arboard::Clipboard;
use base64::Engine;
use image::ImageEncoder;

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if path == "~" || path.starts_with("~/") {
        #[cfg(target_os = "windows")]
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        #[cfg(not(target_os = "windows"))]
        let home = std::env::var("HOME").unwrap_or_default();

        if home.is_empty() {
            return path.to_string();
        }

        if path == "~" {
            return home;
        }
        return path.replacen("~", &home, 1);
    }
    path.to_string()
}

#[derive(Serialize)]
pub struct AttachmentInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(rename = "dataUrl")]
    pub data_url: Option<String>,
    pub size: Option<u64>,
}

fn classify_attachment(path: &str, size: Option<u64>) -> AttachmentInfo {
    let p = Path::new(path);
    let name = p
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let mime = mime_guess::from_path(p).first().map(|m| m.essence_str().to_string());
    let is_image = mime
        .as_deref()
        .map(|m| m.starts_with("image/"))
        .unwrap_or(false);

    AttachmentInfo {
        id: uuid::Uuid::new_v4().to_string(),
        kind: if is_image { "image".into() } else { "file".into() },
        name,
        path: path.to_string(),
        mime_type: mime,
        data_url: None,
        size,
    }
}

fn image_signature(width: u32, height: u32, rgba: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    let take = rgba.len().min(4096);
    rgba[..take].hash(&mut hasher);
    hasher.finish()
}

fn clipboard_image_signature() -> Option<u64> {
    let mut clipboard = Clipboard::new().ok()?;
    let img = clipboard.get_image().ok()?;
    let width = img.width as u32;
    let height = img.height as u32;
    let rgba = img.bytes.into_owned();
    Some(image_signature(width, height, &rgba))
}

pub fn select_directory_dialog() -> Result<Option<String>, String> {
    let picked = rfd::FileDialog::new().pick_folder();
    Ok(picked.map(|p| p.to_string_lossy().to_string()))
}

pub fn attach_files_dialog() -> Result<Vec<AttachmentInfo>, String> {
    let picked = rfd::FileDialog::new().pick_files();
    let Some(files) = picked else {
        return Ok(vec![]);
    };

    let mut result = Vec::with_capacity(files.len());
    for p in files {
        let path = p.to_string_lossy().to_string();
        let size = fs::metadata(&p).ok().map(|m| m.len());
        result.push(classify_attachment(&path, size));
    }
    Ok(result)
}

fn encode_clipboard_image_to_attachment() -> Result<(AttachmentInfo, u64), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    let img = clipboard.get_image().map_err(|e| e.to_string())?;

    let width = img.width as u32;
    let height = img.height as u32;
    let rgba = img.bytes.into_owned();
    let signature = image_signature(width, height, &rgba);

    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;

    let temp_path = std::env::temp_dir().join(format!("imprint-shot-{}.png", uuid::Uuid::new_v4()));
    fs::write(&temp_path, &png).map_err(|e| e.to_string())?;
    let data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    );

    Ok((AttachmentInfo {
        id: uuid::Uuid::new_v4().to_string(),
        kind: "image".into(),
        name: temp_path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "screenshot.png".to_string()),
        path: temp_path.to_string_lossy().to_string(),
        mime_type: Some("image/png".to_string()),
        data_url: Some(data_url),
        size: Some(png.len() as u64),
    }, signature))
}

pub fn take_screenshot_interactive() -> Result<Option<AttachmentInfo>, String> {
    let previous_signature = clipboard_image_signature();

    // Reduce stale-image pickup risk when polling clipboard after capture UX.
    if let Ok(mut clipboard) = Clipboard::new() {
        let _ = clipboard.set_text("imprint_screenshot_pending");
    }

    #[cfg(target_os = "windows")]
    {
        // Opens Windows snipping overlay.
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", "ms-screenclip:"])
            .status();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("screencapture")
            .args(["-i", "-c"])
            .status();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("sh").arg("-c").arg(
            "if command -v gnome-screenshot >/dev/null 2>&1; then gnome-screenshot -a -c; \
             elif command -v grim >/dev/null 2>&1 && command -v slurp >/dev/null 2>&1 && command -v wl-copy >/dev/null 2>&1; then grim -g \"$(slurp)\" - | wl-copy -t image/png; \
             elif command -v maim >/dev/null 2>&1 && command -v xclip >/dev/null 2>&1; then maim -s | xclip -selection clipboard -t image/png -i; \
             else exit 127; fi"
        ).status();
    }

    // Poll clipboard up to ~20s for captured image.
    for _ in 0..100 {
        match encode_clipboard_image_to_attachment() {
            Ok((att, sig)) => {
                // Accept only a newly captured image, not a stale clipboard image.
                if previous_signature.is_some() && Some(sig) == previous_signature {
                    thread::sleep(Duration::from_millis(200));
                    continue;
                }
                return Ok(Some(att));
            }
            Err(_) => thread::sleep(Duration::from_millis(200)),
        }
    }

    Err("No screenshot image detected on clipboard".to_string())
}

pub fn open_external_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // Avoid `cmd /C start` here because OAuth URLs contain `&` query
        // parameters, which cmd treats as command separators and truncates.
        // Use PowerShell Start-Process so the full URL is passed through safely.
        let escaped = url.replace('\'', "''");
        let command = format!("Start-Process '{}'", escaped);
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &command])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Failed to open URL".into());
        }
    }

    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg(url)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Failed to open URL".into());
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = std::process::Command::new("xdg-open")
            .arg(url)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Failed to open URL".into());
        }
    }

    Ok(())
}

pub fn open_in_terminal(path: &str) -> Result<(), String> {
    let expanded = expand_tilde(path);

    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("cmd")
            .args(["/C", "start", "", "cmd", "/K", "cd", "/d", &expanded])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Failed to open terminal".into());
        }
    }

    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .args(["-a", "Terminal", &expanded])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Failed to open terminal".into());
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let terminal_cmd = format!(
            "if command -v x-terminal-emulator >/dev/null 2>&1; then x-terminal-emulator --working-directory='{}'; \
             elif command -v gnome-terminal >/dev/null 2>&1; then gnome-terminal --working-directory='{}'; \
             elif command -v konsole >/dev/null 2>&1; then konsole --workdir '{}'; \
             else xdg-open '{}'; fi",
            expanded, expanded, expanded, expanded
        );
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(terminal_cmd)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Failed to open terminal".into());
        }
    }

    Ok(())
}

/// List directory contents as JSON.
pub fn list_dir(path: &str) -> Result<String, String> {
    let expanded = expand_tilde(path);
    let entries = fs::read_dir(&expanded).map_err(|e| format!("Failed to read dir: {}", e))?;

    let mut results = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read metadata: {}", e))?;

        let modified_unix = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let item = serde_json::json!({
            "name": entry.file_name().to_string_lossy(),
            "is_dir": metadata.is_dir(),
            "size_bytes": if metadata.is_dir() { 0 } else { metadata.len() },
            "modified_unix": modified_unix,
        });
        results.push(item);
    }

    serde_json::to_string_pretty(&results).map_err(|e| format!("JSON error: {}", e))
}

/// Create a directory (including parents).
pub fn create_dir(path: &str) -> Result<String, String> {
    let expanded = expand_tilde(path);
    fs::create_dir_all(&expanded).map_err(|e| format!("Failed to create dir: {}", e))?;
    Ok(format!("Created directory: {}", expanded))
}

/// Move (rename) a file or folder.
pub fn move_file(from: &str, to: &str) -> Result<String, String> {
    let from_expanded = expand_tilde(from);
    let to_expanded = expand_tilde(to);
    fs::rename(&from_expanded, &to_expanded).map_err(|e| format!("Failed to move: {}", e))?;
    Ok(format!("Moved {} → {}", from, to))
}

/// Delete a file or directory.
pub fn delete_file(path: &str) -> Result<String, String> {
    let expanded = expand_tilde(path);
    let meta = fs::metadata(&expanded).map_err(|e| format!("Failed to stat: {}", e))?;
    if meta.is_dir() {
        fs::remove_dir_all(&expanded).map_err(|e| format!("Failed to delete dir: {}", e))?;
    } else {
        fs::remove_file(&expanded).map_err(|e| format!("Failed to delete file: {}", e))?;
    }
    Ok(format!("Deleted: {}", expanded))
}

/// Open a path in the OS-native file manager.
pub fn open_path(path: &str) -> Result<String, String> {
    let expanded = expand_tilde(path);

    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("explorer")
            .arg(&expanded)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("Failed to open path in Explorer: {}", expanded));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg(&expanded)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("Failed to open path in Finder: {}", expanded));
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = std::process::Command::new("xdg-open")
            .arg(&expanded)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("Failed to open path in file manager: {}", expanded));
        }
    }

    Ok(format!("Opened path: {}", expanded))
}

/// Backward-compatible alias kept for previously generated tool calls.
pub fn open_finder(path: &str) -> Result<String, String> {
    open_path(path)
}

/// Run shell commands using an OS-appropriate shell.
pub fn shell_exec(command: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    let output = std::process::Command::new("powershell")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(command)
        .output()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    #[cfg(not(target_os = "windows"))]
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !stderr.is_empty() && stdout.is_empty() {
        Err(stderr)
    } else {
        Ok(format!("{}{}", stdout, stderr))
    }
}

/// Run an AppleScript to control any macOS application.
pub fn run_applescript(script: &str) -> Result<String, String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = script;
        return Err("run_applescript is only available on macOS".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|e| e.to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(stdout.trim().to_string())
        } else {
            Err(stderr.trim().to_string())
        }
    }
}

/// Read a file's text content, optionally truncating to max_chars.
pub fn read_file(path: &str, max_chars: Option<usize>) -> Result<String, String> {
    let expanded = expand_tilde(path);
    let limit = max_chars.unwrap_or(2000);

    if !Path::new(&expanded).is_file() {
        return Err(format!("Path is not a readable file: {}", expanded));
    }

    let content =
        fs::read_to_string(&expanded).map_err(|e| format!("Failed to read file: {}", e))?;

    if content.len() > limit {
        Ok(format!(
            "{}... [truncated — {} total chars]",
            &content[..limit],
            content.len()
        ))
    } else {
        Ok(content)
    }
}

// ─── Computer Use Tools ───

/// Capture the full primary screen as a base64-encoded PNG string for vision analysis.
pub fn screenshot_tool() -> Result<String, String> {
    use xcap::Monitor;

    let monitors = Monitor::all().map_err(|e| format!("Failed to list monitors: {}", e))?;
    let primary = monitors
        .into_iter()
        .find(|m| m.is_primary())
        .or_else(|| Monitor::all().ok().and_then(|mut m| if m.is_empty() { None } else { Some(m.remove(0)) }))
        .ok_or_else(|| "No monitors found".to_string())?;

    let capture = primary.capture_image().map_err(|e| format!("Screen capture failed: {}", e))?;

    // Resize for speed: cap width at 1280px to reduce token cost
    let (w, h) = (capture.width(), capture.height());
    let img: image::DynamicImage = image::DynamicImage::ImageRgba8(capture);
    let resized = if w > 1280 {
        let scale = 1280.0 / w as f64;
        let new_h = (h as f64 * scale) as u32;
        img.resize_exact(1280, new_h, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let mut png_buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png_buf)
        .write_image(
            resized.as_bytes(),
            resized.width(),
            resized.height(),
            resized.color().into(),
        )
        .map_err(|e| format!("PNG encode failed: {}", e))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_buf);
    Ok(format!("SCREENSHOT_BASE64:{}", b64))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotFrame {
    pub base64_png: String,
    pub width: u32,
    pub height: u32,
    pub native_width: u32,
    pub native_height: u32,
    pub scale_x: f64,
    pub scale_y: f64,
}

/// Capture the full primary screen and optionally downscale for faster vision inference.
/// The returned scale factors map coordinates from the vision frame back to native pixels.
pub fn screenshot_frame_tool_with_options(max_width: Option<u32>) -> Result<ScreenshotFrame, String> {
    use xcap::Monitor;

    let monitors = Monitor::all().map_err(|e| format!("Failed to list monitors: {}", e))?;
    let primary = monitors
        .into_iter()
        .find(|m| m.is_primary())
        .or_else(|| {
            Monitor::all()
                .ok()
                .and_then(|mut m| if m.is_empty() { None } else { Some(m.remove(0)) })
        })
        .ok_or_else(|| "No monitors found".to_string())?;

    let capture = primary
        .capture_image()
        .map_err(|e| format!("Screen capture failed: {}", e))?;
    let native_width = capture.width();
    let native_height = capture.height();

    let mut vision_img: image::DynamicImage = image::DynamicImage::ImageRgba8(capture);
    if let Some(target_max_w) = max_width {
        if target_max_w > 0 && native_width > target_max_w {
            let ratio = target_max_w as f64 / native_width as f64;
            let target_h = (native_height as f64 * ratio).round() as u32;
            vision_img = vision_img.resize_exact(
                target_max_w,
                target_h.max(1),
                image::imageops::FilterType::Triangle,
            );
        }
    }

    let width = vision_img.width();
    let height = vision_img.height();
    let scale_x = native_width as f64 / width.max(1) as f64;
    let scale_y = native_height as f64 / height.max(1) as f64;

    let mut png_buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png_buf)
        .write_image(
            vision_img.as_bytes(),
            width,
            height,
            vision_img.color().into(),
        )
        .map_err(|e| format!("PNG encode failed: {}", e))?;

    Ok(ScreenshotFrame {
        base64_png: base64::engine::general_purpose::STANDARD.encode(&png_buf),
        width,
        height,
        native_width,
        native_height,
        scale_x,
        scale_y,
    })
}

/// Move the mouse cursor to absolute screen coordinates (x, y).
/// Cross-platform: on Windows also calls SetCursorPos (which IS DPI-aware)
/// to work around enigo's internal mouse_event() being DPI-unaware on
/// high-DPI / scaled displays.
pub fn mouse_move_tool(x: i32, y: i32) -> Result<String, String> {
    use enigo::{Coordinate, Enigo, Mouse, Settings};

    // Windows: SetCursorPos via inline PowerShell for DPI-correct placement
    #[cfg(target_os = "windows")]
    {
        let ps = format!(
            "Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;\
             public class WinCursor{{[DllImport(\"user32.dll\")]public static extern bool SetCursorPos(int x,int y);}}'; \
             [WinCursor]::SetCursorPos({},{})",
            x, y
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
            .output();
        thread::sleep(Duration::from_millis(80));
    }

    // Also use enigo so its internal state tracks the position for click calls
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Enigo init failed: {}", e))?;
    enigo
        .move_mouse(x, y, Coordinate::Abs)
        .map_err(|e| format!("Mouse move failed: {}", e))?;

    // Let the OS/display-server settle before the next action
    thread::sleep(Duration::from_millis(120));
    Ok(format!("Mouse moved to ({}, {})", x, y))
}

/// Click the mouse at the current cursor position.
pub fn mouse_click_tool(button: &str, click_type: &str) -> Result<String, String> {
    use enigo::{Enigo, Mouse, Settings, Button, Direction};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("Enigo init failed: {}", e))?;

    let btn = match button.to_lowercase().as_str() {
        "right" => Button::Right,
        "middle" => Button::Middle,
        _ => Button::Left,
    };

    match click_type.to_lowercase().as_str() {
        "double" => {
            enigo.button(btn, Direction::Click).map_err(|e| format!("Click failed: {}", e))?;
            thread::sleep(Duration::from_millis(60));
            enigo.button(btn, Direction::Click).map_err(|e| format!("Click failed: {}", e))?;
        }
        _ => {
            enigo.button(btn, Direction::Click).map_err(|e| format!("Click failed: {}", e))?;
        }
    }

    thread::sleep(Duration::from_millis(80));
    Ok(format!("{} {} click performed", button, click_type))
}

/// Type a text string using the keyboard.
pub fn keyboard_type_tool(text: &str) -> Result<String, String> {
    use enigo::{Enigo, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("Enigo init failed: {}", e))?;
    enigo.text(text).map_err(|e| format!("Typing failed: {}", e))?;
    thread::sleep(Duration::from_millis(100));
    Ok(format!("Typed: \"{}\"", if text.len() > 60 { &text[..57] } else { text }))
}

/// Press a keyboard shortcut/hotkey combination (e.g. ["ctrl", "c"] or ["win", "s"]).
pub fn keyboard_hotkey_tool(keys: &[String]) -> Result<String, String> {
    use enigo::{Enigo, Keyboard, Settings, Key, Direction};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("Enigo init failed: {}", e))?;

    let parse_key = |k: &str| -> Result<Key, String> {
        match k.to_lowercase().as_str() {
            "ctrl" | "control" => Ok(Key::Control),
            "alt" => Ok(Key::Alt),
            "shift" => Ok(Key::Shift),
            "win" | "super" | "meta" | "command" | "cmd" => Ok(Key::Meta),
            "enter" | "return" => Ok(Key::Return),
            "tab" => Ok(Key::Tab),
            "escape" | "esc" => Ok(Key::Escape),
            "backspace" => Ok(Key::Backspace),
            "delete" | "del" => Ok(Key::Delete),
            "space" => Ok(Key::Space),
            "up" => Ok(Key::UpArrow),
            "down" => Ok(Key::DownArrow),
            "left" => Ok(Key::LeftArrow),
            "right" => Ok(Key::RightArrow),
            "home" => Ok(Key::Home),
            "end" => Ok(Key::End),
            "pageup" => Ok(Key::PageUp),
            "pagedown" => Ok(Key::PageDown),
            "f1" => Ok(Key::F1),
            "f2" => Ok(Key::F2),
            "f3" => Ok(Key::F3),
            "f4" => Ok(Key::F4),
            "f5" => Ok(Key::F5),
            "f6" => Ok(Key::F6),
            "f7" => Ok(Key::F7),
            "f8" => Ok(Key::F8),
            "f9" => Ok(Key::F9),
            "f10" => Ok(Key::F10),
            "f11" => Ok(Key::F11),
            "f12" => Ok(Key::F12),
            single if single.len() == 1 => Ok(Key::Unicode(single.chars().next().unwrap())),
            other => Err(format!("Unknown key: {}", other)),
        }
    };

    let parsed: Vec<Key> = keys.iter().map(|k| parse_key(k)).collect::<Result<Vec<_>, _>>()?;

    // Press all keys down
    for key in &parsed {
        enigo.key(*key, Direction::Press).map_err(|e| format!("Key press failed: {}", e))?;
        thread::sleep(Duration::from_millis(30));
    }

    // Release in reverse order
    for key in parsed.iter().rev() {
        enigo.key(*key, Direction::Release).map_err(|e| format!("Key release failed: {}", e))?;
        thread::sleep(Duration::from_millis(30));
    }

    thread::sleep(Duration::from_millis(100));
    Ok(format!("Hotkey pressed: {}", keys.join("+")))
}

/// Scroll the mouse wheel.
pub fn mouse_scroll_tool(direction: &str, amount: i32) -> Result<String, String> {
    use enigo::{Enigo, Mouse, Settings, Axis};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("Enigo init failed: {}", e))?;

    let scroll_amount = match direction.to_lowercase().as_str() {
        "up" => amount,
        "down" => -amount,
        "left" => amount,   // horizontal
        "right" => -amount, // horizontal
        _ => return Err(format!("Invalid scroll direction: {}. Use up/down/left/right.", direction)),
    };

    let axis = match direction.to_lowercase().as_str() {
        "left" | "right" => Axis::Horizontal,
        _ => Axis::Vertical,
    };

    enigo.scroll(scroll_amount, axis).map_err(|e| format!("Scroll failed: {}", e))?;
    thread::sleep(Duration::from_millis(80));
    Ok(format!("Scrolled {} by {}", direction, amount))
}

/// Dispatch a tool call by name.
pub fn dispatch_tool(name: &str, args: &serde_json::Value) -> Result<String, String> {
    match name {
        "list_dir" => list_dir(args["path"].as_str().unwrap_or("")),
        "create_dir" => create_dir(args["path"].as_str().unwrap_or("")),
        "move_file" => move_file(
            args["from"].as_str().unwrap_or(""),
            args["to"].as_str().unwrap_or(""),
        ),
        "delete_file" => delete_file(args["path"].as_str().unwrap_or("")),
        "open_path" => open_path(args["path"].as_str().unwrap_or("")),
        "open_finder" => open_finder(args["path"].as_str().unwrap_or("")),
        "shell_exec" => shell_exec(args["command"].as_str().unwrap_or("")),
        "run_applescript" => run_applescript(args["script"].as_str().unwrap_or("")),
        "read_file" => read_file(
            args["path"].as_str().unwrap_or(""),
            args["max_chars"].as_u64().map(|n| n as usize),
        ),
        // ── Computer Use Tools ──
        "screenshot" => screenshot_tool(),
        "mouse_move" => mouse_move_tool(
            args["x"].as_i64().unwrap_or(0) as i32,
            args["y"].as_i64().unwrap_or(0) as i32,
        ),
        "mouse_click" => mouse_click_tool(
            args["button"].as_str().unwrap_or("left"),
            args["click_type"].as_str().unwrap_or("single"),
        ),
        "keyboard_type" => keyboard_type_tool(
            args["text"].as_str().unwrap_or(""),
        ),
        "keyboard_hotkey" => {
            let keys: Vec<String> = args["keys"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            keyboard_hotkey_tool(&keys)
        }
        "mouse_scroll" => mouse_scroll_tool(
            args["direction"].as_str().unwrap_or("down"),
            args["amount"].as_i64().unwrap_or(3) as i32,
        ),
        _ => Err(format!("Unknown tool: {}", name)),
    }
}
