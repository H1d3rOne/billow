use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, EntityTrait,
    IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder, Set,
};

use crate::db::entities::{conversation, folder, project, project_role};
use crate::db::error::DbError;
use crate::models::{AgentType, CollaborativeProjectRoleSummary, CollaborativeProjectSummary};

pub struct NewProjectRole<'a> {
    pub name: &'a str,
    pub role_key: &'a str,
    pub agent_type: AgentType,
    pub instructions: Option<&'a str>,
    pub is_manager: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct ProjectRoleBinding {
    pub project_id: i32,
    pub role_id: i32,
    pub role_key: String,
    pub name: String,
    pub agent_type: AgentType,
    pub instructions: Option<String>,
    pub is_manager: bool,
    pub working_dir: String,
    pub conversation_id: Option<i32>,
    pub conversation_external_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CollaborativeProjectRoleContext {
    pub project_id: i32,
    pub project_name: String,
    pub manager_conversation_id: i32,
    pub working_dir: String,
    pub roles: Vec<ProjectRoleBinding>,
}

fn normalize_role_ref(value: &str) -> String {
    value.trim().to_lowercase()
}

fn role_matches_requested(role: &project_role::Model, requested: &str) -> bool {
    let requested = normalize_role_ref(requested);
    if requested.is_empty() {
        return false;
    }
    normalize_role_ref(&role.role_key) == requested || normalize_role_ref(&role.name) == requested
}

async fn active_conversation_external_id<C>(
    conn: &C,
    conversation_id: Option<i32>,
) -> Result<(Option<i32>, Option<String>), DbError>
where
    C: ConnectionTrait,
{
    let Some(conversation_id) = conversation_id else {
        return Ok((None, None));
    };
    let conversation = conversation::Entity::find_by_id(conversation_id)
        .filter(conversation::Column::DeletedAt.is_null())
        .one(conn)
        .await?;
    Ok(match conversation {
        Some(row) => (Some(conversation_id), row.external_id),
        None => (None, None),
    })
}

fn agent_type_to_db(agent_type: AgentType) -> String {
    serde_json::to_value(agent_type)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn agent_type_from_db(raw: &str) -> Result<AgentType, DbError> {
    serde_json::from_value(serde_json::Value::String(raw.to_string()))
        .map_err(|e| DbError::Migration(format!("Invalid project role agent type: {e}")))
}

pub async fn create_project<C>(
    conn: &C,
    name: &str,
    folder_id: i32,
) -> Result<project::Model, DbError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    let active = project::ActiveModel {
        id: NotSet,
        name: Set(name.to_string()),
        folder_id: Set(folder_id),
        manager_role_id: Set(None),
        manager_conversation_id: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
    };
    Ok(active.insert(conn).await?)
}

pub async fn create_project_role<C>(
    conn: &C,
    project_id: i32,
    input: NewProjectRole<'_>,
) -> Result<project_role::Model, DbError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    let active = project_role::ActiveModel {
        id: NotSet,
        project_id: Set(project_id),
        name: Set(input.name.to_string()),
        role_key: Set(input.role_key.to_string()),
        agent_type: Set(agent_type_to_db(input.agent_type)),
        instructions: Set(input.instructions.map(str::to_string)),
        is_manager: Set(input.is_manager),
        conversation_id: Set(None),
        sort_order: Set(input.sort_order),
        created_at: Set(now),
        updated_at: Set(now),
    };
    Ok(active.insert(conn).await?)
}

pub async fn get_project<C>(conn: &C, project_id: i32) -> Result<Option<project::Model>, DbError>
where
    C: ConnectionTrait,
{
    Ok(project::Entity::find_by_id(project_id)
        .filter(project::Column::DeletedAt.is_null())
        .one(conn)
        .await?)
}

pub async fn get_project_role<C>(
    conn: &C,
    role_id: i32,
) -> Result<Option<project_role::Model>, DbError>
where
    C: ConnectionTrait,
{
    Ok(project_role::Entity::find_by_id(role_id).one(conn).await?)
}

pub async fn get_active_conversation<C>(
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

pub async fn count_project_roles<C>(conn: &C, project_id: i32) -> Result<u64, DbError>
where
    C: ConnectionTrait,
{
    Ok(project_role::Entity::find()
        .filter(project_role::Column::ProjectId.eq(project_id))
        .count(conn)
        .await?)
}

pub async fn project_role_key_exists<C>(
    conn: &C,
    project_id: i32,
    role_key: &str,
) -> Result<bool, DbError>
where
    C: ConnectionTrait,
{
    Ok(project_role::Entity::find()
        .filter(project_role::Column::ProjectId.eq(project_id))
        .filter(project_role::Column::RoleKey.eq(role_key))
        .count(conn)
        .await?
        > 0)
}

pub async fn soft_delete_project<C>(
    conn: &C,
    project_id: i32,
) -> Result<Option<project::Model>, DbError>
where
    C: ConnectionTrait,
{
    let Some(row) = project::Entity::find_by_id(project_id)
        .filter(project::Column::DeletedAt.is_null())
        .one(conn)
        .await?
    else {
        return Ok(None);
    };
    let mut active = row.into_active_model();
    let now = Utc::now();
    active.updated_at = Set(now);
    active.deleted_at = Set(Some(now));
    Ok(Some(active.update(conn).await?))
}

pub async fn set_project_manager<C>(
    conn: &C,
    project_id: i32,
    manager_role_id: i32,
    manager_conversation_id: i32,
) -> Result<project::Model, DbError>
where
    C: ConnectionTrait,
{
    let row = project::Entity::find_by_id(project_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Project not found: {project_id}")))?;
    let mut active = row.into_active_model();
    active.manager_role_id = Set(Some(manager_role_id));
    active.manager_conversation_id = Set(Some(manager_conversation_id));
    active.updated_at = Set(Utc::now());
    Ok(active.update(conn).await?)
}

pub async fn set_role_conversation<C>(
    conn: &C,
    role_id: i32,
    conversation_id: i32,
) -> Result<project_role::Model, DbError>
where
    C: ConnectionTrait,
{
    let row = project_role::Entity::find_by_id(role_id)
        .one(conn)
        .await?
        .ok_or_else(|| DbError::Migration(format!("Project role not found: {role_id}")))?;
    let mut active = row.into_active_model();
    active.conversation_id = Set(Some(conversation_id));
    active.updated_at = Set(Utc::now());
    Ok(active.update(conn).await?)
}

pub async fn find_project_role_for_manager_conversation<C>(
    conn: &C,
    manager_conversation_id: i32,
    role_key: &str,
) -> Result<Option<ProjectRoleBinding>, DbError>
where
    C: ConnectionTrait,
{
    let Some(project) = project::Entity::find()
        .filter(project::Column::ManagerConversationId.eq(Some(manager_conversation_id)))
        .filter(project::Column::DeletedAt.is_null())
        .one(conn)
        .await?
    else {
        return Ok(None);
    };

    let role = project_role::Entity::find()
        .filter(project_role::Column::ProjectId.eq(project.id))
        .all(conn)
        .await?
        .into_iter()
        .find(|role| role_matches_requested(role, role_key));
    let Some(role) = role else {
        return Ok(None);
    };

    let folder = folder::Entity::find_by_id(project.folder_id)
        .one(conn)
        .await?
        .ok_or_else(|| {
            DbError::Migration(format!(
                "Folder not found for collaborative project {}",
                project.id
            ))
        })?;

    let (conversation_id, conversation_external_id) =
        active_conversation_external_id(conn, role.conversation_id).await?;

    Ok(Some(ProjectRoleBinding {
        project_id: project.id,
        role_id: role.id,
        role_key: role.role_key,
        name: role.name,
        agent_type: agent_type_from_db(&role.agent_type)?,
        instructions: role.instructions,
        is_manager: role.is_manager,
        working_dir: folder.path,
        conversation_id,
        conversation_external_id,
    }))
}

pub async fn list_project_roles_for_manager_conversation<C>(
    conn: &C,
    manager_conversation_id: i32,
) -> Result<Option<CollaborativeProjectRoleContext>, DbError>
where
    C: ConnectionTrait,
{
    let Some(project) = project::Entity::find()
        .filter(project::Column::ManagerConversationId.eq(Some(manager_conversation_id)))
        .filter(project::Column::DeletedAt.is_null())
        .one(conn)
        .await?
    else {
        return Ok(None);
    };

    let folder = folder::Entity::find_by_id(project.folder_id)
        .one(conn)
        .await?
        .ok_or_else(|| {
            DbError::Migration(format!(
                "Folder not found for collaborative project {}",
                project.id
            ))
        })?;

    let role_rows = project_role::Entity::find()
        .filter(project_role::Column::ProjectId.eq(project.id))
        .order_by_asc(project_role::Column::SortOrder)
        .order_by_asc(project_role::Column::Id)
        .all(conn)
        .await?;

    let mut roles = Vec::with_capacity(role_rows.len());
    for role in role_rows {
        let (conversation_id, conversation_external_id) =
            active_conversation_external_id(conn, role.conversation_id).await?;
        roles.push(ProjectRoleBinding {
            project_id: project.id,
            role_id: role.id,
            role_key: role.role_key,
            name: role.name,
            agent_type: agent_type_from_db(&role.agent_type)?,
            instructions: role.instructions,
            is_manager: role.is_manager,
            working_dir: folder.path.clone(),
            conversation_id,
            conversation_external_id,
        });
    }

    Ok(Some(CollaborativeProjectRoleContext {
        project_id: project.id,
        project_name: project.name,
        manager_conversation_id,
        working_dir: folder.path,
        roles,
    }))
}

pub async fn list_projects<C>(conn: &C) -> Result<Vec<CollaborativeProjectSummary>, DbError>
where
    C: ConnectionTrait,
{
    let project_rows = project::Entity::find()
        .filter(project::Column::DeletedAt.is_null())
        .order_by_desc(project::Column::UpdatedAt)
        .order_by_desc(project::Column::Id)
        .all(conn)
        .await?;

    let mut summaries = Vec::with_capacity(project_rows.len());
    for project in project_rows {
        let (Some(manager_role_id), Some(manager_conversation_id)) =
            (project.manager_role_id, project.manager_conversation_id)
        else {
            continue;
        };

        let Some(folder) = folder::Entity::find_by_id(project.folder_id)
            .filter(folder::Column::DeletedAt.is_null())
            .one(conn)
            .await?
        else {
            continue;
        };

        let Some(manager_role) = project_role::Entity::find_by_id(manager_role_id)
            .one(conn)
            .await?
        else {
            continue;
        };

        let manager_conversation = conversation::Entity::find_by_id(manager_conversation_id)
            .filter(conversation::Column::DeletedAt.is_null())
            .one(conn)
            .await?;

        let role_rows = project_role::Entity::find()
            .filter(project_role::Column::ProjectId.eq(project.id))
            .order_by_asc(project_role::Column::SortOrder)
            .order_by_asc(project_role::Column::Id)
            .all(conn)
            .await?;
        let roles_count = role_rows.len() as u64;
        let mut roles = Vec::with_capacity(role_rows.len());
        for role in role_rows {
            let conversation = match role.conversation_id {
                Some(conversation_id) => {
                    conversation::Entity::find_by_id(conversation_id)
                        .filter(conversation::Column::DeletedAt.is_null())
                        .one(conn)
                        .await?
                }
                None => None,
            };
            roles.push(CollaborativeProjectRoleSummary {
                role_id: role.id,
                name: role.name,
                role_key: role.role_key,
                agent_type: agent_type_from_db(&role.agent_type)?,
                instructions: role.instructions,
                is_manager: role.is_manager,
                conversation_id: conversation.as_ref().map(|row| row.id),
                conversation_title: conversation.and_then(|row| row.title),
                sort_order: role.sort_order,
            });
        }

        summaries.push(CollaborativeProjectSummary {
            project_id: project.id,
            name: project.name,
            folder_id: project.folder_id,
            folder_name: folder.name,
            folder_path: folder.path,
            manager_role_id,
            manager_conversation_id,
            manager_agent_type: manager_conversation
                .as_ref()
                .map(|row| agent_type_from_db(&row.agent_type))
                .unwrap_or_else(|| agent_type_from_db(&manager_role.agent_type))?,
            manager_title: manager_conversation.and_then(|row| row.title),
            roles_count,
            roles,
            created_at: project.created_at,
            updated_at: project.updated_at,
        });
    }

    Ok(summaries)
}
