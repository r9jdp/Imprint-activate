use super::{BrowserManager, BrowserState};
use tauri::State;

#[tauri::command]
pub async fn browser_connect(state: State<'_, BrowserState>) -> Result<String, String> {
    BrowserManager::ensure_connected(&state).await?;
    Ok("Connected to Chrome via CDP".to_string())
}

#[tauri::command]
pub async fn browser_navigate_command(
    url: String,
    state: State<'_, BrowserState>,
) -> Result<String, String> {
    BrowserManager::ensure_connected(&state).await?;

    let mut guard = state.lock().await;
    let manager = guard.as_mut().ok_or("BrowserManager not initialized")?;
    let page = manager.get_active_page(Some(&url)).await?;

    page.goto(&url)
        .await
        .map_err(|e| format!("Navigation failed: {}", e))?;

    let current_url = page.url().await
        .map_err(|e| format!("Failed to get URL: {}", e))?
        .unwrap_or_default();

    Ok(format!("Navigated to: {}", current_url))
}

#[tauri::command]
pub async fn browser_screenshot_command(
    state: State<'_, BrowserState>,
) -> Result<String, String> {
    BrowserManager::ensure_connected(&state).await?;

    let mut guard = state.lock().await;
    let manager = guard.as_mut().ok_or("BrowserManager not initialized")?;
    let page = manager.get_active_page(None).await?;

    use base64::Engine;
    use chromiumoxide::page::ScreenshotParams;

    let bytes = page
        .screenshot(ScreenshotParams::builder().full_page(false).build())
        .await
        .map_err(|e| format!("Screenshot failed: {}", e))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{}", b64))
}

#[tauri::command]
pub async fn browser_disconnect(state: State<'_, BrowserState>) -> Result<(), String> {
    let mut guard = state.lock().await;
    *guard = None;
    Ok(())
}

/// One-time onboarding: configure Chrome to always launch with remote debugging port.
#[tauri::command]
pub async fn browser_setup_chrome() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        // Use macOS defaults to set RemoteDebuggingPort permanently
        let output = std::process::Command::new("defaults")
            .args(["write", "com.google.Chrome", "RemoteDebuggingPort", "-int", "9222"])
            .output()
            .map_err(|e| format!("Failed to run defaults command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to configure Chrome: {}", stderr));
        }

        // Also write a launch script as a fallback
        let app_support = dirs::data_dir()
            .ok_or("Could not determine Application Support directory")?;
        let imprint_dir = app_support.join("Imprint");
        std::fs::create_dir_all(&imprint_dir)
            .map_err(|e| format!("Failed to create Imprint directory: {}", e))?;

        let script_path = imprint_dir.join("launch_chrome.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\n/Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome --remote-debugging-port=9222 \"$@\"\n",
        )
        .map_err(|e| format!("Failed to write launch script: {}", e))?;

        // Make executable
        std::process::Command::new("chmod")
            .args(["+x", &script_path.to_string_lossy()])
            .output()
            .map_err(|e| format!("Failed to chmod launch script: {}", e))?;

        Ok("Chrome configured via macOS defaults. Restart Chrome for changes to take effect. Chrome will now always start with remote debugging on port 9222.".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, write a helper batch file that launches Chrome with the debug flag
        let app_data = std::env::var("APPDATA")
            .map_err(|_| "Could not determine APPDATA directory".to_string())?;
        let imprint_dir = std::path::PathBuf::from(&app_data).join("Imprint");
        std::fs::create_dir_all(&imprint_dir)
            .map_err(|e| format!("Failed to create Imprint directory: {}", e))?;

        let script_path = imprint_dir.join("launch_chrome.bat");
        std::fs::write(
            &script_path,
            "@echo off\r\nstart \"\" \"C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe\" --remote-debugging-port=9222 %*\r\n",
        )
        .map_err(|e| format!("Failed to write launch script: {}", e))?;

        Ok(format!(
            "Chrome launch script written to {}. Use this script to start Chrome, or add --remote-debugging-port=9222 to your Chrome shortcut target. Restart Chrome for changes to take effect.",
            script_path.display()
        ))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Linux: write a desktop override or shell script
        let home = std::env::var("HOME")
            .map_err(|_| "Could not determine HOME directory".to_string())?;
        let imprint_dir = std::path::PathBuf::from(&home).join(".local/share/Imprint");
        std::fs::create_dir_all(&imprint_dir)
            .map_err(|e| format!("Failed to create Imprint directory: {}", e))?;

        let script_path = imprint_dir.join("launch_chrome.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\ngoogle-chrome --remote-debugging-port=9222 \"$@\"\n",
        )
        .map_err(|e| format!("Failed to write launch script: {}", e))?;

        std::process::Command::new("chmod")
            .args(["+x", &script_path.to_string_lossy()])
            .output()
            .map_err(|e| format!("Failed to chmod launch script: {}", e))?;

        Ok(format!(
            "Chrome launch script written to {}. Use this script to start Chrome, or launch Chrome manually with --remote-debugging-port=9222. Restart Chrome for changes to take effect.",
            script_path.display()
        ))
    }
}
