use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, Set,
};

use crate::db::entities::{conversation, folder, shared_space, shared_space_conversation};
use crate::db::error::DbError;
use crate::models::{AgentType, DbConversationSummary};

pub async fn create(
    conn: &DatabaseConnection,
    folder_id: i32,
    agent_type: AgentType,
    title: Option<String>,
    git_branch: Option<String>,
) -> Result<conversation::Model, DbError> {
    create_with_delegation(conn, folder_id, agent_type, title, git_branch, None).await
}

/// Mirror of [`create`] plus optional delegation linkage. Used by the
/// multi-agent broker when spawning a child sub-session — populates
/// `parent_id` / `parent_tool_use_id` / `delegation_call_id` so the lifecycle
/// subscriber and frontend can rebuild the parent ↔ child binding without
/// inspecting the live broker state.
pub async fn create_with_delegation(
    conn: &DatabaseConnection,
    folder_id: i32,
    agent_type: AgentType,
    title: Option<String>,
    git_branch: Option<String>,
    delegation: Option<crate::acp::delegation::spawner::DelegationLink>,
) -> Result<conversation::Model, DbError> {
    let at_str = serde_json::to_value(agent_type)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let now = Utc::now();
    let (parent_id, parent_tool_use_id, delegation_call_id, delegation_role) = match delegation {
        Some(link) => (
            Some(link.parent_conversation_id),
            Some(link.parent_tool_use_id),
            Some(link.delegation_call_id),
            link.role,
        ),
        None => (None, None, None, None),
    };
    let title = title.or(delegation_role);
    let model = conversation::ActiveModel {
        id: NotSet,
        folder_id: Set(folder_id),
        title: Set(title),
        title_locked: Set(false),
        agent_type: Set(at_str),
        status: Set(conversation::ConversationStatus::InProgress),
        model: Set(None),
        git_branch: Set(git_branch),
        external_id: Set(None),
        parent_id: Set(parent_id),
        parent_tool_use_id: Set(parent_tool_use_id),
        delegation_call_id: Set(delegation_call_id),
        message_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        pinned_at: Set(None),
    };
    Ok(model.insert(conn).await?)
}

pub async fn update_status(
    conn: &DatabaseConnection,
    conversation_id: i32,
    status: conversation::ConversationStatus,
) -> Result<(), DbError> {
    let conv = conversation::Entity::find_by_id(conversation_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Conversation not found: {conversation_id}")))?;
    let mut active: conversation::ActiveModel = conv.into();
    active.status = Set(status);
    active.updated_at = Set(Utc::now());
    active.update(conn).await?;
    Ok(())
}

/// Refresh the delegation linkage on an already-linked long-lived collaborator
/// conversation before sending its next delegated task. The lifecycle resolver
/// reads `delegation_call_id` from the child conversation at TurnComplete, so a
/// reused collaborator must point at the latest broker task id for each turn.
pub async fn update_delegation_link(
    conn: &DatabaseConnection,
    conversation_id: i32,
    link: crate::acp::delegation::spawner::DelegationLink,
) -> Result<(), DbError> {
    let conv = conversation::Entity::find_by_id(conversation_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Conversation not found: {conversation_id}")))?;
    let should_fill_title = !conv.title_locked
        && conv
            .title
            .as_ref()
            .map(|t| t.trim().is_empty() || t == "加载中...")
            .unwrap_or(true);
    let mut active: conversation::ActiveModel = conv.into();
    active.parent_id = Set(Some(link.parent_conversation_id));
    active.parent_tool_use_id = Set(Some(link.parent_tool_use_id));
    active.delegation_call_id = Set(Some(link.delegation_call_id));
    if should_fill_title {
        if let Some(role) = link.role {
            active.title = Set(Some(role));
        }
    }
    active.updated_at = Set(Utc::now());
    active.update(conn).await?;
    Ok(())
}

/// Conditional status transition (CAS): write `new_status` only if the row's
/// current `status` equals `expected`. Returns `true` when the row was
/// updated. Used by the lifecycle subscriber on disconnect/error so a
/// concurrent user-driven `completed` (or a prior `pending_review` from
/// `TurnComplete`) cannot be silently overwritten.
pub async fn update_status_if(
    conn: &DatabaseConnection,
    conversation_id: i32,
    expected: conversation::ConversationStatus,
    new_status: conversation::ConversationStatus,
) -> Result<bool, DbError> {
    use sea_orm::sea_query::Expr;
    let result = conversation::Entity::update_many()
        .col_expr(conversation::Column::Status, Expr::value(new_status))
        .col_expr(conversation::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(conversation::Column::Id.eq(conversation_id))
        .filter(conversation::Column::Status.eq(expected))
        .exec(conn)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Manual rename: set the title AND lock it. Once locked, the per-turn
/// auto-title backfill ([`refresh_auto_title`]) leaves this row alone, so the
/// user's hand-picked name survives every subsequent session-file parse.
pub async fn update_title(
    conn: &DatabaseConnection,
    conversation_id: i32,
    title: String,
) -> Result<(), DbError> {
    let conv = conversation::Entity::find_by_id(conversation_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Conversation not found: {conversation_id}")))?;
    let mut active: conversation::ActiveModel = conv.into();
    active.title = Set(Some(title));
    active.title_locked = Set(true);
    active.updated_at = Set(Utc::now());
    active.update(conn).await?;
    Ok(())
}

/// Auto-derive counterpart to [`update_title`]: write `title` ONLY when the row
/// is not user-locked and the value actually changed. Never sets `title_locked`
/// (the title stays eligible for future auto-refreshes, e.g. when an agent like
/// OpenCode regenerates its own session title) and deliberately does NOT bump
/// `updated_at` — a title backfill is metadata, not user activity, so it must
/// not float the row to the top of a recency-sorted sidebar. Returns `true`
/// when a row was written so the caller can broadcast a sidebar upsert.
///
/// Implemented as a single conditional UPDATE (`... WHERE id = ? AND
/// title_locked = false AND (title IS NULL OR title <> ?)`) so the lock/equality
/// checks and the write are atomic: a manual rename ([`update_title`], which
/// sets `title_locked = true`) that lands between a would-be read and the write
/// can never be clobbered, because the lock predicate is re-evaluated at write
/// time by the database. A non-existent row simply matches nothing (`false`).
pub async fn refresh_auto_title(
    conn: &DatabaseConnection,
    conversation_id: i32,
    title: String,
) -> Result<bool, DbError> {
    use sea_orm::sea_query::Expr;
    let title = title.trim();
    if title.is_empty() {
        return Ok(false);
    }
    let res = conversation::Entity::update_many()
        .col_expr(conversation::Column::Title, Expr::value(title))
        .filter(conversation::Column::Id.eq(conversation_id))
        .filter(conversation::Column::TitleLocked.eq(false))
        .filter(
            sea_orm::Condition::any()
                .add(conversation::Column::Title.is_null())
                .add(conversation::Column::Title.ne(title)),
        )
        .exec(conn)
        .await?;
    Ok(res.rows_affected > 0)
}

/// Pin or unpin a conversation. Sets `pinned_at = now()` when pinning, `NULL`
/// when unpinning. Only the `pinned_at` column is written — `updated_at` is
/// deliberately left untouched (SeaORM updates only the `Set` field), because
/// pinning is a view preference, not conversation activity, and must not float
/// the row to the top of a recency-sorted sidebar (same reasoning as
/// [`refresh_auto_title`]). The sidebar's "Pinned" section orders by `pinned_at`
/// descending, so a freshly pinned conversation jumps to the top.
pub async fn update_pin(
    conn: &DatabaseConnection,
    conversation_id: i32,
    pinned: bool,
) -> Result<(), DbError> {
    let conv = conversation::Entity::find_by_id(conversation_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Conversation not found: {conversation_id}")))?;
    let mut active: conversation::ActiveModel = conv.into();
    active.pinned_at = Set(pinned.then(Utc::now));
    active.update(conn).await?;
    Ok(())
}

pub async fn update_external_id(
    conn: &DatabaseConnection,
    conversation_id: i32,
    external_id: String,
) -> Result<(), DbError> {
    let conv = conversation::Entity::find_by_id(conversation_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Conversation not found: {conversation_id}")))?;
    let mut active: conversation::ActiveModel = conv.into();
    active.external_id = Set(Some(external_id));
    active.updated_at = Set(Utc::now());
    active.update(conn).await?;
    Ok(())
}

pub async fn update_agent_type_and_external_id(
    conn: &DatabaseConnection,
    conversation_id: i32,
    agent_type: AgentType,
    external_id: Option<String>,
) -> Result<(), DbError> {
    let conv = conversation::Entity::find_by_id(conversation_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Conversation not found: {conversation_id}")))?;
    let at_str = serde_json::to_value(agent_type)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let mut active: conversation::ActiveModel = conv.into();
    active.agent_type = Set(at_str);
    active.external_id = Set(external_id);
    active.updated_at = Set(Utc::now());
    active.update(conn).await?;
    Ok(())
}

pub async fn soft_delete(conn: &DatabaseConnection, conversation_id: i32) -> Result<(), DbError> {
    let conv = conversation::Entity::find_by_id(conversation_id)
        .filter(conversation::Column::DeletedAt.is_null())
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Conversation not found: {conversation_id}")))?;
    let mut active: conversation::ActiveModel = conv.into();
    active.deleted_at = Set(Some(Utc::now()));
    active.update(conn).await?;
    Ok(())
}

fn parse_agent_type(s: &str) -> AgentType {
    match serde_json::from_value(serde_json::Value::String(s.to_string())) {
        Ok(at) => at,
        Err(_) => {
            // DB has a value the enum does not recognise (manual edit or removed variant).
            // Fall back to ClaudeCode so the row stays readable, but log so resume-as-wrong-agent
            // regressions are traceable.
            eprintln!(
                "[conversation_service] unknown agent_type {s:?} in DB, falling back to ClaudeCode"
            );
            AgentType::ClaudeCode
        }
    }
}

#[derive(Clone, Copy)]
struct SharedConversationMarker {
    shared_space_id: i32,
    shared_conversation_id: i32,
}

async fn shared_conversation_markers(
    conn: &DatabaseConnection,
    conversation_ids: &[i32],
) -> Result<HashMap<i32, SharedConversationMarker>, DbError> {
    if conversation_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let unique_ids: Vec<i32> = conversation_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let rows = shared_space_conversation::Entity::find()
        .filter(shared_space_conversation::Column::ConversationId.is_in(unique_ids))
        .filter(shared_space_conversation::Column::DeletedAt.is_null())
        .all(conn)
        .await?;

    if rows.is_empty() {
        return Ok(HashMap::new());
    }

    // A removed shared space should release its conversations back to the normal
    // folder list. Only mark conversations whose owning shared space is still
    // active.
    let space_ids: Vec<i32> = rows
        .iter()
        .map(|row| row.space_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let active_space_ids: HashSet<i32> = shared_space::Entity::find()
        .filter(shared_space::Column::Id.is_in(space_ids))
        .filter(shared_space::Column::DeletedAt.is_null())
        .all(conn)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect();

    let mut markers = HashMap::new();
    for row in rows {
        if active_space_ids.contains(&row.space_id) {
            markers.insert(
                row.conversation_id,
                SharedConversationMarker {
                    shared_space_id: row.space_id,
                    shared_conversation_id: row.id,
                },
            );
        }
    }
    Ok(markers)
}

async fn convs_to_summaries(
    conn: &DatabaseConnection,
    rows: Vec<conversation::Model>,
) -> Result<Vec<DbConversationSummary>, DbError> {
    let ids: Vec<i32> = rows.iter().map(|row| row.id).collect();
    let markers = shared_conversation_markers(conn, &ids).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let marker = markers.get(&row.id).copied();
            conv_to_summary(row, marker)
        })
        .collect())
}

fn conv_to_summary(
    r: conversation::Model,
    shared_marker: Option<SharedConversationMarker>,
) -> DbConversationSummary {
    let status = serde_json::to_value(&r.status)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| format!("{:?}", r.status));
    DbConversationSummary {
        id: r.id,
        folder_id: r.folder_id,
        title: r.title,
        title_locked: r.title_locked,
        agent_type: parse_agent_type(&r.agent_type),
        status,
        model: r.model,
        git_branch: r.git_branch,
        external_id: r.external_id,
        message_count: r.message_count as u32,
        created_at: r.created_at,
        updated_at: r.updated_at,
        pinned_at: r.pinned_at,
        shared_space_id: shared_marker.map(|marker| marker.shared_space_id),
        shared_conversation_id: shared_marker.map(|marker| marker.shared_conversation_id),
        parent_id: r.parent_id,
        parent_tool_use_id: r.parent_tool_use_id,
        delegation_call_id: r.delegation_call_id,
    }
}

pub async fn get_by_id(
    conn: &DatabaseConnection,
    conversation_id: i32,
) -> Result<DbConversationSummary, DbError> {
    let conv = conversation::Entity::find_by_id(conversation_id)
        .filter(conversation::Column::DeletedAt.is_null())
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Conversation not found: {conversation_id}")))?;

    let marker = shared_conversation_markers(conn, &[conv.id])
        .await?
        .remove(&conv.id);
    Ok(conv_to_summary(conv, marker))
}

/// Look up a child conversation by its `delegation_call_id` (the broker's
/// `task_id`). Returns `Ok(None)` when no row matches — used by the broker's
/// `ChildStatusLookup` DB fallback to recover a delegation task's terminal
/// status after its in-memory result was evicted from the completed-cache.
/// Unlike [`get_by_id`] this never errors hard on "not found": a missing row
/// is a legitimate "unknown task" answer.
pub async fn get_by_delegation_call_id(
    conn: &DatabaseConnection,
    delegation_call_id: &str,
) -> Result<Option<DbConversationSummary>, DbError> {
    let conv = conversation::Entity::find()
        .filter(conversation::Column::DelegationCallId.eq(delegation_call_id))
        .filter(conversation::Column::DeletedAt.is_null())
        .one(conn)
        .await?;
    if let Some(conv) = conv {
        let marker = shared_conversation_markers(conn, &[conv.id])
            .await?
            .remove(&conv.id);
        Ok(Some(conv_to_summary(conv, marker)))
    } else {
        Ok(None)
    }
}

pub async fn list_by_folder(
    conn: &DatabaseConnection,
    folder_id: i32,
    agent_type: Option<AgentType>,
    search: Option<String>,
    sort_by: Option<String>,
    status: Option<String>,
) -> Result<Vec<DbConversationSummary>, DbError> {
    let mut query = conversation::Entity::find()
        .filter(conversation::Column::FolderId.eq(folder_id))
        .filter(conversation::Column::DeletedAt.is_null());

    // Filter by agent_type
    if let Some(ref at) = agent_type {
        let at_str = serde_json::to_value(at)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        query = query.filter(conversation::Column::AgentType.eq(at_str));
    }

    // Search by title
    if let Some(ref s) = search {
        if !s.is_empty() {
            query = query.filter(conversation::Column::Title.contains(s));
        }
    }

    // Filter by status
    if let Some(ref st) = status {
        if let Ok(status_enum) = serde_json::from_value::<conversation::ConversationStatus>(
            serde_json::Value::String(st.clone()),
        ) {
            query = query.filter(conversation::Column::Status.eq(status_enum));
        }
    }

    // Sort
    query = match sort_by.as_deref() {
        Some("oldest") => query.order_by_asc(conversation::Column::CreatedAt),
        _ => query.order_by_desc(conversation::Column::CreatedAt),
    };

    let rows = query.all(conn).await?;

    convs_to_summaries(conn, rows).await
}

/// List conversations across folders. When `folder_ids` is `None`, queries all
/// When `folder_ids` is provided, results are scoped to that set. Otherwise
/// returns conversations across every non-deleted folder (open or not).
///
/// `include_children` controls visibility of delegation sub-sessions. When
/// `false` (the default for the top-level list), rows whose `parent_id` is
/// non-null are filtered out — they belong to their parent's tool-call view,
/// not the workspace conversation list.
pub async fn list_all(
    conn: &DatabaseConnection,
    folder_ids: Option<Vec<i32>>,
    agent_type: Option<AgentType>,
    search: Option<String>,
    sort_by: Option<String>,
    status: Option<String>,
    include_children: bool,
) -> Result<Vec<DbConversationSummary>, DbError> {
    let mut query = conversation::Entity::find().filter(conversation::Column::DeletedAt.is_null());

    if !include_children {
        query = query.filter(conversation::Column::ParentId.is_null());
    }

    match folder_ids {
        Some(ids) if !ids.is_empty() => {
            query = query.filter(conversation::Column::FolderId.is_in(ids));
        }
        _ => {
            // Exclude conversations whose folder was soft-deleted.
            let active_folder_ids: Vec<i32> = folder::Entity::find()
                .filter(folder::Column::DeletedAt.is_null())
                .all(conn)
                .await?
                .into_iter()
                .map(|m| m.id)
                .collect();
            if active_folder_ids.is_empty() {
                return Ok(Vec::new());
            }
            query = query.filter(conversation::Column::FolderId.is_in(active_folder_ids));
        }
    }

    if let Some(ref at) = agent_type {
        let at_str = serde_json::to_value(at)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        query = query.filter(conversation::Column::AgentType.eq(at_str));
    }

    if let Some(ref s) = search {
        if !s.is_empty() {
            query = query.filter(conversation::Column::Title.contains(s));
        }
    }

    if let Some(ref st) = status {
        if let Ok(status_enum) = serde_json::from_value::<conversation::ConversationStatus>(
            serde_json::Value::String(st.clone()),
        ) {
            query = query.filter(conversation::Column::Status.eq(status_enum));
        }
    }

    query = match sort_by.as_deref() {
        Some("oldest") => query.order_by_asc(conversation::Column::UpdatedAt),
        _ => query.order_by_desc(conversation::Column::UpdatedAt),
    };

    let rows = query.all(conn).await?;
    convs_to_summaries(conn, rows).await
}

/// List delegation children of a single parent conversation, oldest first.
/// Returns rows where `parent_id == parent_conversation_id`. Soft-deleted
/// children are filtered out so a removed sub-session stays hidden in the
/// parent's tool-call view too.
pub async fn list_children(
    conn: &DatabaseConnection,
    parent_conversation_id: i32,
) -> Result<Vec<DbConversationSummary>, DbError> {
    let rows = conversation::Entity::find()
        .filter(conversation::Column::ParentId.eq(parent_conversation_id))
        .filter(conversation::Column::DeletedAt.is_null())
        .order_by_asc(conversation::Column::CreatedAt)
        .all(conn)
        .await?;
    convs_to_summaries(conn, rows).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::delegation::spawner::DelegationLink;
    use crate::db::service::shared_space_service;
    use crate::db::test_helpers::{fresh_in_memory_db, seed_folder};

    /// Build a parent + a delegation child for filter assertions.
    async fn seed_parent_with_child(conn: &DatabaseConnection, folder_id: i32) -> (i32, i32) {
        let parent = create(
            conn,
            folder_id,
            AgentType::ClaudeCode,
            Some("P".into()),
            None,
        )
        .await
        .expect("parent");
        let link = DelegationLink {
            parent_conversation_id: parent.id,
            parent_tool_use_id: "tu-1".into(),
            delegation_call_id: "call-1".into(),
            role: None,
        };
        let child = create_with_delegation(
            conn,
            folder_id,
            AgentType::Codex,
            Some("C".into()),
            None,
            Some(link),
        )
        .await
        .expect("child");
        (parent.id, child.id)
    }

    #[tokio::test]
    async fn list_all_excludes_children_by_default() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-list-children-default").await;
        let (parent, _child) = seed_parent_with_child(&db.conn, folder).await;

        let rows = list_all(&db.conn, None, None, None, None, None, false)
            .await
            .expect("list");
        let ids: Vec<i32> = rows.iter().map(|r| r.id).collect();
        assert!(ids.contains(&parent), "parent must remain visible: {ids:?}");
        assert_eq!(
            rows.len(),
            1,
            "expected only the parent, got {} rows: {ids:?}",
            rows.len()
        );
    }

    #[tokio::test]
    async fn list_all_includes_children_when_requested() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-list-children-on").await;
        let (parent, child) = seed_parent_with_child(&db.conn, folder).await;

        let rows = list_all(&db.conn, None, None, None, None, None, true)
            .await
            .expect("list");
        let ids: Vec<i32> = rows.iter().map(|r| r.id).collect();
        assert!(
            ids.contains(&parent) && ids.contains(&child),
            "both parent + child must appear when include_children=true, got: {ids:?}",
        );
    }

    #[tokio::test]
    async fn list_children_returns_only_matching_parent() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-list-children-only").await;
        let (parent_a, child_a) = seed_parent_with_child(&db.conn, folder).await;
        let (_parent_b, _child_b) = seed_parent_with_child(&db.conn, folder).await;

        let rows = list_children(&db.conn, parent_a).await.expect("list");
        assert_eq!(
            rows.len(),
            1,
            "expected 1 child of parent_a, got {}",
            rows.len()
        );
        assert_eq!(rows[0].id, child_a);
        assert_eq!(rows[0].parent_id, Some(parent_a));
    }

    #[tokio::test]
    async fn summaries_mark_active_shared_space_conversations() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-shared-marker").await;
        let normal = create(
            &db.conn,
            folder,
            AgentType::ClaudeCode,
            Some("normal".into()),
            None,
        )
        .await
        .expect("normal");
        let shared = create(
            &db.conn,
            folder,
            AgentType::Codex,
            Some("shared".into()),
            None,
        )
        .await
        .expect("shared");
        let space = shared_space_service::create_space(&db.conn, "S", folder)
            .await
            .expect("space");
        let shared_conversation = shared_space_service::create_shared_conversation(
            &db.conn,
            space.id,
            shared.id,
            Some("shared".into()),
            AgentType::Codex,
        )
        .await
        .expect("shared conversation");

        let rows = list_all(&db.conn, None, None, None, None, None, false)
            .await
            .expect("list");
        let normal_summary = rows
            .iter()
            .find(|row| row.id == normal.id)
            .expect("normal summary");
        assert_eq!(normal_summary.shared_space_id, None);
        assert_eq!(normal_summary.shared_conversation_id, None);
        let shared_summary = rows
            .iter()
            .find(|row| row.id == shared.id)
            .expect("shared summary");
        assert_eq!(shared_summary.shared_space_id, Some(space.id));
        assert_eq!(
            shared_summary.shared_conversation_id,
            Some(shared_conversation.id)
        );

        shared_space_service::soft_delete_space(&db.conn, space.id)
            .await
            .expect("remove shared space");
        let released = get_by_id(&db.conn, shared.id)
            .await
            .expect("shared after remove");
        assert_eq!(released.shared_space_id, None);
        assert_eq!(released.shared_conversation_id, None);
    }

    #[tokio::test]
    async fn update_pin_sets_and_clears_without_bumping_updated_at() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-update-pin").await;
        let conv = create(
            &db.conn,
            folder,
            AgentType::ClaudeCode,
            Some("c".into()),
            None,
        )
        .await
        .expect("create");

        // Freshly created rows are unpinned, and the summary projection carries
        // the field through (conv_to_summary mapping).
        let before = get_by_id(&db.conn, conv.id).await.expect("get before");
        assert!(
            before.pinned_at.is_none(),
            "new conversation must be unpinned"
        );
        let updated_at_before = before.updated_at;

        // Pin → pinned_at populated; updated_at must NOT move (pin is a view
        // preference, not activity).
        update_pin(&db.conn, conv.id, true).await.expect("pin");
        let pinned = get_by_id(&db.conn, conv.id).await.expect("get pinned");
        assert!(
            pinned.pinned_at.is_some(),
            "pinned_at must be set after pin"
        );
        assert_eq!(
            pinned.updated_at, updated_at_before,
            "pinning must not bump updated_at"
        );

        // Unpin → pinned_at cleared back to NULL; updated_at still unchanged.
        update_pin(&db.conn, conv.id, false).await.expect("unpin");
        let unpinned = get_by_id(&db.conn, conv.id).await.expect("get unpinned");
        assert!(
            unpinned.pinned_at.is_none(),
            "pinned_at must clear after unpin"
        );
        assert_eq!(
            unpinned.updated_at, updated_at_before,
            "unpinning must not bump updated_at"
        );
    }

    #[tokio::test]
    async fn list_children_excludes_soft_deleted() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-list-children-soft-del").await;
        let (parent, child) = seed_parent_with_child(&db.conn, folder).await;

        soft_delete(&db.conn, child).await.expect("soft delete");

        let rows = list_children(&db.conn, parent).await.expect("list");
        assert!(
            rows.is_empty(),
            "soft-deleted child must not appear: {rows:?}"
        );
    }

    #[tokio::test]
    async fn create_leaves_title_unlocked() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-title-unlocked").await;
        let row = create(
            &db.conn,
            folder,
            AgentType::ClaudeCode,
            Some("hi".into()),
            None,
        )
        .await
        .expect("create");
        let summary = get_by_id(&db.conn, row.id).await.expect("get");
        assert!(
            !summary.title_locked,
            "new conversation must start unlocked"
        );
    }

    #[tokio::test]
    async fn update_title_locks_the_title() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-title-lock").await;
        let row = create(&db.conn, folder, AgentType::ClaudeCode, None, None)
            .await
            .expect("create");
        update_title(&db.conn, row.id, "My name".into())
            .await
            .expect("rename");
        let summary = get_by_id(&db.conn, row.id).await.expect("get");
        assert_eq!(summary.title.as_deref(), Some("My name"));
        assert!(summary.title_locked, "manual rename must lock the title");
    }

    #[tokio::test]
    async fn refresh_auto_title_writes_when_unlocked_and_changed() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-title-auto").await;
        let row = create(
            &db.conn,
            folder,
            AgentType::ClaudeCode,
            Some("old".into()),
            None,
        )
        .await
        .expect("create");

        let wrote = refresh_auto_title(&db.conn, row.id, "fresh".into())
            .await
            .expect("auto");
        assert!(wrote, "an unlocked, changed title must be written");

        let summary = get_by_id(&db.conn, row.id).await.expect("get");
        assert_eq!(summary.title.as_deref(), Some("fresh"));
        assert!(
            !summary.title_locked,
            "auto refresh must NOT lock — the title stays eligible for future refreshes"
        );
    }

    #[tokio::test]
    async fn refresh_auto_title_skips_when_unchanged_or_empty() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-title-auto-skip").await;
        let row = create(
            &db.conn,
            folder,
            AgentType::ClaudeCode,
            Some("same".into()),
            None,
        )
        .await
        .expect("create");

        assert!(
            !refresh_auto_title(&db.conn, row.id, "same".into())
                .await
                .expect("auto-same"),
            "identical title must be a no-op"
        );
        assert!(
            !refresh_auto_title(&db.conn, row.id, String::new())
                .await
                .expect("auto-empty"),
            "empty title must be a no-op"
        );
    }

    #[tokio::test]
    async fn refresh_auto_title_never_clobbers_a_locked_title() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-title-auto-locked").await;
        let row = create(&db.conn, folder, AgentType::ClaudeCode, None, None)
            .await
            .expect("create");
        update_title(&db.conn, row.id, "User pick".into())
            .await
            .expect("rename");

        let wrote = refresh_auto_title(&db.conn, row.id, "parser title".into())
            .await
            .expect("auto");
        assert!(!wrote, "a locked title must never be auto-overwritten");
        let summary = get_by_id(&db.conn, row.id).await.expect("get");
        assert_eq!(summary.title.as_deref(), Some("User pick"));
        assert!(summary.title_locked);
    }

    #[tokio::test]
    async fn refresh_auto_title_does_not_bump_updated_at() {
        let db = fresh_in_memory_db().await;
        let folder = seed_folder(&db, "/tmp/billow-title-no-bump").await;
        let row = create(
            &db.conn,
            folder,
            AgentType::ClaudeCode,
            Some("old".into()),
            None,
        )
        .await
        .expect("create");
        let before = row.updated_at;

        let wrote = refresh_auto_title(&db.conn, row.id, "fresh".into())
            .await
            .expect("auto");
        assert!(wrote);

        let summary = get_by_id(&db.conn, row.id).await.expect("get");
        assert_eq!(summary.title.as_deref(), Some("fresh"));
        assert_eq!(
            summary.updated_at, before,
            "auto-title backfill is metadata, not activity — it must not bump updated_at"
        );
    }
}
