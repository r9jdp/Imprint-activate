use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::classifier::{AgentOperation, has_sudo};

/// What kind of scope violation occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScopeViolation {
    None,
    PathOutOfScope(String),
    SudoDenied,
    NetworkDenied,
    SystemPathDenied(String),
}

impl ScopeViolation {
    pub fn is_violation(&self) -> bool {
        !matches!(self, ScopeViolation::None)
    }

    pub fn message(&self) -> String {
        match self {
            ScopeViolation::None => String::new(),
            ScopeViolation::PathOutOfScope(p) => {
                format!("BLOCKED: Path '{}' is outside the allowed scope. Configure allowed paths in Settings → Safety.", p)
            }
            ScopeViolation::SudoDenied => {
                "BLOCKED: Sudo/privilege escalation is not allowed. Enable it in Settings → Safety if needed.".to_string()
            }
            ScopeViolation::NetworkDenied => {
                "BLOCKED: Network commands are not allowed in current scope settings.".to_string()
            }
            ScopeViolation::SystemPathDenied(p) => {
                format!("BLOCKED: Cannot modify system path '{}'. Disable system path protection in Settings → Safety if needed.", p)
            }
        }
    }
}

/// Persisted scope configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeConfig {
    pub allowed_paths: Vec<String>,
    pub deny_sudo: bool,
    pub deny_network_commands: bool,
    pub deny_system_paths: bool,
}

impl Default for ScopeConfig {
    fn default() -> Self {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "~".to_string());
        Self {
            allowed_paths: vec![home],
            deny_sudo: true,
            deny_network_commands: false,
            deny_system_paths: true,
        }
    }
}

impl ScopeConfig {
    fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".imprint")
    }

    fn config_path() -> PathBuf {
        Self::config_dir().join("scope.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    serde_json::from_str(&content).unwrap_or_default()
                }
                Err(_) => Self::default(),
            }
        } else {
            let config = Self::default();
            config.save().ok();
            config
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(Self::config_path(), json).map_err(|e| e.to_string())
    }
}

/// Runtime scope checker.
pub struct ScopeGuard {
    pub config: Arc<Mutex<ScopeConfig>>,
}

impl ScopeGuard {
    pub fn new(config: ScopeConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
        }
    }

    pub fn from_saved() -> Self {
        Self::new(ScopeConfig::load())
    }

    pub fn update_config(&self, new_config: ScopeConfig) -> Result<(), String> {
        new_config.save()?;
        let mut cfg = self.config.lock().map_err(|e| e.to_string())?;
        *cfg = new_config;
        Ok(())
    }

    pub fn get_config(&self) -> Result<ScopeConfig, String> {
        let cfg = self.config.lock().map_err(|e| e.to_string())?;
        Ok(cfg.clone())
    }

    /// Check if an operation is allowed under current scope rules.
    pub fn check(&self, op: &AgentOperation) -> ScopeViolation {
        let cfg = match self.config.lock() {
            Ok(c) => c.clone(),
            Err(_) => return ScopeViolation::None, // fail open if lock poisoned
        };

        match op {
            // Read-only operations and path opening are always allowed
            AgentOperation::ReadFile(_)
            | AgentOperation::ReadDir(_)
            | AgentOperation::OpenPath(_) => ScopeViolation::None,

            // Path-based write operations
            AgentOperation::WriteFile { path, .. }
            | AgentOperation::CreateFile { path, .. }
            | AgentOperation::DeleteFile(path)
            | AgentOperation::DeleteDir(path)
            | AgentOperation::CreateDir(path) => {
                if cfg.deny_system_paths && super::classifier::is_system_path(path) {
                    return ScopeViolation::SystemPathDenied(path.to_string_lossy().to_string());
                }
                self.check_path_in_scope(&cfg, path)
            }

            AgentOperation::MoveFile { from, to } | AgentOperation::CopyFile { from, to } => {
                if cfg.deny_system_paths
                    && (super::classifier::is_system_path(from) || super::classifier::is_system_path(to))
                {
                    return ScopeViolation::SystemPathDenied(
                        format!("{} or {}", from.display(), to.display()),
                    );
                }
                let v1 = self.check_path_in_scope(&cfg, from);
                if v1.is_violation() {
                    return v1;
                }
                self.check_path_in_scope(&cfg, to)
            }

            AgentOperation::RunCommand { cmd, .. } => {
                if cfg.deny_sudo && has_sudo(cmd) {
                    return ScopeViolation::SudoDenied;
                }
                if cfg.deny_network_commands && is_network_command(cmd) {
                    return ScopeViolation::NetworkDenied;
                }
                ScopeViolation::None
            }

            AgentOperation::RunAppleScript(script) => {
                let lower = script.to_lowercase();
                if cfg.deny_sudo && lower.contains("with administrator privileges") {
                    return ScopeViolation::SudoDenied;
                }
                ScopeViolation::None
            }

            // Computer Use operations don't touch the filesystem — always allowed
            AgentOperation::Screenshot
            | AgentOperation::MouseMove { .. }
            | AgentOperation::MouseClick { .. }
            | AgentOperation::KeyboardType { .. }
            | AgentOperation::KeyboardHotkey { .. }
            | AgentOperation::MouseScroll { .. }
            | AgentOperation::ComputerUse { .. } => ScopeViolation::None,

            // Browser automation operations don't touch the filesystem — always allowed
            AgentOperation::BrowserNavigate { .. }
            | AgentOperation::BrowserClick { .. }
            | AgentOperation::BrowserTypeText { .. }
            | AgentOperation::BrowserScroll { .. }
            | AgentOperation::BrowserExtractText { .. }
            | AgentOperation::BrowserScreenshot
            | AgentOperation::BrowserGetPageState
            | AgentOperation::BrowserWaitFor { .. }
            | AgentOperation::BrowserGoBack
            | AgentOperation::BrowserEvaluate { .. } => ScopeViolation::None,
        }
    }

    fn check_path_in_scope(&self, cfg: &ScopeConfig, path: &std::path::Path) -> ScopeViolation {
        let path_str = path.to_string_lossy().to_string();

        // Canonicalize for comparison if possible
        let canonical = std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf());
        let canonical_str = canonical.to_string_lossy().to_string();

        for allowed in &cfg.allowed_paths {
            let allowed_canonical = std::fs::canonicalize(allowed)
                .unwrap_or_else(|_| PathBuf::from(allowed));
            let allowed_str = allowed_canonical.to_string_lossy().to_string();

            if canonical_str.starts_with(&allowed_str) || path_str.starts_with(allowed) {
                return ScopeViolation::None;
            }
        }

        ScopeViolation::PathOutOfScope(path_str)
    }
}

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
