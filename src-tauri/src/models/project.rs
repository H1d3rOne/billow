use serde::{Deserialize, Serialize};

use chrono::{DateTime, Utc};

use crate::models::{AgentType, FolderDetail};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollaborativeProjectParams {
    pub name: String,
    pub working_dir: String,
    pub roles: Vec<CreateCollaborativeProjectRoleParams>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollaborativeProjectRoleParams {
    pub name: String,
    pub role_key: String,
    pub agent_type: AgentType,
    pub instructions: Option<String>,
    pub is_manager: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollaborativeProjectResult {
    pub project_id: i32,
    pub folder_id: i32,
    pub folder: FolderDetail,
    pub manager_role_id: i32,
    pub manager_conversation_id: i32,
    pub manager_title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCollaborativeProjectRoleParams {
    pub project_id: i32,
    pub name: String,
    pub role_key: String,
    pub agent_type: AgentType,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCollaborativeProjectRoleResult {
    pub role: CollaborativeProjectRoleSummary,
    pub conversation_id: i32,
    pub conversation_title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveCollaborativeProjectResult {
    pub project_id: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnsureProjectRoleConversationParams {
    pub role_id: i32,
    #[serde(default)]
    pub recreate: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRoleConversationResult {
    pub project_id: i32,
    pub role_id: i32,
    pub conversation_id: i32,
    pub conversation_title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborativeProjectRoleSummary {
    pub role_id: i32,
    pub name: String,
    pub role_key: String,
    pub agent_type: AgentType,
    pub instructions: Option<String>,
    pub is_manager: bool,
    pub conversation_id: Option<i32>,
    pub conversation_title: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborativeProjectSummary {
    pub project_id: i32,
    pub name: String,
    pub folder_id: i32,
    pub folder_name: String,
    pub folder_path: String,
    pub manager_role_id: i32,
    pub manager_conversation_id: i32,
    pub manager_agent_type: AgentType,
    pub manager_title: Option<String>,
    pub roles_count: u64,
    pub roles: Vec<CollaborativeProjectRoleSummary>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
