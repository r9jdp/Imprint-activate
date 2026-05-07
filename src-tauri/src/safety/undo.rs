use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::classifier::AgentOperation;

/// What was captured before an operation, enabling reversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationSnapshot {
    /// A file was created — undo by deleting it
    FileCreated { path: PathBuf },
    /// A file was modified — undo by restoring original content
    FileModified {
        path: PathBuf,
        original_content: String,
    },
    /// A file was deleted — undo by recreating with saved content
    FileDeleted {
        path: PathBuf,
        original_content: String,
    },
    /// A file was moved — undo by moving back
    FileMoved { from: PathBuf, to: PathBuf },
    /// A directory was created — undo by removing (only if empty)
    DirectoryCreated { path: PathBuf },
    /// A command was run — cannot be undone
    CommandExecuted { command: String },
    /// Operation cannot be undone
    NotUndoable { reason: String },
}

impl OperationSnapshot {
    pub fn can_undo(&self) -> bool {
        !matches!(
            self,
            OperationSnapshot::CommandExecuted { .. } | OperationSnapshot::NotUndoable { .. }
        )
    }

    pub fn undo_description(&self) -> Option<String> {
        match self {
            OperationSnapshot::FileCreated { path } => {
                Some(format!("Delete created file: {}", path.display()))
            }
            OperationSnapshot::FileModified { path, .. } => {
                Some(format!("Restore original content of: {}", path.display()))
            }
            OperationSnapshot::FileDeleted { path, .. } => {
                Some(format!("Recreate deleted file: {}", path.display()))
            }
            OperationSnapshot::FileMoved { from, to } => {
                Some(format!("Move back: {} → {}", to.display(), from.display()))
            }
            OperationSnapshot::DirectoryCreated { path } => {
                Some(format!("Remove created directory: {}", path.display()))
            }
            OperationSnapshot::CommandExecuted { .. } => None,
            OperationSnapshot::NotUndoable { .. } => None,
        }
    }
}

/// A single undo entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub description: String,
    pub snapshot: OperationSnapshot,
}

/// The undo stack — holds recent operations that can be reversed.
pub struct UndoStack {
    entries: Arc<Mutex<Vec<UndoEntry>>>,
    max_entries: usize,
}

impl UndoStack {
    pub fn new(max_entries: usize) -> Self {
        let stack = Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            max_entries,
        };
        stack.load().ok();
        stack
    }

    fn storage_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".imprint")
            .join("undo")
    }

    fn today_file() -> PathBuf {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        Self::storage_dir().join(format!("{}.json", date))
    }

    /// Clear all entries (call at the start of each new agent run).
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
        self.persist().ok();
    }

    pub fn push(&self, entry: UndoEntry) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.push(entry);
            // Trim to max
            while entries.len() > self.max_entries {
                entries.remove(0);
            }
        }
        self.persist().ok();
    }

    pub fn get_entries(&self) -> Vec<UndoEntry> {
        self.entries
            .lock()
            .map(|e| e.clone())
            .unwrap_or_default()
    }

    pub fn pop_last(&self) -> Option<UndoEntry> {
        let entry = self.entries.lock().ok()?.pop();
        self.persist().ok();
        entry
    }

    pub fn remove_by_id(&self, id: &str) -> Option<UndoEntry> {
        let mut entries = self.entries.lock().ok()?;
        let idx = entries.iter().position(|e| e.id == id)?;
        let entry = entries.remove(idx);
        drop(entries);
        self.persist().ok();
        Some(entry)
    }

    pub fn persist(&self) -> Result<(), String> {
        let dir = Self::storage_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let entries = self.entries.lock().map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&*entries).map_err(|e| e.to_string())?;
        std::fs::write(Self::today_file(), json).map_err(|e| e.to_string())
    }

    pub fn load(&self) -> Result<(), String> {
        let path = Self::today_file();
        if !path.exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let loaded: Vec<UndoEntry> =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;
        let mut entries = self.entries.lock().map_err(|e| e.to_string())?;
        *entries = loaded;
        Ok(())
    }
}

/// Take a snapshot of the current state before an operation executes.
pub fn snapshot_before_operation(op: &AgentOperation) -> Result<OperationSnapshot, String> {
    match op {
        AgentOperation::DeleteFile(path) => {
            if path.exists() && path.is_file() {
                let content =
                    std::fs::read_to_string(path).map_err(|e| format!("Cannot snapshot file for undo: {}", e))?;
                Ok(OperationSnapshot::FileDeleted {
                    path: path.clone(),
                    original_content: content,
                })
            } else {
                Ok(OperationSnapshot::NotUndoable {
                    reason: "File does not exist or is not a regular file".to_string(),
                })
            }
        }

        AgentOperation::DeleteDir(path) => {
            // Directories are complex to snapshot — mark as not undoable
            Ok(OperationSnapshot::NotUndoable {
                reason: format!("Directory deletion ({}) cannot be fully undone", path.display()),
            })
        }

        AgentOperation::WriteFile { path, .. } | AgentOperation::CreateFile { path, .. } => {
            if path.exists() && path.is_file() {
                let content =
                    std::fs::read_to_string(path).map_err(|e| format!("Cannot snapshot file for undo: {}", e))?;
                Ok(OperationSnapshot::FileModified {
                    path: path.clone(),
                    original_content: content,
                })
            } else {
                Ok(OperationSnapshot::FileCreated {
                    path: path.clone(),
                })
            }
        }

        AgentOperation::MoveFile { from, to } => Ok(OperationSnapshot::FileMoved {
            from: from.clone(),
            to: to.clone(),
        }),

        AgentOperation::CreateDir(path) => Ok(OperationSnapshot::DirectoryCreated {
            path: path.clone(),
        }),

        AgentOperation::RunCommand { cmd, .. } => Ok(OperationSnapshot::CommandExecuted {
            command: cmd.clone(),
        }),

        AgentOperation::RunAppleScript(script) => Ok(OperationSnapshot::CommandExecuted {
            command: format!("AppleScript: {}", if script.len() > 50 { &script[..50] } else { script }),
        }),

        // Read-only operations don't need snapshots
        AgentOperation::ReadFile(_)
        | AgentOperation::ReadDir(_)
        | AgentOperation::OpenPath(_)
        | AgentOperation::CopyFile { .. } => Ok(OperationSnapshot::NotUndoable {
            reason: "Read-only or copy operation".to_string(),
        }),

        // Computer Use operations — transient, not undoable
        AgentOperation::Screenshot
        | AgentOperation::MouseMove { .. }
        | AgentOperation::MouseClick { .. }
        | AgentOperation::KeyboardType { .. }
        | AgentOperation::KeyboardHotkey { .. }
        | AgentOperation::MouseScroll { .. }
        | AgentOperation::ComputerUse { .. } => Ok(OperationSnapshot::NotUndoable {
            reason: "Computer use action (transient)".to_string(),
        }),

        // Browser automation operations — transient, not undoable
        AgentOperation::BrowserNavigate { .. }
        | AgentOperation::BrowserClick { .. }
        | AgentOperation::BrowserTypeText { .. }
        | AgentOperation::BrowserScroll { .. }
        | AgentOperation::BrowserExtractText { .. }
        | AgentOperation::BrowserScreenshot
        | AgentOperation::BrowserGetPageState
        | AgentOperation::BrowserWaitFor { .. }
        | AgentOperation::BrowserGoBack
        | AgentOperation::BrowserEvaluate { .. } => Ok(OperationSnapshot::NotUndoable {
            reason: "Browser action (transient)".to_string(),
        }),
    }
}

/// Apply an undo operation — reverse what was done.
pub fn apply_undo(entry: &UndoEntry) -> Result<String, String> {
    match &entry.snapshot {
        OperationSnapshot::FileCreated { path } => {
            if path.exists() {
                std::fs::remove_file(path)
                    .map_err(|e| format!("Failed to undo file creation: {}", e))?;
                Ok(format!("Undone: deleted created file {}", path.display()))
            } else {
                Ok(format!("File {} was already removed", path.display()))
            }
        }

        OperationSnapshot::FileModified {
            path,
            original_content,
        } => {
            std::fs::write(path, original_content)
                .map_err(|e| format!("Failed to restore file: {}", e))?;
            Ok(format!("Undone: restored original content of {}", path.display()))
        }

        OperationSnapshot::FileDeleted {
            path,
            original_content,
        } => {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(path, original_content)
                .map_err(|e| format!("Failed to recreate file: {}", e))?;
            Ok(format!("Undone: recreated deleted file {}", path.display()))
        }

        OperationSnapshot::FileMoved { from, to } => {
            if to.exists() {
                std::fs::rename(to, from)
                    .map_err(|e| format!("Failed to move back: {}", e))?;
                Ok(format!(
                    "Undone: moved {} back to {}",
                    to.display(),
                    from.display()
                ))
            } else {
                Err(format!(
                    "Cannot undo move: {} no longer exists",
                    to.display()
                ))
            }
        }

        OperationSnapshot::DirectoryCreated { path } => {
            if path.exists() && path.is_dir() {
                std::fs::remove_dir(path)
                    .map_err(|e| format!("Failed to remove directory (may not be empty): {}", e))?;
                Ok(format!("Undone: removed created directory {}", path.display()))
            } else {
                Ok(format!("Directory {} was already removed", path.display()))
            }
        }

        OperationSnapshot::CommandExecuted { command } => {
            Err(format!("Cannot undo command execution: {}", command))
        }

        OperationSnapshot::NotUndoable { reason } => {
            Err(format!("Cannot undo: {}", reason))
        }
    }
}
