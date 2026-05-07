use chromiumoxide::Page;
use std::time::{Duration, Instant};

/// JavaScript that installs MutationObserver + network idle tracker.
/// Sets `window.__imprint_dom_settled` and `window.__imprint_network_idle`.
const INJECT_SETTLE_OBSERVERS_JS: &str = r#"
(() => {
    // Reset flags
    window.__imprint_dom_settled = false;
    window.__imprint_network_idle = false;

    // --- DOM mutation settle ---
    if (window.__imprint_mutation_observer) {
        window.__imprint_mutation_observer.disconnect();
    }
    let domTimer = null;
    window.__imprint_mutation_observer = new MutationObserver(() => {
        window.__imprint_dom_settled = false;
        clearTimeout(domTimer);
        domTimer = setTimeout(() => { window.__imprint_dom_settled = true; }, 200);
    });
    window.__imprint_mutation_observer.observe(document.body || document.documentElement, {
        childList: true,
        subtree: true,
        attributes: true,
        characterData: true,
    });
    // If no mutations at all within 200ms, mark settled
    domTimer = setTimeout(() => { window.__imprint_dom_settled = true; }, 200);

    // --- Network idle ---
    let netTimer = null;
    function resetNetTimer() {
        window.__imprint_network_idle = false;
        clearTimeout(netTimer);
        netTimer = setTimeout(() => { window.__imprint_network_idle = true; }, 300);
    }
    resetNetTimer();

    try {
        if (window.__imprint_perf_observer) {
            window.__imprint_perf_observer.disconnect();
        }
        window.__imprint_perf_observer = new PerformanceObserver((list) => {
            if (list.getEntries().length > 0) {
                resetNetTimer();
            }
        });
        window.__imprint_perf_observer.observe({ type: 'resource', buffered: false });
    } catch (_) {
        // PerformanceObserver not available — mark idle immediately
        window.__imprint_network_idle = true;
    }
})()
"#;

const CHECK_SETTLED_JS: &str =
    "window.__imprint_dom_settled === true && window.__imprint_network_idle === true";

/// Clean up observers to avoid leaks.
const CLEANUP_OBSERVERS_JS: &str = r#"
(() => {
    if (window.__imprint_mutation_observer) {
        window.__imprint_mutation_observer.disconnect();
        window.__imprint_mutation_observer = null;
    }
    if (window.__imprint_perf_observer) {
        window.__imprint_perf_observer.disconnect();
        window.__imprint_perf_observer = null;
    }
})()
"#;

/// Wait for the page to stabilize after an action.
///
/// Injects a MutationObserver and PerformanceObserver, then polls until both
/// report settled OR the hard timeout is reached.
///
/// - `max_ms`: hard ceiling in milliseconds (e.g. 5000 for navigation, 2000 for clicks)
/// - Minimum wait: 150ms (gives DOM a chance to start reacting)
pub async fn wait_for_stable(page: &Page, max_ms: u64) -> Result<(), String> {
    // Inject observers
    let _ = page.evaluate(INJECT_SETTLE_OBSERVERS_JS).await;

    let start = Instant::now();
    let min_wait = Duration::from_millis(150);
    let max_wait = Duration::from_millis(max_ms);
    let poll_interval = Duration::from_millis(100);

    loop {
        let elapsed = start.elapsed();

        if elapsed >= max_wait {
            break;
        }

        // Don't check too early — give DOM time to start mutating
        if elapsed >= min_wait {
            let settled: bool = page
                .evaluate(CHECK_SETTLED_JS)
                .await
                .ok()
                .and_then(|v| v.into_value().ok())
                .unwrap_or(false);

            if settled {
                break;
            }
        }

        tokio::time::sleep(poll_interval).await;
    }

    // Clean up
    let _ = page.evaluate(CLEANUP_OBSERVERS_JS).await;
    Ok(())
}
