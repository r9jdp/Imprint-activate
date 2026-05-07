use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::accessibility::{
    self as ax, AxNode, AxPropertyName,
};
use chromiumoxide::cdp::browser_protocol::dom::{
    BackendNodeId, GetBoxModelParams, ScrollIntoViewIfNeededParams,
};
use serde::{Deserialize, Serialize};

/// An interactive element discovered via the accessibility tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    /// 1-based index for Set-of-Marks labeling
    pub index: usize,
    /// Accessibility node ID (session-scoped)
    pub ax_node_id: String,
    /// Backend DOM node ID for CDP commands (getBoxModel, scrollIntoView, etc.)
    pub backend_node_id: Option<i64>,
    /// ARIA role (e.g. "button", "link", "textbox")
    pub role: String,
    /// Browser-computed accessible name
    pub name: String,
    /// Accessible description
    pub description: String,
    /// Bounding rect in viewport coordinates
    pub bounds: Option<Rect>,
    /// Frame ID for cross-frame elements
    pub frame_id: Option<String>,
    /// Whether the element can receive focus
    pub is_focusable: bool,
    /// Whether the element is editable (textbox, combobox, etc.)
    pub is_editable: bool,
    /// Current value (for inputs, sliders, etc.)
    pub value: Option<String>,
    /// Whether the element is in the current viewport
    pub viewport_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn center_x(&self) -> f64 {
        self.x + self.width / 2.0
    }
    pub fn center_y(&self) -> f64 {
        self.y + self.height / 2.0
    }
}

/// Roles considered interactive for element discovery.
const INTERACTIVE_ROLES: &[&str] = &[
    "button",
    "link",
    "textbox",
    "combobox",
    "listbox",
    "checkbox",
    "radio",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "tab",
    "switch",
    "slider",
    "spinbutton",
    "searchbox",
    "option",
    "treeitem",
    "cell",
    "row",
    "gridcell",
    "columnheader",
    "rowheader",
];

/// Extract the string value from an AX node's AxValue field.
fn ax_value_str(val: &Option<ax::AxValue>) -> String {
    match val {
        Some(v) => {
            if let Some(ref json_val) = v.value {
                if let Some(s) = json_val.as_str() {
                    return s.to_string();
                }
                return json_val.to_string();
            }
            String::new()
        }
        None => String::new(),
    }
}

/// Check if an AX node has a specific boolean property set to true.
fn has_ax_property(node: &AxNode, prop_name: AxPropertyName) -> bool {
    if let Some(ref props) = node.properties {
        for p in props {
            if p.name == prop_name {
                if let Some(ref json_val) = p.value.value {
                    return json_val.as_bool().unwrap_or(false);
                }
            }
        }
    }
    false
}

/// Get viewport dimensions via JS.
async fn get_viewport_size(page: &Page) -> (f64, f64) {
    let result: serde_json::Value = page
        .evaluate("[window.innerWidth, window.innerHeight]")
        .await
        .ok()
        .and_then(|v| v.into_value().ok())
        .unwrap_or(serde_json::json!([1280, 720]));

    let w = result[0].as_f64().unwrap_or(1280.0);
    let h = result[1].as_f64().unwrap_or(720.0);
    (w, h)
}

/// Get bounding box for a backend node ID using DOM.getBoxModel.
/// Returns None if the element has no layout (hidden, zero-size, etc.).
pub async fn get_element_bounds(page: &Page, backend_node_id: BackendNodeId) -> Option<Rect> {
    let params = GetBoxModelParams::builder()
        .backend_node_id(backend_node_id)
        .build();

    let resp = page.execute(params).await.ok()?;
    let model = resp.result.model;

    // content quad: [x1,y1, x2,y2, x3,y3, x4,y4]
    let content = model.content.inner();
    if content.len() < 8 {
        return None;
    }

    let x = content[0];
    let y = content[1];
    let width = content[2] - content[0];
    let height = content[5] - content[1];

    // Filter out zero-size or negative-size elements
    if width < 2.0 || height < 2.0 {
        return None;
    }

    Some(Rect {
        x,
        y,
        width,
        height,
    })
}

/// Discover all interactive elements on the page using the accessibility tree.
///
/// This is the primary element discovery method — far more stable than CSS selectors
/// because accessible names are computed by the browser across shadow DOM, labels, aria-*, etc.
pub async fn get_interactive_elements(page: &Page) -> Result<Vec<InteractiveElement>, String> {
    // Enable accessibility domain
    let enable_params = ax::EnableParams::default();
    let _ = page.execute(enable_params).await;

    let (vp_width, vp_height) = get_viewport_size(page).await;

    // Fetch the full accessibility tree
    let tree_params = ax::GetFullAxTreeParams::builder().build();

    let tree_result = page
        .execute(tree_params)
        .await
        .map_err(|e| format!("Failed to get accessibility tree: {}", e))?;

    let nodes = tree_result.result.nodes;

    let mut elements: Vec<InteractiveElement> = Vec::new();
    let mut index = 1usize;

    for node in &nodes {
        // Skip ignored nodes
        if node.ignored {
            continue;
        }

        let role = ax_value_str(&node.role);
        let name = ax_value_str(&node.name);

        // Determine if this node is interactive
        let is_interactive_role = INTERACTIVE_ROLES.iter().any(|r| role == *r);
        let is_focusable = has_ax_property(node, AxPropertyName::Focusable);
        let is_editable = has_ax_property(node, AxPropertyName::Editable);

        // Keep if it has an interactive role, or is focusable with a name, or is editable
        if !is_interactive_role && !is_editable && !(is_focusable && !name.is_empty()) {
            continue;
        }

        // Skip generic nodes without a name (noise)
        if role == "generic" && name.is_empty() && !is_editable {
            continue;
        }

        // Get bounds if we have a backend DOM node
        let backend_bid: Option<BackendNodeId> = node.backend_dom_node_id.clone();
        let bounds = match &backend_bid {
            Some(bid) => {
                let bid_clone: BackendNodeId = bid.clone();
                get_element_bounds(page, bid_clone).await
            }
            None => None,
        };

        // Determine viewport visibility
        let viewport_visible = bounds.as_ref().map_or(false, |b| {
            b.x + b.width > 0.0
                && b.y + b.height > 0.0
                && b.x < vp_width
                && b.y < vp_height
        });

        let value = ax_value_str(&node.value);
        let description = ax_value_str(&node.description);

        let bid_i64: Option<i64> = backend_bid.as_ref().map(|id: &BackendNodeId| *id.inner());
        let fid_str: Option<String> = node.frame_id.as_ref().map(|f: &chromiumoxide::cdp::browser_protocol::page::FrameId| f.inner().to_string());

        elements.push(InteractiveElement {
            index,
            ax_node_id: node.node_id.inner().to_string(),
            backend_node_id: bid_i64,
            role: role.clone(),
            name: name.clone(),
            description,
            bounds,
            frame_id: fid_str,
            is_focusable,
            is_editable,
            value: if value.is_empty() { None } else { Some(value) },
            viewport_visible,
        });

        index += 1;
    }

    // Sort: viewport-visible first, then by position (top-to-bottom, left-to-right)
    elements.sort_by(|a, b| {
        b.viewport_visible
            .cmp(&a.viewport_visible)
            .then_with(|| {
                let ay = a.bounds.as_ref().map(|r| r.y).unwrap_or(f64::MAX);
                let by = b.bounds.as_ref().map(|r| r.y).unwrap_or(f64::MAX);
                ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                let ax = a.bounds.as_ref().map(|r| r.x).unwrap_or(f64::MAX);
                let bx = b.bounds.as_ref().map(|r| r.x).unwrap_or(f64::MAX);
                ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    // Re-assign indices after sorting
    for (i, el) in elements.iter_mut().enumerate() {
        el.index = i + 1;
    }

    // Cap at 300 elements
    elements.truncate(300);

    Ok(elements)
}

/// Find an element by query: integer index, text match, or CSS selector fallback.
///
/// Returns the matching InteractiveElement, or an error if not found.
pub fn find_element_by_query<'a>(
    query: &str,
    cached_elements: &'a [InteractiveElement],
) -> Result<&'a InteractiveElement, String> {
    let trimmed = query.trim();

    // Try as integer index first (Set-of-Marks reference)
    if let Ok(idx) = trimmed.parse::<usize>() {
        return cached_elements
            .iter()
            .find(|e| e.index == idx)
            .ok_or_else(|| {
                format!(
                    "Element index {} not found. Available: 1-{}",
                    idx,
                    cached_elements.len()
                )
            });
    }

    // Try fuzzy text match against accessible names
    let query_lower = trimmed.to_lowercase();

    // First pass: exact name match
    if let Some(el) = cached_elements
        .iter()
        .find(|e| e.name.to_lowercase() == query_lower)
    {
        return Ok(el);
    }

    // Second pass: name contains query
    let mut candidates: Vec<(&InteractiveElement, usize)> = cached_elements
        .iter()
        .filter_map(|e| {
            let name_lower = e.name.to_lowercase();
            if name_lower.contains(&query_lower) {
                // Score: shorter name = better match (more specific)
                let score = 1000 - name_lower.len().min(999);
                Some((e, score))
            } else {
                None
            }
        })
        .collect();

    // Boost viewport-visible and interactive-role elements
    for (el, score) in &mut candidates {
        if el.viewport_visible {
            *score += 200;
        }
        match el.role.as_str() {
            "button" | "link" | "tab" => *score += 100,
            "menuitem" | "option" | "treeitem" => *score += 50,
            _ => {}
        }
    }

    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    if let Some((el, _)) = candidates.first() {
        return Ok(el);
    }

    // Third pass: match against role
    if let Some(el) = cached_elements
        .iter()
        .find(|e| e.role.to_lowercase() == query_lower)
    {
        return Ok(el);
    }

    Err(format!(
        "No element matched '{}'. Use browser_get_page_state to see available elements.",
        trimmed
    ))
}

/// Inject Set-of-Marks (SoM) numbered labels onto the page.
///
/// Each interactive element gets a small orange label with its index number,
/// positioned at the top-left corner of the element.
pub async fn inject_som_markers(
    page: &Page,
    elements: &[InteractiveElement],
) -> Result<(), String> {
    // Build the marker injection script
    let markers: Vec<serde_json::Value> = elements
        .iter()
        .filter(|e| e.viewport_visible && e.bounds.is_some())
        .map(|e| {
            let b = e.bounds.as_ref().unwrap();
            serde_json::json!({
                "index": e.index,
                "x": b.x,
                "y": b.y,
                "width": b.width,
                "height": b.height,
            })
        })
        .collect();

    let markers_json =
        serde_json::to_string(&markers).unwrap_or_else(|_| "[]".to_string());

    let js = format!(
        r#"
        (() => {{
            // Remove any existing markers
            document.querySelectorAll('[data-imprint-som]').forEach(el => el.remove());

            const elements = {markers_json};
            elements.forEach(({{ index, x, y, width, height }}) => {{
                const marker = document.createElement('div');
                marker.dataset.imprintSom = String(index);
                marker.textContent = String(index);
                marker.style.cssText =
                    'position:fixed;z-index:2147483647;' +
                    'left:' + x + 'px;top:' + Math.max(0, y - 16) + 'px;' +
                    'background:#e65100;color:white;' +
                    'font:bold 10px/13px monospace;' +
                    'padding:1px 3px;border-radius:2px;' +
                    'pointer-events:none;min-width:14px;text-align:center;' +
                    'box-shadow:0 1px 3px rgba(0,0,0,0.4);';
                document.body.appendChild(marker);
            }});
        }})()
        "#
    );

    page.evaluate(js)
        .await
        .map_err(|e| format!("Failed to inject SoM markers: {}", e))?;

    Ok(())
}

/// Remove all Set-of-Marks labels from the page.
pub async fn remove_som_markers(page: &Page) -> Result<(), String> {
    page.evaluate(
        "document.querySelectorAll('[data-imprint-som]').forEach(el => el.remove())",
    )
    .await
    .map_err(|e| format!("Failed to remove SoM markers: {}", e))?;
    Ok(())
}

/// Scroll an element into view using CDP (more reliable than JS scrollIntoView).
pub async fn scroll_into_view(page: &Page, backend_node_id: i64) -> Result<(), String> {
    let params = ScrollIntoViewIfNeededParams::builder()
        .backend_node_id(BackendNodeId::new(backend_node_id))
        .build();

    page.execute(params)
        .await
        .map_err(|e| format!("ScrollIntoViewIfNeeded failed: {}", e))?;

    Ok(())
}

/// Check if a point is occluded by another element using elementFromPoint.
/// Returns true if the element at (x, y) matches the intended backend_node_id,
/// or if we can't determine occlusion (benefit of the doubt).
pub async fn check_occlusion(page: &Page, x: f64, y: f64) -> bool {
    let js = format!(
        r#"
        (() => {{
            const el = document.elementFromPoint({x}, {y});
            if (!el) return {{ occluded: false }};
            // Check if the element or its ancestors have high z-index overlays
            const style = window.getComputedStyle(el);
            const isOverlay = style.position === 'fixed' &&
                (el.classList.toString().match(/overlay|modal|backdrop|cookie|banner|popup/i) ||
                 style.zIndex > 999);
            return {{ occluded: isOverlay, tag: el.tagName, className: el.className?.toString()?.slice(0, 100) }};
        }})()
        "#
    );

    let result: serde_json::Value = page
        .evaluate(js)
        .await
        .ok()
        .and_then(|v| v.into_value().ok())
        .unwrap_or(serde_json::json!({"occluded": false}));

    result["occluded"].as_bool().unwrap_or(false)
}

/// Serialize elements to a compact JSON representation for the LLM.
pub fn elements_to_json(elements: &[InteractiveElement]) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = elements
        .iter()
        .map(|e| {
            let mut obj = serde_json::json!({
                "index": e.index,
                "role": e.role,
                "name": e.name,
            });
            if !e.description.is_empty() {
                obj["description"] = serde_json::json!(e.description);
            }
            if let Some(ref v) = e.value {
                obj["value"] = serde_json::json!(v);
            }
            if let Some(ref b) = e.bounds {
                obj["bounds"] = serde_json::json!({
                    "x": (b.x as i32),
                    "y": (b.y as i32),
                    "w": (b.width as i32),
                    "h": (b.height as i32),
                });
            }
            if e.is_editable {
                obj["editable"] = serde_json::json!(true);
            }
            if e.viewport_visible {
                obj["visible"] = serde_json::json!(true);
            }
            obj
        })
        .collect();
    serde_json::Value::Array(arr)
}
