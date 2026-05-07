use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentEvent {
    pub kind: String,
    // "tool_call" | "tool_result" | "message" | "done" | "error"
    pub content: String,
}
