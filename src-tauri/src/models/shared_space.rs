use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::{AgentType, FolderDetail};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedSpaceParams {
    pub name: String,
    pub working_dir: String,
    pub agent_type: AgentType,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedSpaceResult {
    pub space_id: i32,
    pub folder_id: i32,
    pub folder: FolderDetail,
    pub active_agent_type: AgentType,
    pub active_shared_conversation_id: i32,
    pub active_conversation_id: i32,
    pub conversation_title: String,
    pub context_snapshot_id: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedSpaceConversationParams {
    pub space_id: i32,
    pub title: Option<String>,
    pub agent_type: Option<AgentType>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedSpaceConversationResult {
    pub space_id: i32,
    pub shared_conversation_id: i32,
    pub agent_type: AgentType,
    pub conversation_id: i32,
    pub conversation_title: String,
    pub context_snapshot_id: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchSharedSpaceAgentParams {
    pub space_id: i32,
    pub agent_type: AgentType,
    pub shared_conversation_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchSharedSpaceAgentResult {
    pub space_id: i32,
    pub shared_conversation_id: i32,
    pub agent_type: AgentType,
    pub conversation_id: i32,
    pub conversation_title: String,
    pub context_snapshot_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSharedSpaceResult {
    pub space_id: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedSpaceConversationAgentSummary {
    pub agent_type: AgentType,
    pub native_session_id: Option<String>,
    pub last_used_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedSpaceConversationSummary {
    pub shared_conversation_id: i32,
    pub conversation_id: i32,
    pub conversation_title: Option<String>,
    pub active_agent_type: AgentType,
    pub agents: Vec<SharedSpaceConversationAgentSummary>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedSpaceSummary {
    pub space_id: i32,
    pub name: String,
    pub folder_id: i32,
    pub folder_name: String,
    pub folder_path: String,
    pub active_agent_type: Option<AgentType>,
    pub active_shared_conversation_id: Option<i32>,
    pub active_conversation_id: Option<i32>,
    pub conversations: Vec<SharedSpaceConversationSummary>,
    pub messages_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendSharedSpaceMessageParams {
    pub space_id: i32,
    pub binding_id: Option<i32>,
    pub role: String,
    pub content: String,
    pub source_agent_type: Option<AgentType>,
    pub native_message_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSharedSpaceMessagesParams {
    pub space_id: i32,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedSpaceMessageSummary {
    pub message_id: i32,
    pub space_id: i32,
    pub binding_id: Option<i32>,
    pub role: String,
    pub content: String,
    pub source_agent_type: Option<AgentType>,
    pub native_message_id: Option<String>,
    pub created_at: DateTime<Utc>,
}
