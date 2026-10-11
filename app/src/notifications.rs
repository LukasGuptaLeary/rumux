use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub workspace_id: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_index: Option<usize>,
    pub title: String,
    pub subtitle: Option<String>,
    pub body: String,
    pub timestamp: u64,
    pub read: bool,
}
