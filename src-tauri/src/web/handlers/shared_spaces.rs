use std::sync::Arc;

use axum::{extract::Extension, Json};

use crate::app_error::AppCommandError;
use crate::app_state::AppState;
use crate::commands::shared_spaces as shared_space_commands;
use crate::models::{
    AppendSharedSpaceMessageParams, CreateSharedSpaceConversationParams,
    CreateSharedSpaceConversationResult, CreateSharedSpaceParams, CreateSharedSpaceResult,
    ListSharedSpaceMessagesParams, RemoveSharedSpaceResult, SharedSpaceMessageSummary,
    SharedSpaceSummary, SwitchSharedSpaceAgentParams, SwitchSharedSpaceAgentResult,
};

pub async fn create_shared_space(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<CreateSharedSpaceParams>,
) -> Result<Json<CreateSharedSpaceResult>, AppCommandError> {
    Ok(Json(
        shared_space_commands::create_shared_space_core(&state.emitter, &state.db, params).await?,
    ))
}

pub async fn list_shared_spaces(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<Vec<SharedSpaceSummary>>, AppCommandError> {
    Ok(Json(
        shared_space_commands::list_shared_spaces_core(&state.db).await?,
    ))
}

pub async fn create_shared_space_conversation(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<CreateSharedSpaceConversationParams>,
) -> Result<Json<CreateSharedSpaceConversationResult>, AppCommandError> {
    Ok(Json(
        shared_space_commands::create_shared_space_conversation_core(
            &state.emitter,
            &state.db,
            params,
        )
        .await?,
    ))
}

pub async fn switch_shared_space_agent(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<SwitchSharedSpaceAgentParams>,
) -> Result<Json<SwitchSharedSpaceAgentResult>, AppCommandError> {
    Ok(Json(
        shared_space_commands::switch_shared_space_agent_core(&state.emitter, &state.db, params)
            .await?,
    ))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSharedSpaceParams {
    pub space_id: i32,
}

pub async fn remove_shared_space(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<RemoveSharedSpaceParams>,
) -> Result<Json<RemoveSharedSpaceResult>, AppCommandError> {
    Ok(Json(
        shared_space_commands::remove_shared_space_core(&state.db, params.space_id).await?,
    ))
}

pub async fn append_shared_space_message(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<AppendSharedSpaceMessageParams>,
) -> Result<Json<SharedSpaceMessageSummary>, AppCommandError> {
    Ok(Json(
        shared_space_commands::append_shared_space_message_core(&state.db, params).await?,
    ))
}

pub async fn list_shared_space_messages(
    Extension(state): Extension<Arc<AppState>>,
    Json(params): Json<ListSharedSpaceMessagesParams>,
) -> Result<Json<Vec<SharedSpaceMessageSummary>>, AppCommandError> {
    Ok(Json(
        shared_space_commands::list_shared_space_messages_core(&state.db, params).await?,
    ))
}
