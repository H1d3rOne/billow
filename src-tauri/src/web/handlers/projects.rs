use std::sync::Arc;

use axum::{extract::Extension, Json};

use crate::app_error::AppCommandError;
use crate::app_state::AppState;
use crate::commands::projects as project_commands;
use crate::models::{
    AddCollaborativeProjectRoleParams, AddCollaborativeProjectRoleResult,
    CollaborativeProjectSummary, CreateCollaborativeProjectParams,
    CreateCollaborativeProjectResult, EnsureProjectRoleConversationParams,
    ProjectRoleConversationResult, RemoveCollaborativeProjectResult,
};

pub async fn create_collaborative_project(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<CreateCollaborativeProjectParams>,
) -> Result<Json<CreateCollaborativeProjectResult>, AppCommandError> {
    Ok(Json(
        project_commands::create_collaborative_project_core(&state.emitter, &state.db, params)
            .await?,
    ))
}

pub async fn list_collaborative_projects(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<Vec<CollaborativeProjectSummary>>, AppCommandError> {
    Ok(Json(
        project_commands::list_collaborative_projects_core(&state.db).await?,
    ))
}

pub async fn add_collaborative_project_role(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<AddCollaborativeProjectRoleParams>,
) -> Result<Json<AddCollaborativeProjectRoleResult>, AppCommandError> {
    Ok(Json(
        project_commands::add_collaborative_project_role_core(&state.emitter, &state.db, params)
            .await?,
    ))
}

pub async fn ensure_project_role_conversation(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<EnsureProjectRoleConversationParams>,
) -> Result<Json<ProjectRoleConversationResult>, AppCommandError> {
    Ok(Json(
        project_commands::ensure_project_role_conversation_core(&state.emitter, &state.db, params)
            .await?,
    ))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveCollaborativeProjectParams {
    pub project_id: i32,
}

pub async fn remove_collaborative_project(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<RemoveCollaborativeProjectParams>,
) -> Result<Json<RemoveCollaborativeProjectResult>, AppCommandError> {
    Ok(Json(
        project_commands::remove_collaborative_project_core(&state.db, params.project_id).await?,
    ))
}
