use super::accessibility::{
    self, InteractiveElement, elements_to_json, find_element_by_query,
    get_interactive_elements, inject_som_markers, remove_som_markers,
    scroll_into_view,
};
use super::wait;
use super::{BrowserManager, BrowserState};
use base64::Engine;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::cdp::browser_protocol::dom::BackendNodeId;
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

const MAX_TEXT_LENGTH: usize = 4000;
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Cached elements from the last browser_get_page_state call.
/// Used by browser_click/browser_type_text for SoM index lookups.
static CACHED_ELEMENTS: std::sync::LazyLock<Arc<Mutex<Vec<InteractiveElement>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

pub async fn dispatch_browser_tool(
    name: &str,
    args: &Value,
    state: &BrowserState,
) -> Result<String, String> {
    // Ensure we have a live connection
    BrowserManager::ensure_connected(state).await.map_err(|e| {
        format!(
            "Browser connection failed: {}. Tell the user: 'Imprint uses an isolated secure Chrome profile. A new Chrome window will open. If you need to perform actions on a logged-in site like Telegram, please log in manually in that specific window first.'",
            e
        )
    })?;

    let mut guard = state.lock().await;
    let manager = guard.as_mut().ok_or("BrowserManager not initialized")?;
    // For navigate, prefer a tab already on the target domain.
    let target_url_hint = if name == "browser_navigate" { args["url"].as_str() } else { None };
    let page = manager.get_active_page(target_url_hint).await?;

    let should_poll_tabs = name == "browser_click" || name == "browser_navigate";
    let mut known_targets = std::collections::HashSet::new();
    if should_poll_tabs {
        if let Ok(pages) = manager.browser.pages().await {
            for p in pages {
                known_targets.insert(p.target_id().clone());
            }
        }
    }

    let result = match name {
        "browser_navigate" => {
            let url = args["url"]
                .as_str()
                .ok_or("browser_navigate requires 'url' parameter")?;
            browser_navigate(&page, url).await
        }
        "browser_click" => {
            let selector = args["selector"]
                .as_str()
                .ok_or("browser_click requires 'selector' parameter")?;
            browser_click(&page, selector).await
        }
        "browser_type_text" => {
            let selector = args["selector"]
                .as_str()
                .ok_or("browser_type_text requires 'selector' parameter")?;
            let text = args["text"]
                .as_str()
                .ok_or("browser_type_text requires 'text' parameter")?;
            browser_type_text(&page, selector, text).await
        }
        "browser_scroll" => {
            let direction = args["direction"].as_str().unwrap_or("down");
            let amount = args["amount"].as_f64().unwrap_or(500.0);
            browser_scroll(&page, direction, amount).await
        }
        "browser_extract_text" => {
            let selector = args["selector"].as_str();
            browser_extract_text(&page, selector).await
        }
        "browser_screenshot" => browser_screenshot(&page).await,
        "browser_get_page_state" => browser_get_page_state(&page).await,
        "browser_wait_for" => {
            let selector = args["selector"]
                .as_str()
                .ok_or("browser_wait_for requires 'selector' parameter")?;
            let timeout_ms = args["timeout_ms"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);
            browser_wait_for(&page, selector, timeout_ms).await
        }
        "browser_go_back" => browser_go_back(&page).await,
        "browser_evaluate" => {
            let expression = args["expression"]
                .as_str()
                .ok_or("browser_evaluate requires 'expression' parameter")?;
            browser_evaluate(&page, expression).await
        }
        _ => Err(format!("Unknown browser tool: {}", name)),
    };

    if should_poll_tabs {
        // Wait and poll for new tabs with non-blank URL for up to 3 seconds
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Ok(pages) = manager.browser.pages().await {
                let mut found_new = false;
                for p in &pages {
                    if !known_targets.contains(p.target_id()) {
                        if let Ok(Some(url)) = p.url().await {
                            if !url.is_empty() && url != "about:blank" {
                                manager.active_page = Some(p.clone());
                                found_new = true;
                                break;
                            }
                        }
                    }
                }
                if found_new {
                    break;
                }
            }
        }
    }

    result
}

async fn browser_navigate(
    page: &chromiumoxide::Page,
    url: &str,
) -> Result<String, String> {
    tokio::time::timeout(Duration::from_secs(30), async {
        // Check if already on this URL — skip navigation to preserve SPA state
        let target_host = url.split('/').nth(2).unwrap_or("");
        if let Ok(Some(current_url)) = page.url().await {
            if !target_host.is_empty() && current_url.contains(target_host) {
                let title = page
                    .get_title()
                    .await
                    .map_err(|e| format!("Failed to get title: {}", e))?
                    .unwrap_or_default();
                return Ok(format!(
                    "Already on: {} (\"{}\") — reusing existing tab",
                    current_url, title
                ));
            }
        }

        page.goto(url)
            .await
            .map_err(|e| format!("Navigation failed: {}", e))?;

        // Wait for document.readyState === 'complete' (up to 5 seconds)
        for _ in 0..20 {
            let ready: String = page
                .evaluate("document.readyState")
                .await
                .ok()
                .and_then(|v| v.into_value().ok())
                .unwrap_or_default();
            if ready == "complete" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        // Smart waiting: MutationObserver + network idle (replaces hardcoded 1500ms)
        let _ = wait::wait_for_stable(&page, 5000).await;

        let current_url = page
            .url()
            .await
            .map_err(|e| format!("Failed to get URL: {}", e))?
            .unwrap_or_default();
        let title = page
            .get_title()
            .await
            .map_err(|e| format!("Failed to get title: {}", e))?
            .unwrap_or_default();

        Ok(format!("Navigated to: {} (\"{}\")", current_url, title))
    })
    .await
    .map_err(|_| "Navigation timed out after 30 seconds".to_string())?
}

async fn browser_click(
    page: &chromiumoxide::Page,
    selector: &str,
) -> Result<String, String> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let before = capture_page_snapshot(page).await?;
        let _before_mutation_count = inject_mutation_counter(page).await;

        // Try AX-based resolution first (SoM index or accessible name)
        let cached = CACHED_ELEMENTS.lock().await;
        let ax_element = find_element_by_query(selector, &cached).ok().cloned();
        drop(cached);

        let (description, click_result) = if let Some(ref el) = ax_element {
            let desc = format!("{}(\"{}\")", el.role, el.name);
            let result = click_with_retry(page, el).await;
            (desc, result)
        } else {
            // Fall back to legacy JS-based resolution
            let target = resolve_target(page, selector).await?;
            let desc = describe_target(&target);
            let result = click_legacy(page, selector, &target).await;
            (desc, result)
        };

        click_result?;

        // Smart waiting (replaces hardcoded 400ms)
        let _ = wait::wait_for_stable(page, 2000).await;

        let after = capture_page_snapshot(page).await?;
        let mutation_count = read_mutation_count(page).await;
        let verification = verify_click_outcome_generic(&before, &after, mutation_count);

        if !verification.success {
            return Err(format!(
                "Click sent to {} but the page did not react as expected. {}",
                description, verification.message
            ));
        }

        Ok(format!("Clicked {}. {}", description, verification.message))
    })
    .await
    .map_err(|_| "Click timed out after 30 seconds".to_string())?
}

/// Click an element using AX-based resolution with retry escalation.
///
/// Attempt 1: CDP dispatchMouseEvent at center coordinates
/// Attempt 2: Re-fetch bounds after 300ms, retry
/// Attempt 3: Scroll to center + retry
/// Attempt 4: Force JS click via backendNodeId
async fn click_with_retry(
    page: &chromiumoxide::Page,
    element: &InteractiveElement,
) -> Result<(), String> {
    let backend_id = element.backend_node_id.ok_or_else(|| {
        format!("Element {} has no backend node ID", element.index)
    })?;

    for attempt in 0..4 {
        // On attempt 2+, re-fetch bounds (element may have moved)
        let bounds = if attempt >= 1 {
            tokio::time::sleep(Duration::from_millis(300)).await;
            accessibility::get_element_bounds(
                page,
                BackendNodeId::new(backend_id),
            )
            .await
        } else {
            element.bounds.clone()
        };

        // Attempt 3: scroll into view first
        if attempt >= 2 {
            let _ = scroll_into_view(page, backend_id).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            // Re-fetch bounds after scroll
            let bounds_after_scroll = accessibility::get_element_bounds(
                page,
                BackendNodeId::new(backend_id),
            )
            .await;
            if let Some(ref b) = bounds_after_scroll {
                return cdp_click(page, b.center_x(), b.center_y()).await;
            }
        }

        // Attempt 4: force JS click (bypass coordinates entirely)
        if attempt >= 3 {
            return js_force_click(page, backend_id).await;
        }

        if let Some(ref b) = bounds {
            match cdp_click(page, b.center_x(), b.center_y()).await {
                Ok(()) => return Ok(()),
                Err(_) if attempt < 3 => continue,
                Err(e) => return Err(e),
            }
        }
    }

    Err(format!(
        "Failed to click element {} ('{}') after 4 attempts",
        element.index, element.name
    ))
}

/// Click at coordinates using CDP Input.dispatchMouseEvent.
async fn cdp_click(page: &chromiumoxide::Page, x: f64, y: f64) -> Result<(), String> {
    let press_params = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MousePressed)
        .x(x)
        .y(y)
        .button(MouseButton::Left)
        .click_count(1)
        .build()
        .map_err(|e| format!("Failed to build MousePressed params: {}", e))?;

    page.execute(press_params)
        .await
        .map_err(|e| format!("MousePressed failed: {}", e))?;

    let release_params = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MouseReleased)
        .x(x)
        .y(y)
        .button(MouseButton::Left)
        .click_count(1)
        .build()
        .map_err(|e| format!("Failed to build MouseReleased params: {}", e))?;

    page.execute(release_params)
        .await
        .map_err(|e| format!("MouseReleased failed: {}", e))?;

    Ok(())
}

/// Force-click via JS using CDP ResolveNode + CallFunctionOn.
async fn js_force_click(page: &chromiumoxide::Page, backend_node_id: i64) -> Result<(), String> {
    use chromiumoxide::cdp::browser_protocol::dom::ResolveNodeParams;
    use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;

    let resolve_params = ResolveNodeParams::builder()
        .backend_node_id(BackendNodeId::new(backend_node_id))
        .build();

    let resolve_result = page
        .execute(resolve_params)
        .await
        .map_err(|e| format!("ResolveNode failed: {}", e))?;

    let object_id = resolve_result
        .result
        .object
        .object_id
        .ok_or_else(|| "Resolved node has no objectId".to_string())?;

    let call_params = CallFunctionOnParams::builder()
        .object_id(object_id)
        .function_declaration("function() { this.click(); }")
        .build()
        .map_err(|e| format!("Failed to build CallFunctionOn: {}", e))?;

    page.execute(call_params)
        .await
        .map_err(|e| format!("JS force click failed: {}", e))?;

    Ok(())
}

/// Legacy click using the old JS resolve_target approach.
async fn click_legacy(
    page: &chromiumoxide::Page,
    selector: &str,
    target: &Value,
) -> Result<(), String> {
    match target["strategy"].as_str().unwrap_or("text") {
        "css" => {
            let element = page
                .find_element(selector)
                .await
                .map_err(|e| format!("CSS lookup failed '{}': {}", selector, e))?;
            element
                .click()
                .await
                .map_err(|e| format!("Click failed: {}", e))?;
        }
        _ => {
            let x = target["x"].as_f64().unwrap_or(0.0);
            let y = target["y"].as_f64().unwrap_or(0.0);
            cdp_click(page, x, y).await?;
        }
    }
    Ok(())
}

async fn browser_type_text(
    page: &chromiumoxide::Page,
    selector: &str,
    text: &str,
) -> Result<String, String> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let before = capture_page_snapshot(page).await?;

        // Try AX-based element resolution first
        let cached = CACHED_ELEMENTS.lock().await;
        let ax_element = find_element_by_query(selector, &cached).ok().cloned();
        drop(cached);

        let description = if let Some(ref el) = ax_element {
            // Use backendNodeId to focus the element
            if let Some(bid) = el.backend_node_id {
                let _ = scroll_into_view(page, bid).await;
                let _ = js_focus_by_backend_id(page, bid).await;
            }
            format!("{}(\"{}\")", el.role, el.name)
        } else {
            // Legacy: CSS selector
            let element = page
                .find_element(selector)
                .await
                .map_err(|e| format!("Element not found '{}': {}", selector, e))?;

            element
                .scroll_into_view()
                .await
                .map_err(|e| format!("Failed to scroll '{}': {}", selector, e))?;
            element
                .focus()
                .await
                .map_err(|e| format!("Failed to focus '{}': {}", selector, e))?;
            selector.to_string()
        };

        tokio::time::sleep(Duration::from_millis(75)).await;

        // Clear existing content
        let clear_js = r#"
            (() => {
                const el = document.activeElement;
                if (!el) return false;
                if ('value' in el) {
                    el.value = '';
                    el.dispatchEvent(new Event('input', { bubbles: true }));
                    el.dispatchEvent(new Event('change', { bubbles: true }));
                    return true;
                }
                if (el.isContentEditable) {
                    el.textContent = '';
                    el.dispatchEvent(new InputEvent('input', { bubbles: true, data: '' }));
                    return true;
                }
                return false;
            })()
        "#;
        let _ = page.evaluate(clear_js).await;

        // Type via CDP keyboard events
        use chromiumoxide::cdp::browser_protocol::input::InsertTextParams;
        let insert_params = InsertTextParams::new(text.to_string());
        page.execute(insert_params)
            .await
            .map_err(|e| format!("Failed to type text: {}", e))?;

        // Smart waiting (replaces hardcoded 400ms)
        let _ = wait::wait_for_stable(page, 2000).await;

        let after = capture_page_snapshot(page).await?;
        let verification = verify_type_outcome_generic(text, &before, &after);
        if !verification.success {
            return Err(format!(
                "Typing into '{}' did not update the page as expected. {}",
                description, verification.message
            ));
        }

        Ok(format!(
            "Typed \"{}\" into {}. {}",
            if text.len() > 50 {
                format!("{}...", &text[..47])
            } else {
                text.to_string()
            },
            description,
            verification.message
        ))
    })
    .await
    .map_err(|_| "Type text timed out after 30 seconds".to_string())?
}

/// Focus an element by its backend node ID via JS.
async fn js_focus_by_backend_id(page: &chromiumoxide::Page, backend_node_id: i64) -> Result<(), String> {
    use chromiumoxide::cdp::browser_protocol::dom::ResolveNodeParams;
    use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;

    let resolve_params = ResolveNodeParams::builder()
        .backend_node_id(BackendNodeId::new(backend_node_id))
        .build();

    let resolve_result = page
        .execute(resolve_params)
        .await
        .map_err(|e| format!("ResolveNode failed: {}", e))?;

    let object_id = resolve_result
        .result
        .object
        .object_id
        .ok_or_else(|| "Resolved node has no objectId".to_string())?;

    let call_params = CallFunctionOnParams::builder()
        .object_id(object_id)
        .function_declaration("function() { this.focus(); }")
        .build()
        .map_err(|e| format!("Failed to build CallFunctionOn params: {}", e))?;

    page.execute(call_params)
        .await
        .map_err(|e| format!("JS focus failed: {}", e))?;

    Ok(())
}

async fn browser_scroll(
    page: &chromiumoxide::Page,
    direction: &str,
    amount: f64,
) -> Result<String, String> {
    let y = match direction {
        "up" => -amount,
        _ => amount,
    };
    let js = format!("window.scrollBy(0, {}); [window.scrollX, window.scrollY]", y);
    let result: Vec<f64> = page
        .evaluate(js)
        .await
        .map_err(|e| format!("Scroll failed: {}", e))?
        .into_value()
        .unwrap_or_default();

    Ok(format!(
        "Scrolled {} by {}px. Current position: ({}, {})",
        direction,
        amount,
        result.first().unwrap_or(&0.0),
        result.get(1).unwrap_or(&0.0)
    ))
}

async fn browser_extract_text(
    page: &chromiumoxide::Page,
    selector: Option<&str>,
) -> Result<String, String> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let text = if let Some(sel) = selector {
            let element = page
                .find_element(sel)
                .await
                .map_err(|e| format!("Element not found '{}': {}", sel, e))?;
            element
                .inner_text()
                .await
                .map_err(|e| format!("Failed to get text: {}", e))?
                .unwrap_or_default()
        } else {
            let js = "document.body.innerText";
            page.evaluate(js)
                .await
                .map_err(|e| format!("Failed to extract page text: {}", e))?
                .into_value::<String>()
                .unwrap_or_default()
        };

        // Truncate to keep context window sane
        if text.len() > MAX_TEXT_LENGTH {
            Ok(format!("{}... [truncated, {} total chars]", &text[..MAX_TEXT_LENGTH], text.len()))
        } else {
            Ok(text)
        }
    })
    .await
    .map_err(|_| "Extract text timed out after 30 seconds".to_string())?
}

async fn browser_screenshot(page: &chromiumoxide::Page) -> Result<String, String> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let screenshot_bytes = page
            .screenshot(ScreenshotParams::builder().full_page(false).build())
            .await
            .map_err(|e| format!("Screenshot failed: {}", e))?;

        let b64 = base64::engine::general_purpose::STANDARD.encode(&screenshot_bytes);
        // Use the SCREENSHOT_BASE64: prefix to reuse the existing vision pipeline in agent.rs
        Ok(format!("SCREENSHOT_BASE64:{}", b64))
    })
    .await
    .map_err(|_| "Screenshot timed out after 30 seconds".to_string())?
}

async fn browser_get_page_state(page: &chromiumoxide::Page) -> Result<String, String> {
    tokio::time::timeout(Duration::from_secs(30), async {
        // Wait for page to be ready before extracting state
        for _ in 0..12 {
            let ready: String = page
                .evaluate("document.readyState")
                .await
                .ok()
                .and_then(|v| v.into_value().ok())
                .unwrap_or_default();
            if ready == "complete" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let current_url = page.url().await
            .map_err(|e| format!("Failed to get URL: {}", e))?
            .unwrap_or_default();
        let title = page.get_title().await
            .map_err(|e| format!("Failed to get title: {}", e))?
            .unwrap_or_default();

        // Primary: Use accessibility tree for element discovery (stable, cross-shadow-DOM)
        let ax_elements = get_interactive_elements(page).await.unwrap_or_default();

        let elements_json = if ax_elements.is_empty() {
            // Fallback to JS-based discovery if AX tree fails
            let elements_js = actionable_elements_js();
            page.evaluate(elements_js)
                .await
                .ok()
                .and_then(|v| v.into_value().ok())
                .unwrap_or(Value::Array(vec![]))
        } else {
            elements_to_json(&ax_elements)
        };

        // Cache elements for browser_click/browser_type_text SoM lookups
        {
            let mut cached = CACHED_ELEMENTS.lock().await;
            *cached = ax_elements.clone();
        }

        // Inject Set-of-Marks visual labels before screenshot
        let som_injected = inject_som_markers(page, &ax_elements).await.is_ok();

        // Take screenshot (with SoM markers visible)
        let screenshot_bytes = page
            .screenshot(ScreenshotParams::builder().full_page(false).build())
            .await
            .map_err(|e| format!("Page state screenshot failed: {}", e))?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&screenshot_bytes);

        // Remove SoM markers after screenshot
        if som_injected {
            let _ = remove_som_markers(page).await;
        }

        let state = serde_json::json!({
            "url": current_url,
            "title": title,
            "elements": elements_json,
            "element_count": ax_elements.len(),
            "hint": "Use element index numbers (e.g. browser_click(selector=\"7\")) to interact with elements. Numbers are shown as orange labels in the screenshot.",
        });

        // Return JSON state + screenshot (separated by marker so agent.rs can detect it)
        Ok(format!(
            "{}\nSCREENSHOT_BASE64:{}",
            serde_json::to_string_pretty(&state).unwrap_or_default(),
            b64
        ))
    })
    .await
    .map_err(|_| "Get page state timed out after 30 seconds".to_string())?
}

async fn browser_wait_for(
    page: &chromiumoxide::Page,
    selector: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let timeout = Duration::from_millis(timeout_ms);
    let poll_interval = Duration::from_millis(250);
    let start = std::time::Instant::now();

    loop {
        if page.find_element(selector).await.is_ok() {
            return Ok(format!("Element found: {}", selector));
        }
        if start.elapsed() >= timeout {
            return Err(format!(
                "Timed out after {}ms waiting for: {}",
                timeout_ms, selector
            ));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

async fn browser_go_back(page: &chromiumoxide::Page) -> Result<String, String> {
    page.evaluate("window.history.back()")
        .await
        .map_err(|e| format!("Go back failed: {}", e))?;

    let _ = wait::wait_for_stable(page, 3000).await;

    let url = page.url().await
        .map_err(|e| format!("Failed to get URL after going back: {}", e))?
        .unwrap_or_default();

    Ok(format!("Navigated back to: {}", url))
}

async fn browser_evaluate(
    page: &chromiumoxide::Page,
    expression: &str,
) -> Result<String, String> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let result: Value = page
            .evaluate(expression)
            .await
            .map_err(|e| format!("JS evaluation failed: {}", e))?
            .into_value()
            .unwrap_or(Value::Null);

        let output = match result {
            Value::String(s) => s,
            Value::Null => "null".to_string(),
            other => serde_json::to_string_pretty(&other).unwrap_or_else(|_| format!("{:?}", other)),
        };

        if output.len() > MAX_TEXT_LENGTH {
            Ok(format!("{}... [truncated]", &output[..MAX_TEXT_LENGTH]))
        } else {
            Ok(output)
        }
    })
    .await
    .map_err(|_| "JS evaluation timed out after 30 seconds".to_string())?
}

fn actionable_elements_js() -> &'static str {
    r#"
        (() => {
            const selectorParts = [
                'a',
                'button',
                'input',
                'select',
                'textarea',
                '[role]',
                '[onclick]',
                '[tabindex]',
                '[contenteditable="true"]',
                '[aria-label]',
                '[data-testid]',
                '[class*="btn"]',
                '[class*="button"]',
                '[class*="clickable"]',
                '[class*="chat"]',
                '[class*="item"]',
            ];
            const seen = new Set();
            const candidates = [];

            function cssPath(el) {
                if (!(el instanceof Element)) return null;
                if (el.id) return `#${CSS.escape(el.id)}`;
                const parts = [];
                let node = el;
                while (node && node.nodeType === Node.ELEMENT_NODE && parts.length < 4) {
                    let part = node.tagName.toLowerCase();
                    if (node.classList && node.classList.length) {
                        part += '.' + Array.from(node.classList).slice(0, 2).map(cls => CSS.escape(cls)).join('.');
                    }
                    const parent = node.parentElement;
                    if (parent) {
                        const siblings = Array.from(parent.children).filter(child => child.tagName === node.tagName);
                        if (siblings.length > 1) {
                            part += `:nth-of-type(${siblings.indexOf(node) + 1})`;
                        }
                    }
                    parts.unshift(part);
                    node = parent;
                }
                return parts.join(' > ');
            }

            function isActionable(el) {
                if (!(el instanceof HTMLElement)) return false;
                const rect = el.getBoundingClientRect();
                if (rect.width < 4 || rect.height < 4) return false;
                const style = window.getComputedStyle(el);
                if (style.visibility === 'hidden' || style.display === 'none') return false;
                return true;
            }

            for (const selector of selectorParts) {
                for (const el of document.querySelectorAll(selector)) {
                    if (!isActionable(el)) continue;
                    if (seen.has(el)) continue;
                    seen.add(el);
                    candidates.push(el);
                }
            }

            return candidates.slice(0, 150).map(el => {
                const rect = el.getBoundingClientRect();
                const text = (el.innerText || el.textContent || '').trim().replace(/\s+/g, ' ').slice(0, 120);
                const label = el.getAttribute('aria-label') || el.getAttribute('title') || el.getAttribute('placeholder') || null;
                const pointer = window.getComputedStyle(el).cursor === 'pointer';
                const role = el.getAttribute('role');
                return {
                    tag: el.tagName.toLowerCase(),
                    text: text || null,
                    id: el.id || null,
                    name: el.getAttribute('name'),
                    placeholder: el.getAttribute('placeholder'),
                    aria_label: el.getAttribute('aria-label'),
                    title: el.getAttribute('title'),
                    href: el.getAttribute('href'),
                    type: el.getAttribute('type'),
                    role,
                    contenteditable: el.isContentEditable,
                    pointer,
                    selector: cssPath(el),
                    bounds: {
                        x: Math.round(rect.x),
                        y: Math.round(rect.y),
                        width: Math.round(rect.width),
                        height: Math.round(rect.height),
                    },
                    telegram_hint: (role && role.includes('option')) || /chat|dialog|search/i.test(`${text} ${label || ''}`),
                };
            });
        })()
    "#
}

async fn capture_page_snapshot(page: &chromiumoxide::Page) -> Result<Value, String> {
    page.evaluate(page_snapshot_js())
        .await
        .map_err(|e| format!("Failed to capture page snapshot: {}", e))?
        .into_value()
        .map_err(|e| format!("Failed to parse page snapshot: {}", e))
}

fn page_snapshot_js() -> &'static str {
    r#"
        (() => {
            function textOf(el) {
                if (!el) return null;
                return (el.innerText || el.textContent || '').trim().replace(/\s+/g, ' ').slice(0, 160) || null;
            }

            function attrs(el) {
                if (!el) return null;
                return {
                    tag: el.tagName ? el.tagName.toLowerCase() : null,
                    id: el.id || null,
                    className: typeof el.className === 'string' ? el.className.slice(0, 200) : null,
                    role: el.getAttribute ? el.getAttribute('role') : null,
                    ariaLabel: el.getAttribute ? el.getAttribute('aria-label') : null,
                    title: el.getAttribute ? el.getAttribute('title') : null,
                    placeholder: el.getAttribute ? el.getAttribute('placeholder') : null,
                    value: 'value' in el ? String(el.value || '').slice(0, 500) : null,
                    text: textOf(el),
                    ariaExpanded: el.getAttribute ? el.getAttribute('aria-expanded') : null,
                    ariaChecked: el.getAttribute ? el.getAttribute('aria-checked') : null,
                    ariaSelected: el.getAttribute ? el.getAttribute('aria-selected') : null,
                    ariaPressed: el.getAttribute ? el.getAttribute('aria-pressed') : null,
                };
            }

            const active = document.activeElement;
            const activeAttrs = attrs(active);

            return {
                url: location.href,
                title: document.title,
                activeElement: activeAttrs,
                selectedText: window.getSelection ? String(window.getSelection()).slice(0, 200) : null,
                elementCount: document.querySelectorAll('*').length,
            };
        })()
    "#
}

async fn resolve_target(page: &chromiumoxide::Page, selector: &str) -> Result<Value, String> {
    let safe_selector = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());
    let js = format!(
        r#"(() => {{
            function textOf(el) {{
                return (el?.innerText || el?.textContent || '').trim().replace(/\s+/g, ' ');
            }}

            function summarize(el, strategy) {{
                if (!el) return null;
                el.scrollIntoView({{ block: 'center', inline: 'center', behavior: 'instant' }});
                const rect = el.getBoundingClientRect();
                return {{
                    ok: true,
                    strategy,
                    x: rect.left + (rect.width / 2),
                    y: rect.top + (rect.height / 2),
                    text: textOf(el).slice(0, 160) || null,
                    tag: el.tagName?.toLowerCase() || null,
                    role: el.getAttribute?.('role') || null,
                    ariaLabel: el.getAttribute?.('aria-label') || null,
                    title: el.getAttribute?.('title') || null,
                    className: typeof el.className === 'string' ? el.className.slice(0, 200) : null,
                    selected: el.getAttribute?.('aria-selected') === 'true' || el.classList?.contains('active') || el.classList?.contains('selected'),
                }};
            }}

            const rawSelector = {safe_selector}.trim();
            if (!rawSelector) return {{ ok: false, error: 'Empty selector' }};

            try {{
                const css = document.querySelector(rawSelector);
                if (css) return summarize(css, 'css');
            }} catch (_) {{}}

            if (rawSelector.startsWith('//') || rawSelector.startsWith('(')) {{
                try {{
                    const xp = document.evaluate(rawSelector, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue;
                    if (xp) return summarize(xp, 'xpath');
                }} catch (_) {{}}
            }}

            let textQuery = rawSelector;
            const contains = rawSelector.match(/:contains\(['"]?(.*?)['"]?\)/i);
            const hasText = rawSelector.match(/:has-text\(['"]?(.*?)['"]?\)/i);
            const textEq = rawSelector.match(/^text=['"]?(.*?)['"]?$/i);
            if (contains) textQuery = contains[1];
            else if (hasText) textQuery = hasText[1];
            else if (textEq) textQuery = textEq[1];

            const q = textQuery.trim().toLowerCase();
            if (!q) return {{ ok: false, error: 'Empty text query' }};

            const all = Array.from(document.querySelectorAll('*'));
            const candidates = all.filter(el => {{
                const rect = el.getBoundingClientRect();
                if (rect.width < 4 || rect.height < 4) return false;
                const style = window.getComputedStyle(el);
                if (style.visibility === 'hidden' || style.display === 'none') return false;
                const text = textOf(el).toLowerCase();
                if (!text || text.length > 200) return false;
                return text.includes(q);
            }});

            const ranked = candidates
                .map(el => {{
                    const text = textOf(el).toLowerCase();
                    const exact = text === q;
                    const pointer = window.getComputedStyle(el).cursor === 'pointer';
                    const role = el.getAttribute('role') || '';
                    const score = (exact ? 100 : 0) + (pointer ? 20 : 0) + (/button|option|tab|link|menuitem|listitem/.test(role) ? 15 : 0);
                    return {{ el, score }};
                }})
                .sort((a, b) => b.score - a.score);

            if (!ranked.length) return {{ ok: false, error: `No visible element matched "${{rawSelector}}"` }};
            return summarize(ranked[0].el, 'text');
        }})()"#
    );

    let result: Value = page
        .evaluate(js)
        .await
        .map_err(|e| format!("Target resolution failed for '{}': {}", selector, e))?
        .into_value()
        .map_err(|e| format!("Failed to parse target resolution for '{}': {}", selector, e))?;

    if result["ok"].as_bool().unwrap_or(false) {
        Ok(result)
    } else {
        Err(result["error"]
            .as_str()
            .unwrap_or("Failed to resolve target")
            .to_string())
    }
}

struct VerificationResult {
    success: bool,
    message: String,
}

/// Generic click verification: checks multiple signals for ANY state change.
fn verify_click_outcome_generic(before: &Value, after: &Value, mutation_count: u64) -> VerificationResult {
    // 1. URL changed (navigation)
    let before_url = before["url"].as_str().unwrap_or_default();
    let after_url = after["url"].as_str().unwrap_or_default();
    if before_url != after_url {
        return VerificationResult {
            success: true,
            message: format!("Navigated to {}", after_url),
        };
    }

    // 2. Active element changed
    let before_active = before["activeElement"]["text"].as_str().unwrap_or_default();
    let after_active = after["activeElement"]["text"].as_str().unwrap_or_default();
    let before_active_tag = before["activeElement"]["tag"].as_str().unwrap_or_default();
    let after_active_tag = after["activeElement"]["tag"].as_str().unwrap_or_default();
    if before_active != after_active || before_active_tag != after_active_tag {
        return VerificationResult {
            success: true,
            message: format!("Focus changed to {}", summarize_active(after)),
        };
    }

    // 3. ARIA state changed (expanded, checked, selected, pressed)
    for attr in &["ariaExpanded", "ariaChecked", "ariaSelected", "ariaPressed"] {
        let before_val = before["activeElement"][attr].as_str().unwrap_or_default();
        let after_val = after["activeElement"][attr].as_str().unwrap_or_default();
        if before_val != after_val {
            return VerificationResult {
                success: true,
                message: format!("{} changed from '{}' to '{}'", attr, before_val, after_val),
            };
        }
    }

    // 4. Significant DOM mutations occurred
    if mutation_count > 3 {
        return VerificationResult {
            success: true,
            message: format!("Page updated ({} DOM mutations detected)", mutation_count),
        };
    }

    // 5. Element count changed (new elements appeared — modal, dropdown, etc.)
    let before_count = before["elementCount"].as_u64().unwrap_or(0);
    let after_count = after["elementCount"].as_u64().unwrap_or(0);
    if before_count != after_count && (after_count as i64 - before_count as i64).unsigned_abs() > 2 {
        return VerificationResult {
            success: true,
            message: format!("DOM changed ({} → {} elements)", before_count, after_count),
        };
    }

    VerificationResult {
        success: false,
        message: format!(
            "No visible state change detected. Active: {}. Mutations: {}.",
            summarize_active(after),
            mutation_count
        ),
    }
}

/// Generic type verification: checks if text appeared in the focused field.
fn verify_type_outcome_generic(text: &str, before: &Value, after: &Value) -> VerificationResult {
    let after_value = after["activeElement"]["value"].as_str().unwrap_or_default();
    let after_text = after["activeElement"]["text"].as_str().unwrap_or_default();
    if after_value == text || after_text == text || after_value.contains(text) || after_text.contains(text) {
        return VerificationResult {
            success: true,
            message: format!("Field now contains {}", truncate_for_message(text)),
        };
    }

    // Check if active element changed at all (some fields don't expose value easily)
    let before_value = before["activeElement"]["value"].as_str().unwrap_or_default();
    let before_text = before["activeElement"]["text"].as_str().unwrap_or_default();
    if (after_value != before_value && !after_value.is_empty())
        || (after_text != before_text && !after_text.is_empty())
    {
        return VerificationResult {
            success: true,
            message: format!("Field content changed to {}", truncate_for_message(if !after_value.is_empty() { after_value } else { after_text })),
        };
    }

    VerificationResult {
        success: false,
        message: format!(
            "Expected '{}' to appear in focused field. Value after: '{}', text after: '{}'.",
            truncate_for_message(text),
            truncate_for_message(after_value),
            truncate_for_message(after_text),
        ),
    }
}

/// Inject a mutation counter that tracks DOM changes.
async fn inject_mutation_counter(page: &chromiumoxide::Page) -> u64 {
    let js = r#"
        (() => {
            window.__imprint_mutation_count = 0;
            if (window.__imprint_click_observer) {
                window.__imprint_click_observer.disconnect();
            }
            window.__imprint_click_observer = new MutationObserver((mutations) => {
                window.__imprint_mutation_count += mutations.length;
            });
            window.__imprint_click_observer.observe(document.body || document.documentElement, {
                childList: true, subtree: true, attributes: true,
            });
            return 0;
        })()
    "#;
    page.evaluate(js)
        .await
        .ok()
        .and_then(|v| v.into_value().ok())
        .unwrap_or(0)
}

/// Read the mutation count and clean up the observer.
async fn read_mutation_count(page: &chromiumoxide::Page) -> u64 {
    let js = r#"
        (() => {
            const count = window.__imprint_mutation_count || 0;
            if (window.__imprint_click_observer) {
                window.__imprint_click_observer.disconnect();
                window.__imprint_click_observer = null;
            }
            return count;
        })()
    "#;
    page.evaluate(js)
        .await
        .ok()
        .and_then(|v| v.into_value().ok())
        .unwrap_or(0)
}

fn summarize_active(snapshot: &Value) -> String {
    let active = &snapshot["activeElement"];
    let tag = active["tag"].as_str().unwrap_or("unknown");
    let text = active["text"].as_str().unwrap_or_default();
    let label = active["ariaLabel"].as_str().unwrap_or_default();
    let value = active["value"].as_str().unwrap_or_default();
    let summary = if !value.is_empty() {
        value
    } else if !text.is_empty() {
        text
    } else {
        label
    };
    if summary.is_empty() {
        tag.to_string()
    } else {
        format!("{}({})", tag, truncate_for_message(summary))
    }
}

fn truncate_for_message(input: &str) -> String {
    if input.len() > 80 {
        format!("{}...", &input[..77])
    } else {
        input.to_string()
    }
}

fn describe_target(target: &Value) -> String {
    let tag = target["tag"].as_str().unwrap_or("element");
    let text = target["text"].as_str().unwrap_or_default();
    let title = target["title"].as_str().unwrap_or_default();
    let aria = target["ariaLabel"].as_str().unwrap_or_default();
    let detail = [text, aria, title]
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or("");
    if detail.is_empty() {
        tag.to_string()
    } else {
        format!("{}({})", tag, truncate_for_message(detail))
    }
}
