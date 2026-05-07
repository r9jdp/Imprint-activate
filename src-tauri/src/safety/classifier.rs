use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// Represents a concrete agent operation, mapped 1:1 from tool calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentOperation {
    ReadFile(PathBuf),
    WriteFile { path: PathBuf, content: String },
    CreateFile { path: PathBuf, content: String },
    DeleteFile(PathBuf),
    DeleteDir(PathBuf),
    MoveFile { from: PathBuf, to: PathBuf },
    CopyFile { from: PathBuf, to: PathBuf },
    CreateDir(PathBuf),
    RunCommand { cmd: String, args: Vec<String> },
    ReadDir(PathBuf),
    OpenPath(PathBuf),
    RunAppleScript(String),
    // Computer Use operations
    Screenshot,
    MouseMove { x: i32, y: i32 },
    MouseClick { button: String, click_type: String },
    KeyboardType { text: String },
    KeyboardHotkey { keys: Vec<String> },
    MouseScroll { direction: String, amount: i32 },
    ComputerUse { task: String, max_steps: i32, live_vision: bool },
    // Browser automation operations
    BrowserNavigate { url: String },
    BrowserClick { selector: String },
    BrowserTypeText { selector: String, text: String },
    BrowserScroll { direction: String, amount: f64 },
    BrowserExtractText { selector: Option<String> },
    BrowserScreenshot,
    BrowserGetPageState,
    BrowserWaitFor { selector: String },
    BrowserGoBack,
    BrowserEvaluate { expression: String },
}

/// Risk classification for operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RiskLevel {
    Safe,
    Caution,
    Dangerous,
    Nuclear,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Safe => write!(f, "safe"),
            RiskLevel::Caution => write!(f, "caution"),
            RiskLevel::Dangerous => write!(f, "dangerous"),
            RiskLevel::Nuclear => write!(f, "nuclear"),
        }
    }
}

/// Check if a command uses sudo or equivalent privilege escalation.
pub fn has_sudo(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    lower.starts_with("sudo ")
        || lower.contains(" sudo ")
        || lower.starts_with("doas ")
        || lower.contains(" doas ")
        || lower.starts_with("pkexec ")
}

/// Check if a path is a system directory that shouldn't be modified.
pub fn is_system_path(path: &std::path::Path) -> bool {
    let p = path.to_string_lossy();
    let p = p.as_ref();

    #[cfg(target_os = "macos")]
    {
        if p.starts_with("/System")
            || p.starts_with("/Library")
            || p.starts_with("/usr")
            || p.starts_with("/bin")
            || p.starts_with("/sbin")
            || p.starts_with("/etc")
            || p.starts_with("/var")
            || p.starts_with("/private/etc")
            || p.starts_with("/private/var")
        {
            return true;
        }
    }

    #[cfg(target_os = "windows")]
    {
        let lower = p.to_lowercase();
        if lower.starts_with("c:\\windows")
            || lower.starts_with("c:\\program files")
            || lower.starts_with("c:\\program files (x86)")
            || lower.starts_with("c:\\programdata")
        {
            return true;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if p.starts_with("/usr")
            || p.starts_with("/bin")
            || p.starts_with("/sbin")
            || p.starts_with("/etc")
            || p.starts_with("/var")
            || p.starts_with("/boot")
            || p.starts_with("/sys")
            || p.starts_with("/proc")
        {
            return true;
        }
    }

    false
}

/// Check if a command involves network activity.
fn is_network_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    lower.contains("curl ")
        || lower.contains("wget ")
        || lower.contains("ssh ")
        || lower.contains("scp ")
        || lower.contains("rsync ")
        || lower.contains("nc ")
        || lower.contains("netcat ")
        || lower.contains("nmap ")
}

/// Check if a command is destructive.
fn is_destructive_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    lower.contains("rm -rf")
        || lower.contains("rm -r ")
        || lower.contains("rm ")
        || lower.contains("rmdir")
        || lower.contains("mkfs")
        || lower.contains("dd if=")
        || lower.contains("format ")
        || lower.contains("> /dev/")
        || lower.contains("kill -9")
        || lower.contains("killall")
        || lower.contains("pkill")
        || lower.contains("shutdown")
        || lower.contains("reboot")
        || lower.contains("init 0")
        || lower.contains("init 6")
}

/// Classify the risk level of an operation.
pub fn classify_operation(op: &AgentOperation) -> RiskLevel {
    match op {
        // Read-only operations are safe
        AgentOperation::ReadFile(_) | AgentOperation::ReadDir(_) => RiskLevel::Safe,

        // Opening paths is safe (just shows in file manager)
        AgentOperation::OpenPath(_) => RiskLevel::Safe,

        // Creating directories is low risk
        AgentOperation::CreateDir(path) => {
            if is_system_path(path) {
                RiskLevel::Dangerous
            } else {
                RiskLevel::Safe
            }
        }

        // File creation/write is caution
        AgentOperation::WriteFile { path, .. } | AgentOperation::CreateFile { path, .. } => {
            if is_system_path(path) {
                RiskLevel::Dangerous
            } else {
                RiskLevel::Caution
            }
        }

        // Move/rename is caution, unless system paths
        AgentOperation::MoveFile { from, to } | AgentOperation::CopyFile { from, to } => {
            if is_system_path(from) || is_system_path(to) {
                RiskLevel::Dangerous
            } else {
                RiskLevel::Caution
            }
        }

        // Deletion is dangerous
        AgentOperation::DeleteFile(path) | AgentOperation::DeleteDir(path) => {
            if is_system_path(path) {
                RiskLevel::Nuclear
            } else {
                RiskLevel::Dangerous
            }
        }

        // Shell commands need deeper analysis
        AgentOperation::RunCommand { cmd, .. } => {
            if has_sudo(cmd) {
                return RiskLevel::Nuclear;
            }
            if is_destructive_command(cmd) {
                return RiskLevel::Dangerous;
            }
            if is_network_command(cmd) {
                return RiskLevel::Caution;
            }
            // Simple informational commands
            let lower = cmd.to_lowercase();
            if lower.starts_with("ls ")
                || lower.starts_with("echo ")
                || lower.starts_with("cat ")
                || lower.starts_with("head ")
                || lower.starts_with("tail ")
                || lower.starts_with("wc ")
                || lower.starts_with("pwd")
                || lower.starts_with("whoami")
                || lower.starts_with("date")
                || lower.starts_with("uname")
                || lower.starts_with("which ")
                || lower.starts_with("type ")
                || lower.starts_with("file ")
                || lower.starts_with("find ")
                || lower.starts_with("grep ")
                || lower.starts_with("rg ")
                || lower == "ls"
                || lower == "pwd"
            {
                return RiskLevel::Safe;
            }
            RiskLevel::Caution
        }

        // AppleScript can do anything — caution by default
        AgentOperation::RunAppleScript(script) => {
            let lower = script.to_lowercase();
            if lower.contains("do shell script") && has_sudo(&lower) {
                RiskLevel::Nuclear
            } else if lower.contains("delete") || lower.contains("do shell script") {
                RiskLevel::Dangerous
            } else {
                RiskLevel::Caution
            }
        }

        // Computer Use operations
        AgentOperation::Screenshot => RiskLevel::Safe,
        AgentOperation::MouseMove { .. } => RiskLevel::Caution,
        AgentOperation::MouseClick { .. } => RiskLevel::Caution,
        AgentOperation::MouseScroll { .. } => RiskLevel::Safe,
        AgentOperation::ComputerUse { .. } => RiskLevel::Caution,
        AgentOperation::KeyboardType { .. } => RiskLevel::Caution,
        AgentOperation::KeyboardHotkey { .. } => RiskLevel::Caution,

        // Browser automation operations
        AgentOperation::BrowserNavigate { .. } => RiskLevel::Caution,
        AgentOperation::BrowserClick { .. } => RiskLevel::Caution,
        AgentOperation::BrowserTypeText { .. } => RiskLevel::Caution,
        AgentOperation::BrowserScroll { .. } => RiskLevel::Safe,
        AgentOperation::BrowserExtractText { .. } => RiskLevel::Safe,
        AgentOperation::BrowserScreenshot => RiskLevel::Safe,
        AgentOperation::BrowserGetPageState => RiskLevel::Safe,
        AgentOperation::BrowserWaitFor { .. } => RiskLevel::Safe,
        AgentOperation::BrowserGoBack => RiskLevel::Safe,
        AgentOperation::BrowserEvaluate { .. } => RiskLevel::Dangerous,
    }
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde_path(path: &str) -> PathBuf {
    if path == "~" || path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            if path == "~" {
                return home;
            }
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Convert a tool call (name + args) into an AgentOperation and its risk level.
pub fn classify_tool_call(name: &str, args: &Value) -> (AgentOperation, RiskLevel) {
    let op = match name {
        "read_file" => {
            let path = args["path"].as_str().unwrap_or("");
            AgentOperation::ReadFile(expand_tilde_path(path))
        }
        "list_dir" => {
            let path = args["path"].as_str().unwrap_or("");
            AgentOperation::ReadDir(expand_tilde_path(path))
        }
        "create_dir" => {
            let path = args["path"].as_str().unwrap_or("");
            AgentOperation::CreateDir(expand_tilde_path(path))
        }
        "move_file" => {
            let from = args["from"].as_str().unwrap_or("");
            let to = args["to"].as_str().unwrap_or("");
            AgentOperation::MoveFile {
                from: expand_tilde_path(from),
                to: expand_tilde_path(to),
            }
        }
        "delete_file" => {
            let path = args["path"].as_str().unwrap_or("");
            let expanded = expand_tilde_path(path);
            // Check if it's a directory to differentiate
            if expanded.is_dir() {
                AgentOperation::DeleteDir(expanded)
            } else {
                AgentOperation::DeleteFile(expanded)
            }
        }
        "open_path" | "open_finder" => {
            let path = args["path"].as_str().unwrap_or("");
            AgentOperation::OpenPath(expand_tilde_path(path))
        }
        "shell_exec" => {
            let command = args["command"].as_str().unwrap_or("").to_string();
            AgentOperation::RunCommand {
                cmd: command,
                args: vec![],
            }
        }
        "run_applescript" => {
            let script = args["script"].as_str().unwrap_or("").to_string();
            AgentOperation::RunAppleScript(script)
        }
        // Computer Use tools
        "screenshot" => AgentOperation::Screenshot,
        "mouse_move" => AgentOperation::MouseMove {
            x: args["x"].as_i64().unwrap_or(0) as i32,
            y: args["y"].as_i64().unwrap_or(0) as i32,
        },
        "mouse_click" => AgentOperation::MouseClick {
            button: args["button"].as_str().unwrap_or("left").to_string(),
            click_type: args["click_type"].as_str().unwrap_or("single").to_string(),
        },
        "keyboard_type" => AgentOperation::KeyboardType {
            text: args["text"].as_str().unwrap_or("").to_string(),
        },
        "keyboard_hotkey" => {
            let keys = args["keys"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            AgentOperation::KeyboardHotkey { keys }
        }
        "mouse_scroll" => AgentOperation::MouseScroll {
            direction: args["direction"].as_str().unwrap_or("down").to_string(),
            amount: args["amount"].as_i64().unwrap_or(3) as i32,
        },
        "computer_use" => AgentOperation::ComputerUse {
            task: args["task"].as_str().unwrap_or("").to_string(),
            max_steps: args["max_steps"].as_i64().unwrap_or(15) as i32,
            live_vision: args["live_vision"].as_bool().unwrap_or_else(|| {
                let task = args["task"].as_str().unwrap_or("").to_lowercase();
                [
                    "canva",
                    "figma",
                    "presentation",
                    "drag",
                    "drop",
                    "timeline",
                    "animation",
                    "video",
                    "stream",
                    "live",
                    "continuously",
                    "constantly",
                ]
                .iter()
                .any(|k| task.contains(k))
            }),
        },
        // Browser automation tools
        "browser_navigate" => AgentOperation::BrowserNavigate {
            url: args["url"].as_str().unwrap_or("").to_string(),
        },
        "browser_click" => AgentOperation::BrowserClick {
            selector: args["selector"].as_str().unwrap_or("").to_string(),
        },
        "browser_type_text" => AgentOperation::BrowserTypeText {
            selector: args["selector"].as_str().unwrap_or("").to_string(),
            text: args["text"].as_str().unwrap_or("").to_string(),
        },
        "browser_scroll" => AgentOperation::BrowserScroll {
            direction: args["direction"].as_str().unwrap_or("down").to_string(),
            amount: args["amount"].as_f64().unwrap_or(500.0),
        },
        "browser_extract_text" => AgentOperation::BrowserExtractText {
            selector: args["selector"].as_str().map(String::from),
        },
        "browser_screenshot" => AgentOperation::BrowserScreenshot,
        "browser_get_page_state" => AgentOperation::BrowserGetPageState,
        "browser_wait_for" => AgentOperation::BrowserWaitFor {
            selector: args["selector"].as_str().unwrap_or("").to_string(),
        },
        "browser_go_back" => AgentOperation::BrowserGoBack,
        "browser_evaluate" => AgentOperation::BrowserEvaluate {
            expression: args["expression"].as_str().unwrap_or("").to_string(),
        },
        _ => {
            // Unknown tools get Caution by default
            AgentOperation::RunCommand {
                cmd: format!("unknown_tool:{}", name),
                args: vec![],
            }
        }
    };

    let risk = classify_operation(&op);
    (op, risk)
}

/// Generate a human-readable description of an operation for UI display.
pub fn operation_human_description(op: &AgentOperation) -> String {
    match op {
        AgentOperation::ReadFile(p) => format!("Read file: {}", p.display()),
        AgentOperation::WriteFile { path, .. } => format!("Write to file: {}", path.display()),
        AgentOperation::CreateFile { path, .. } => format!("Create file: {}", path.display()),
        AgentOperation::DeleteFile(p) => format!("Delete file: {}", p.display()),
        AgentOperation::DeleteDir(p) => format!("Delete directory: {}", p.display()),
        AgentOperation::MoveFile { from, to } => {
            format!("Move {} → {}", from.display(), to.display())
        }
        AgentOperation::CopyFile { from, to } => {
            format!("Copy {} → {}", from.display(), to.display())
        }
        AgentOperation::CreateDir(p) => format!("Create directory: {}", p.display()),
        AgentOperation::RunCommand { cmd, .. } => {
            format!("Run command: {}", cmd)
        }
        AgentOperation::ReadDir(p) => format!("List directory: {}", p.display()),
        AgentOperation::OpenPath(p) => format!("Open in file manager: {}", p.display()),
        AgentOperation::RunAppleScript(s) => {
            format!("Run AppleScript: {}", s)
        }
        // Computer Use
        AgentOperation::Screenshot => "Take screenshot (vision)".to_string(),
        AgentOperation::MouseMove { x, y } => format!("Move mouse to ({}, {})", x, y),
        AgentOperation::MouseClick { button, click_type } => {
            format!("{} {} click", button, click_type)
        }
        AgentOperation::KeyboardType { text } => {
            format!("Type: \"{}\"", text)
        }
        AgentOperation::KeyboardHotkey { keys } => {
            format!("Press hotkey: {}", keys.join("+"))
        }
        AgentOperation::MouseScroll { direction, amount } => {
            format!("Scroll {} by {}", direction, amount)
        }
        AgentOperation::ComputerUse {
            task,
            max_steps,
            live_vision,
        } => {
            let mode = if *live_vision { "live vision" } else { "snapshot vision" };
            format!(
                "Run computer-use loop for task '{}' (max {} steps, {})",
                task, max_steps, mode
            )
        }
        // Browser automation
        AgentOperation::BrowserNavigate { url } => {
            format!("Browser: navigate to {}", url)
        }
        AgentOperation::BrowserClick { selector } => format!("Browser: click {}", selector),
        AgentOperation::BrowserTypeText { selector, text } => {
            format!("Browser: type \"{}\" into {}", text, selector)
        }
        AgentOperation::BrowserScroll { direction, amount } => {
            format!("Browser: scroll {} by {}px", direction, amount)
        }
        AgentOperation::BrowserExtractText { selector } => {
            match selector {
                Some(s) => format!("Browser: extract text from {}", s),
                None => "Browser: extract page text".to_string(),
            }
        }
        AgentOperation::BrowserScreenshot => "Browser: take screenshot".to_string(),
        AgentOperation::BrowserGetPageState => "Browser: get page state".to_string(),
        AgentOperation::BrowserWaitFor { selector } => format!("Browser: wait for {}", selector),
        AgentOperation::BrowserGoBack => "Browser: go back".to_string(),
        AgentOperation::BrowserEvaluate { expression } => {
            format!("Browser: evaluate JS: {}", expression)
        }
    }
}
