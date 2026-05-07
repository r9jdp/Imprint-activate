use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

use super::classifier::{AgentOperation, RiskLevel};

/// A single step in an execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedStep {
    pub index: usize,
    pub description: String,
    pub operation: AgentOperation,
    pub risk: RiskLevel,
    pub can_undo: bool,
    pub undo_description: Option<String>,
}

/// An execution plan generated before running a series of operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub id: String,
    pub summary: String,
    pub steps: Vec<PlannedStep>,
    pub overall_risk: RiskLevel,
}

/// Manages the approval state for pending plans.
pub struct PlanApprovalState {
    /// The pending approval channel sender, if a plan is awaiting approval.
    pending: Arc<Mutex<Option<PendingPlan>>>,
}

struct PendingPlan {
    plan_id: String,
    sender: oneshot::Sender<bool>,
}

impl PlanApprovalState {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
        }
    }

    /// Set up a new pending plan and return the receiver to await approval.
    pub fn wait_for_approval(&self, plan_id: String) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut pending) = self.pending.lock() {
            *pending = Some(PendingPlan {
                plan_id,
                sender: tx,
            });
        }
        rx
    }

    /// Approve a pending plan.
    pub fn approve(&self, plan_id: &str) -> Result<(), String> {
        let mut pending = self.pending.lock().map_err(|e| e.to_string())?;
        if let Some(p) = pending.take() {
            if p.plan_id == plan_id {
                p.sender.send(true).map_err(|_| "Approval channel closed".to_string())?;
                Ok(())
            } else {
                // Put it back
                *pending = Some(p);
                Err(format!("Plan ID mismatch: expected {}, got {}", plan_id, plan_id))
            }
        } else {
            Err("No pending plan to approve".to_string())
        }
    }

    /// Reject a pending plan.
    pub fn reject(&self, plan_id: &str) -> Result<(), String> {
        let mut pending = self.pending.lock().map_err(|e| e.to_string())?;
        if let Some(p) = pending.take() {
            if p.plan_id == plan_id {
                p.sender.send(false).map_err(|_| "Rejection channel closed".to_string())?;
                Ok(())
            } else {
                *pending = Some(p);
                Err(format!("Plan ID mismatch"))
            }
        } else {
            Err("No pending plan to reject".to_string())
        }
    }
}

/// Try to parse a plan from the LLM's response text.
/// Looks for `<imprint_plan>...</imprint_plan>` blocks containing JSON.
pub fn parse_plan_from_response(text: &str) -> Option<serde_json::Value> {
    let start_tag = "<imprint_plan>";
    let end_tag = "</imprint_plan>";

    let start = text.find(start_tag)?;
    let end = text.find(end_tag)?;
    if end <= start {
        return None;
    }

    let json_str = &text[start + start_tag.len()..end].trim();
    serde_json::from_str(json_str).ok()
}

/// Build an ExecutionPlan from parsed plan JSON, classifying each step.
pub fn build_execution_plan(
    plan_json: &serde_json::Value,
) -> Result<ExecutionPlan, String> {
    let summary = plan_json["summary"]
        .as_str()
        .unwrap_or("Execution plan")
        .to_string();

    let steps_json = plan_json["steps"]
        .as_array()
        .ok_or("Plan missing 'steps' array")?;

    let mut steps = Vec::new();
    let mut overall_risk = RiskLevel::Safe;

    for (i, step) in steps_json.iter().enumerate() {
        let description = step["description"]
            .as_str()
            .unwrap_or("Unknown step")
            .to_string();

        let tool_name = step["tool"].as_str().unwrap_or("");
        let tool_args = step.get("args").cloned().unwrap_or(serde_json::json!({}));

        let (operation, risk) = super::classifier::classify_tool_call(tool_name, &tool_args);
        overall_risk = overall_risk.max(risk);

        let snapshot_hint = super::undo::snapshot_before_operation(&operation);
        let (can_undo, undo_description) = match snapshot_hint {
            Ok(snap) => (snap.can_undo(), snap.undo_description()),
            Err(_) => (false, None),
        };

        steps.push(PlannedStep {
            index: i,
            description,
            operation,
            risk,
            can_undo,
            undo_description,
        });
    }

    Ok(ExecutionPlan {
        id: uuid::Uuid::new_v4().to_string(),
        summary,
        steps,
        overall_risk,
    })
}
