use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, EntityTrait,
    IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::db::entities::{
    conversation, folder, shared_space, shared_space_agent_binding, shared_space_context_snapshot,
    shared_space_conversation, shared_space_conversation_agent, shared_space_message,
};
use crate::db::error::DbError;
use crate::models::{
    AgentType, SharedSpaceConversationAgentSummary, SharedSpaceConversationSummary,
    SharedSpaceMessageSummary, SharedSpaceSummary,
};

fn agent_type_to_db(agent_type: AgentType) -> String {
    serde_json::to_value(agent_type)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn agent_type_from_db(raw: &str) -> Result<AgentType, DbError> {
    serde_json::from_value(serde_json::Value::String(raw.to_string()))
        .map_err(|e| DbError::Migration(format!("Invalid shared space agent type: {e}")))
}

fn optional_agent_type_from_db(raw: Option<String>) -> Result<Option<AgentType>, DbError> {
    raw.as_deref().map(agent_type_from_db).transpose()
}

pub struct NewSharedSpaceMessage<'a> {
    pub space_id: i32,
    pub binding_id: Option<i32>,
    pub role: &'a str,
    pub content: &'a str,
    pub source_agent_type: Option<AgentType>,
    pub native_message_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct SharedSpaceWithFolder {
    pub space: shared_space::Model,
    pub folder: folder::Model,
}

pub async fn create_space<C>(
    conn: &C,
    name: &str,
    folder_id: i32,
) -> Result<shared_space::Model, DbError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    let active = shared_space::ActiveModel {
        id: NotSet,
        name: Set(name.to_string()),
        folder_id: Set(folder_id),
        active_agent_binding_id: Set(None),
        active_shared_conversation_id: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
    };
    Ok(active.insert(conn).await?)
}

pub async fn get_space_with_folder<C>(
    conn: &C,
    space_id: i32,
) -> Result<Option<SharedSpaceWithFolder>, DbError>
where
    C: ConnectionTrait,
{
    let Some(space) = shared_space::Entity::find_by_id(space_id)
        .filter(shared_space::Column::DeletedAt.is_null())
        .one(conn)
        .await?
    else {
        return Ok(None);
    };

    let Some(folder) = folder::Entity::find_by_id(space.folder_id)
        .filter(folder::Column::DeletedAt.is_null())
        .one(conn)
        .await?
    else {
        return Ok(None);
    };

    Ok(Some(SharedSpaceWithFolder { space, folder }))
}

pub async fn create_shared_conversation<C>(
    conn: &C,
    space_id: i32,
    conversation_id: i32,
    title: Option<String>,
    active_agent_type: AgentType,
) -> Result<shared_space_conversation::Model, DbError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    let active = shared_space_conversation::ActiveModel {
        id: NotSet,
        space_id: Set(space_id),
        conversation_id: Set(conversation_id),
        title: Set(title),
        active_agent_type: Set(agent_type_to_db(active_agent_type)),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
    };
    Ok(active.insert(conn).await?)
}

pub async fn get_shared_conversation<C>(
    conn: &C,
    shared_conversation_id: i32,
) -> Result<Option<shared_space_conversation::Model>, DbError>
where
    C: ConnectionTrait,
{
    Ok(
        shared_space_conversation::Entity::find_by_id(shared_conversation_id)
            .filter(shared_space_conversation::Column::DeletedAt.is_null())
            .one(conn)
            .await?,
    )
}

pub async fn get_shared_conversation_for_conversation<C>(
    conn: &C,
    conversation_id: i32,
) -> Result<Option<shared_space_conversation::Model>, DbError>
where
    C: ConnectionTrait,
{
    Ok(shared_space_conversation::Entity::find()
        .filter(shared_space_conversation::Column::ConversationId.eq(conversation_id))
        .filter(shared_space_conversation::Column::DeletedAt.is_null())
        .one(conn)
        .await?)
}

pub async fn list_shared_conversations<C>(
    conn: &C,
    space_id: i32,
) -> Result<Vec<shared_space_conversation::Model>, DbError>
where
    C: ConnectionTrait,
{
    Ok(shared_space_conversation::Entity::find()
        .filter(shared_space_conversation::Column::SpaceId.eq(space_id))
        .filter(shared_space_conversation::Column::DeletedAt.is_null())
        .order_by_desc(shared_space_conversation::Column::UpdatedAt)
        .order_by_desc(shared_space_conversation::Column::Id)
        .all(conn)
        .await?)
}

pub async fn get_active_shared_conversation<C>(
    conn: &C,
    space: &shared_space::Model,
) -> Result<Option<shared_space_conversation::Model>, DbError>
where
    C: ConnectionTrait,
{
    if let Some(id) = space.active_shared_conversation_id {
        if let Some(row) = get_shared_conversation(conn, id).await? {
            return Ok(Some(row));
        }
    }
    Ok(shared_space_conversation::Entity::find()
        .filter(shared_space_conversation::Column::SpaceId.eq(space.id))
        .filter(shared_space_conversation::Column::DeletedAt.is_null())
        .order_by_desc(shared_space_conversation::Column::UpdatedAt)
        .order_by_desc(shared_space_conversation::Column::Id)
        .one(conn)
        .await?)
}

pub async fn set_active_shared_conversation<C>(
    conn: &C,
    space_id: i32,
    shared_conversation_id: i32,
) -> Result<shared_space::Model, DbError>
where
    C: ConnectionTrait,
{
    let row = shared_space::Entity::find_by_id(space_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Shared space not found: {space_id}")))?;
    let mut active = row.into_active_model();
    active.active_shared_conversation_id = Set(Some(shared_conversation_id));
    active.updated_at = Set(Utc::now());
    Ok(active.update(conn).await?)
}

pub async fn set_shared_conversation_agent<C>(
    conn: &C,
    shared_conversation_id: i32,
    agent_type: AgentType,
) -> Result<shared_space_conversation::Model, DbError>
where
    C: ConnectionTrait,
{
    let row = shared_space_conversation::Entity::find_by_id(shared_conversation_id)
        .one(conn)
        .await?
        .ok_or_else(|| {
            DbError::Migration(format!(
                "Shared space conversation not found: {shared_conversation_id}"
            ))
        })?;
    let mut active = row.into_active_model();
    active.active_agent_type = Set(agent_type_to_db(agent_type));
    active.updated_at = Set(Utc::now());
    Ok(active.update(conn).await?)
}

pub async fn ensure_conversation_agent_session<C>(
    conn: &C,
    shared_conversation_id: i32,
    agent_type: AgentType,
) -> Result<shared_space_conversation_agent::Model, DbError>
where
    C: ConnectionTrait,
{
    let raw = agent_type_to_db(agent_type);
    ensure_conversation_agent_session_raw(conn, shared_conversation_id, &raw, None).await
}

pub async fn ensure_conversation_agent_session_raw<C>(
    conn: &C,
    shared_conversation_id: i32,
    agent_type: &str,
    native_session_id: Option<String>,
) -> Result<shared_space_conversation_agent::Model, DbError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    if let Some(row) = shared_space_conversation_agent::Entity::find()
        .filter(
            shared_space_conversation_agent::Column::SharedConversationId
                .eq(shared_conversation_id),
        )
        .filter(shared_space_conversation_agent::Column::AgentType.eq(agent_type))
        .one(conn)
        .await?
    {
        let mut active = row.into_active_model();
        if native_session_id.is_some() {
            active.native_session_id = Set(native_session_id);
        }
        active.last_used_at = Set(now);
        active.updated_at = Set(now);
        return Ok(active.update(conn).await?);
    }

    let active = shared_space_conversation_agent::ActiveModel {
        id: NotSet,
        shared_conversation_id: Set(shared_conversation_id),
        agent_type: Set(agent_type.to_string()),
        native_session_id: Set(native_session_id),
        last_used_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
    };
    Ok(active.insert(conn).await?)
}

pub async fn set_conversation_agent_native_session<C>(
    conn: &C,
    shared_conversation_id: i32,
    agent_type: &str,
    native_session_id: Option<String>,
) -> Result<shared_space_conversation_agent::Model, DbError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    if let Some(row) = shared_space_conversation_agent::Entity::find()
        .filter(
            shared_space_conversation_agent::Column::SharedConversationId
                .eq(shared_conversation_id),
        )
        .filter(shared_space_conversation_agent::Column::AgentType.eq(agent_type))
        .one(conn)
        .await?
    {
        let mut active = row.into_active_model();
        active.native_session_id = Set(native_session_id);
        active.last_used_at = Set(now);
        active.updated_at = Set(now);
        return Ok(active.update(conn).await?);
    }

    let active = shared_space_conversation_agent::ActiveModel {
        id: NotSet,
        shared_conversation_id: Set(shared_conversation_id),
        agent_type: Set(agent_type.to_string()),
        native_session_id: Set(native_session_id),
        last_used_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
    };
    Ok(active.insert(conn).await?)
}

pub async fn upsert_agent_session_for_conversation<C>(
    conn: &C,
    conversation_id: i32,
    agent_type: &str,
    native_session_id: Option<String>,
) -> Result<(), DbError>
where
    C: ConnectionTrait,
{
    if let Some(shared_conversation) =
        get_shared_conversation_for_conversation(conn, conversation_id).await?
    {
        set_conversation_agent_native_session(
            conn,
            shared_conversation.id,
            agent_type,
            native_session_id,
        )
        .await?;
    }
    Ok(())
}

pub async fn list_conversation_agent_sessions<C>(
    conn: &C,
    shared_conversation_id: i32,
) -> Result<Vec<shared_space_conversation_agent::Model>, DbError>
where
    C: ConnectionTrait,
{
    Ok(shared_space_conversation_agent::Entity::find()
        .filter(
            shared_space_conversation_agent::Column::SharedConversationId
                .eq(shared_conversation_id),
        )
        .order_by_desc(shared_space_conversation_agent::Column::LastUsedAt)
        .order_by_desc(shared_space_conversation_agent::Column::Id)
        .all(conn)
        .await?)
}

pub async fn conversation_active<C>(
    conn: &C,
    conversation_id: i32,
) -> Result<Option<conversation::Model>, DbError>
where
    C: ConnectionTrait,
{
    Ok(conversation::Entity::find_by_id(conversation_id)
        .filter(conversation::Column::DeletedAt.is_null())
        .one(conn)
        .await?)
}

pub async fn soft_delete_space<C>(
    conn: &C,
    space_id: i32,
) -> Result<Option<shared_space::Model>, DbError>
where
    C: ConnectionTrait,
{
    let Some(row) = shared_space::Entity::find_by_id(space_id)
        .filter(shared_space::Column::DeletedAt.is_null())
        .one(conn)
        .await?
    else {
        return Ok(None);
    };
    let now = Utc::now();
    let mut active = row.into_active_model();
    active.updated_at = Set(now);
    active.deleted_at = Set(Some(now));
    Ok(Some(active.update(conn).await?))
}

pub async fn create_message<C>(
    conn: &C,
    input: NewSharedSpaceMessage<'_>,
) -> Result<shared_space_message::Model, DbError>
where
    C: ConnectionTrait,
{
    let active = shared_space_message::ActiveModel {
        id: NotSet,
        space_id: Set(input.space_id),
        binding_id: Set(input.binding_id),
        role: Set(input.role.to_string()),
        content: Set(input.content.to_string()),
        source_agent_type: Set(input.source_agent_type.map(agent_type_to_db)),
        native_message_id: Set(input.native_message_id.map(str::to_string)),
        created_at: Set(Utc::now()),
    };
    Ok(active.insert(conn).await?)
}

pub async fn list_messages<C>(
    conn: &C,
    space_id: i32,
    limit: u64,
) -> Result<Vec<SharedSpaceMessageSummary>, DbError>
where
    C: ConnectionTrait,
{
    let rows = shared_space_message::Entity::find()
        .filter(shared_space_message::Column::SpaceId.eq(space_id))
        .order_by_desc(shared_space_message::Column::CreatedAt)
        .order_by_desc(shared_space_message::Column::Id)
        .limit(limit)
        .all(conn)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows.into_iter().rev() {
        out.push(SharedSpaceMessageSummary {
            message_id: row.id,
            space_id: row.space_id,
            binding_id: row.binding_id,
            role: row.role,
            content: row.content,
            source_agent_type: optional_agent_type_from_db(row.source_agent_type)?,
            native_message_id: row.native_message_id,
            created_at: row.created_at,
        });
    }
    Ok(out)
}

pub async fn messages_count<C>(conn: &C, space_id: i32) -> Result<u64, DbError>
where
    C: ConnectionTrait,
{
    Ok(shared_space_message::Entity::find()
        .filter(shared_space_message::Column::SpaceId.eq(space_id))
        .count(conn)
        .await?)
}

pub async fn get_context_snapshot<C>(
    conn: &C,
    space_id: i32,
) -> Result<Option<shared_space_context_snapshot::Model>, DbError>
where
    C: ConnectionTrait,
{
    Ok(shared_space_context_snapshot::Entity::find()
        .filter(shared_space_context_snapshot::Column::SpaceId.eq(space_id))
        .one(conn)
        .await?)
}

pub async fn upsert_context_snapshot<C>(
    conn: &C,
    space_id: i32,
    summary: &str,
    recent_messages_json: &str,
) -> Result<shared_space_context_snapshot::Model, DbError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    if let Some(row) = shared_space_context_snapshot::Entity::find()
        .filter(shared_space_context_snapshot::Column::SpaceId.eq(space_id))
        .one(conn)
        .await?
    {
        let mut active = row.into_active_model();
        active.summary = Set(summary.to_string());
        active.recent_messages_json = Set(recent_messages_json.to_string());
        active.updated_at = Set(now);
        return Ok(active.update(conn).await?);
    }

    let active = shared_space_context_snapshot::ActiveModel {
        id: NotSet,
        space_id: Set(space_id),
        summary: Set(summary.to_string()),
        recent_messages_json: Set(recent_messages_json.to_string()),
        updated_at: Set(now),
    };
    Ok(active.insert(conn).await?)
}

pub async fn list_spaces<C>(conn: &C) -> Result<Vec<SharedSpaceSummary>, DbError>
where
    C: ConnectionTrait,
{
    let rows = shared_space::Entity::find()
        .filter(shared_space::Column::DeletedAt.is_null())
        .order_by_desc(shared_space::Column::UpdatedAt)
        .order_by_desc(shared_space::Column::Id)
        .all(conn)
        .await?;

    let mut summaries = Vec::with_capacity(rows.len());
    for space in rows {
        let Some(folder) = folder::Entity::find_by_id(space.folder_id)
            .filter(folder::Column::DeletedAt.is_null())
            .one(conn)
            .await?
        else {
            continue;
        };

        let shared_conversations = list_shared_conversations(conn, space.id).await?;
        let mut conversations = Vec::with_capacity(shared_conversations.len());
        let mut active_agent_type = None;
        let mut active_conversation_id = None;

        for shared_conversation in shared_conversations {
            let conversation =
                conversation::Entity::find_by_id(shared_conversation.conversation_id)
                    .filter(conversation::Column::DeletedAt.is_null())
                    .one(conn)
                    .await?;
            let Some(conversation) = conversation else {
                continue;
            };
            let agent_sessions =
                list_conversation_agent_sessions(conn, shared_conversation.id).await?;
            let mut agents = Vec::with_capacity(agent_sessions.len());
            for session in agent_sessions {
                agents.push(SharedSpaceConversationAgentSummary {
                    agent_type: agent_type_from_db(&session.agent_type)?,
                    native_session_id: session.native_session_id,
                    last_used_at: session.last_used_at,
                    created_at: session.created_at,
                    updated_at: session.updated_at,
                });
            }
            let conversation_agent = agent_type_from_db(&shared_conversation.active_agent_type)?;
            if Some(shared_conversation.id) == space.active_shared_conversation_id {
                active_agent_type = Some(conversation_agent);
                active_conversation_id = Some(conversation.id);
            }
            conversations.push(SharedSpaceConversationSummary {
                shared_conversation_id: shared_conversation.id,
                conversation_id: conversation.id,
                conversation_title: shared_conversation.title.or(conversation.title),
                active_agent_type: conversation_agent,
                agents,
                created_at: shared_conversation.created_at,
                updated_at: shared_conversation.updated_at,
            });
        }

        summaries.push(SharedSpaceSummary {
            space_id: space.id,
            name: space.name,
            folder_id: space.folder_id,
            folder_name: folder.name,
            folder_path: folder.path,
            active_agent_type,
            active_shared_conversation_id: space.active_shared_conversation_id,
            active_conversation_id,
            conversations,
            messages_count: messages_count(conn, space.id).await?,
            created_at: space.created_at,
            updated_at: space.updated_at,
        });
    }

    Ok(summaries)
}

// Legacy binding helpers are kept for compatibility with old shared-space
// message rows and pre-migration data. New shared-space orchestration uses
// `shared_space_conversation(_agent)` instead.
pub async fn create_binding<C>(
    conn: &C,
    space_id: i32,
    agent_type: AgentType,
    conversation_id: Option<i32>,
) -> Result<shared_space_agent_binding::Model, DbError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    let active = shared_space_agent_binding::ActiveModel {
        id: NotSet,
        space_id: Set(space_id),
        agent_type: Set(agent_type_to_db(agent_type)),
        conversation_id: Set(conversation_id),
        native_session_id: Set(None),
        last_used_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
    };
    Ok(active.insert(conn).await?)
}

pub async fn set_active_binding<C>(
    conn: &C,
    space_id: i32,
    binding_id: i32,
) -> Result<shared_space::Model, DbError>
where
    C: ConnectionTrait,
{
    let row = shared_space::Entity::find_by_id(space_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Shared space not found: {space_id}")))?;
    let mut active = row.into_active_model();
    active.active_agent_binding_id = Set(Some(binding_id));
    active.updated_at = Set(Utc::now());
    Ok(active.update(conn).await?)
}
