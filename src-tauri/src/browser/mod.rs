pub mod accessibility;
pub mod actions;
pub mod commands;
pub mod wait;

use chromiumoxide::browser::Browser;
use chromiumoxide::page::Page;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type BrowserState = Arc<Mutex<Option<BrowserManager>>>;

pub fn new_state() -> BrowserState {
    Arc::new(Mutex::new(None))
}

pub struct BrowserManager {
    pub browser: Browser,
    _handler_handle: tokio::task::JoinHandle<()>,
    pub active_page: Option<Page>,
}

/// Error type for browser connection failures.
#[derive(Debug)]
pub enum BrowserError {
    NotRunning(String),
    ConnectionFailed(String),
}

impl std::fmt::Display for BrowserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowserError::NotRunning(msg) => write!(f, "{}", msg),
            BrowserError::ConnectionFailed(msg) => write!(f, "{}", msg),
        }
    }
}

impl BrowserManager {
    /// Probe whether Chrome is listening on the CDP port.
    async fn is_chrome_listening() -> bool {
        reqwest::Client::new()
            .get("http://localhost:9222/json/version")
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .is_ok()
    }



    /// Return the path to the Chrome binary on this platform.
    fn chrome_binary() -> Option<&'static str> {
        #[cfg(target_os = "macos")]
        return if std::path::Path::new(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        )
        .exists()
        {
            Some("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
        } else {
            None
        };

        #[cfg(target_os = "windows")]
        {
            let candidates = [
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            ];
            for c in &candidates {
                if std::path::Path::new(c).exists() {
                    return Some(c);
                }
            }
            None
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let candidates = [
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/chromium-browser",
            ];
            for c in &candidates {
                if std::path::Path::new(c).exists() {
                    return Some(c);
                }
            }
            None
        }
    }



    /// Launch Chrome with remote debugging port using an isolated profile.
    fn launch_chrome_with_debug() -> Result<(), String> {
        let binary = Self::chrome_binary().ok_or_else(|| {
            "Chrome not found at the default installation path. Please install Google Chrome.".to_string()
        })?;

        let profile_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("~"))
            .join(".imprint")
            .join("chrome-profile");
            
        std::fs::create_dir_all(&profile_dir).map_err(|e| format!("Failed to create profile dir: {}", e))?;

        std::process::Command::new(binary)
            .args([
                "--remote-debugging-port=9222",
                &format!("--user-data-dir={}", profile_dir.display()),
                "--restore-last-session",
                "--no-first-run",
                "--no-default-browser-check",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to launch Chrome ({}): {}", binary, e))?;

        Ok(())
    }

    /// Perform the actual CDP connection once port 9222 is confirmed open.
    async fn do_connect() -> Result<Self, BrowserError> {
        let (browser, mut handler) = Browser::connect("http://localhost:9222")
            .await
            .map_err(|e| {
                BrowserError::ConnectionFailed(format!("CDP connect failed: {}", e))
            })?;

        let handler_handle = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            browser,
            _handler_handle: handler_handle,
            active_page: None,
        })
    }

    /// Connect to Chrome for automation.
    ///
    /// Checks if port 9222 is open. If not, launches the browser using a dedicated Imprint profile.
    pub async fn connect() -> Result<Self, BrowserError> {
        // Fast path: port already open
        if Self::is_chrome_listening().await {
            return Self::do_connect().await;
        }

        // Launch isolated automation Chrome
        Self::launch_chrome_with_debug().map_err(|e| BrowserError::ConnectionFailed(e))?;

        // Wait up to 20 seconds for port 9222 to open
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if Self::is_chrome_listening().await {
                return Self::do_connect().await;
            }
        }

        Err(BrowserError::ConnectionFailed(
            "Chrome was launched with an isolated profile, but port 9222 did not open within 20 seconds. \
             Ensure Chrome is installed at the default path.".to_string()
        ))
    }

    /// Find the most relevant page for a target URL.
    /// Prefers the tracked active page first. Then falls back to domain-match or first tab.
    pub async fn get_active_page(&mut self, target_url: Option<&str>) -> Result<Page, String> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| format!("Failed to get pages: {}", e))?;

        // 1. Try to use the active page if it is still alive
        if let Some(active) = &self.active_page {
            if active.url().await.is_ok() {
                // The channel is still alive
                // We could verify it's still in `pages`, but `url().await.is_ok()` handles closed pages.
                
                // Let's also check if they explicitly want to navigate to a new domain but 
                // normally we just use the active page. The dispatch logic handles navigating the active page.
                return Ok(active.clone());
            }
        }

        // 2. If no active page, try to find a page matching the target URL
        if let Some(url) = target_url {
            let target_host = url.split('/').nth(2).unwrap_or("");
            if !target_host.is_empty() {
                for page in &pages {
                    if let Ok(Some(current)) = page.url().await {
                        if current.contains(target_host) {
                            self.active_page = Some(page.clone());
                            return Ok(page.clone());
                        }
                    }
                }
            }
        }

        // 3. Fallback to the very first tab
        if let Some(page) = pages.into_iter().next() {
            self.active_page = Some(page.clone());
            return Ok(page);
        }
        
        let page = self.browser
            .new_page("about:blank")
            .await
            .map_err(|e| format!("Failed to create page: {}", e))?;
            
        self.active_page = Some(page.clone());
        Ok(page)
    }

    /// Close all tabs created by automation that are not the current active handle
    pub async fn close_orphaned_tabs(&mut self) {
        if let Ok(pages) = self.browser.pages().await {
            let active_target_id = self.active_page.as_ref().map(|p| p.target_id().clone());
            for page in pages {
                // If it's not the active target, attempt to close it
                if Some(page.target_id().clone()) != active_target_id {
                    let _ = page.close().await;
                }
            }
        }
    }

    /// Ensure the BrowserState has a live connection. Reconnects if dead.
    pub async fn ensure_connected(state: &BrowserState) -> Result<(), String> {
        let mut guard = state.lock().await;

        if let Some(ref manager) = *guard {
            if manager.browser.pages().await.is_ok() {
                return Ok(());
            }
            *guard = None;
        }

        let manager = BrowserManager::connect().await.map_err(|e| e.to_string())?;
        *guard = Some(manager);
        Ok(())
    }
}
