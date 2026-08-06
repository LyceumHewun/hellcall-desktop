use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSessionEvent {
    pub id: String,
    pub kind: String,
    pub text: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSessionSummary {
    pub id: String,
    pub title: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSessionRecord {
    #[serde(flatten)]
    pub summary: AiSessionSummary,
    pub events: Vec<AiSessionEvent>,
}
