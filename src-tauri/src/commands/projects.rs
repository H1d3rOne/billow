use std::collections::HashSet;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;

use crate::acp::delegation::listener::{
    ProjectDelegationAccess, ProjectDelegationContext, ProjectDelegationRole,
};
use crate::app_error::AppCommandError;
use crate::commands::{conversations, folders};
use crate::db::service::project_service;
use crate::db::AppDatabase;
use crate::models::{
    AddCollaborativeProjectRoleParams, AddCollaborativeProjectRoleResult,
    CollaborativeProjectRoleSummary, CollaborativeProjectSummary, CreateCollaborativeProjectParams,
    CreateCollaborativeProjectResult, CreateCollaborativeProjectRoleParams,
    EnsureProjectRoleConversationParams, ProjectRoleConversationResult,
    RemoveCollaborativeProjectResult,
};
use crate::web::event_bridge::EventEmitter;

pub struct DbProjectDelegationAccess {
    conn: DatabaseConnection,
}

impl DbProjectDelegationAccess {
    pub fn new(conn: DatabaseConnection) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl ProjectDelegationAccess for DbProjectDelegationAccess {
    async fn resolve_role(
        &self,
        manager_conversation_id: i32,
        role_key: &str,
    ) -> Option<ProjectDelegationRole> {
        match project_service::find_project_role_for_manager_conversation(
            &self.conn,
            manager_conversation_id,
            role_key,
        )
        .await
        {
            Ok(Some(binding)) => Some(ProjectDelegationRole {
                role_id: binding.role_id,
                name: binding.name,
                role_key: binding.role_key,
                agent_type: binding.agent_type,
                instructions: binding.instructions,
                is_manager: binding.is_manager,
                working_dir: binding.working_dir,
                conversation_id: binding.conversation_id,
                conversation_external_id: binding.conversation_external_id,
            }),
            Ok(None) => None,
            Err(err) => {
                eprintln!(
                    "[projects] failed to resolve collaborative project role '{role_key}' \
                     for manager conversation {manager_conversation_id}: {err}"
                );
                None
            }
        }
    }

    async fn bind_role_conversation(&self, role_id: i32, conversation_id: i32) {
        if let Err(err) =
            project_service::set_role_conversation(&self.conn, role_id, conversation_id).await
        {
            eprintln!(
                "[projects] failed to bind collaborative project role {role_id} \
                 to conversation {conversation_id}: {err}"
            );
        }
    }

    async fn list_roles(&self, manager_conversation_id: i32) -> Option<ProjectDelegationContext> {
        match project_service::list_project_roles_for_manager_conversation(
            &self.conn,
            manager_conversation_id,
        )
        .await
        {
            Ok(Some(ctx)) => Some(ProjectDelegationContext {
                project_id: ctx.project_id,
                project_name: ctx.project_name,
                manager_conversation_id: ctx.manager_conversation_id,
                working_dir: ctx.working_dir,
                roles: ctx
                    .roles
                    .into_iter()
                    .map(|binding| ProjectDelegationRole {
                        role_id: binding.role_id,
                        name: binding.name,
                        role_key: binding.role_key,
                        agent_type: binding.agent_type,
                        instructions: binding.instructions,
                        is_manager: binding.is_manager,
                        working_dir: binding.working_dir,
                        conversation_id: binding.conversation_id,
                        conversation_external_id: binding.conversation_external_id,
                    })
                    .collect(),
            }),
            Ok(None) => None,
            Err(err) => {
                eprintln!(
                    "[projects] failed to list collaborative project roles for manager \
                     conversation {manager_conversation_id}: {err}"
                );
                None
            }
        }
    }
}

fn clean_required(value: &str, field: &str) -> Result<String, AppCommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppCommandError::invalid_input(format!(
            "{field} is required"
        )));
    }
    Ok(trimmed.to_string())
}

fn validate_role_key(role_key: &str) -> Result<(), AppCommandError> {
    if role_key.len() > 64 {
        return Err(AppCommandError::invalid_input(
            "Role key must be 64 characters or fewer",
        ));
    }
    if !role_key
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(AppCommandError::invalid_input(
            "Role key may only contain lowercase letters, numbers, underscores, and hyphens",
        ));
    }
    Ok(())
}

fn normalize_roles(
    roles: Vec<CreateCollaborativeProjectRoleParams>,
) -> Result<Vec<CreateCollaborativeProjectRoleParams>, AppCommandError> {
    if roles.is_empty() {
        return Err(AppCommandError::invalid_input(
            "At least one project role is required",
        ));
    }

    let manager_count = roles.iter().filter(|role| role.is_manager).count();
    if manager_count != 1 {
        return Err(AppCommandError::invalid_input(
            "Exactly one project manager role is required",
        ));
    }

    let mut seen = HashSet::new();
    roles
        .into_iter()
        .map(|role| {
            let name = clean_required(&role.name, "Role name")?;
            let role_key = clean_required(&role.role_key, "Role key")?;
            validate_role_key(&role_key)?;
            if !seen.insert(role_key.clone()) {
                return Err(AppCommandError::invalid_input(format!(
                    "Duplicate role key: {role_key}"
                )));
            }
            let instructions = role.instructions.and_then(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            });
            Ok(CreateCollaborativeProjectRoleParams {
                name,
                role_key,
                agent_type: role.agent_type,
                instructions,
                is_manager: role.is_manager,
            })
        })
        .collect()
}

pub async fn create_collaborative_project_core(
    emitter: &EventEmitter,
    db: &AppDatabase,
    params: CreateCollaborativeProjectParams,
) -> Result<CreateCollaborativeProjectResult, AppCommandError> {
    let name = clean_required(&params.name, "Project name")?;
    let working_dir = clean_required(&params.working_dir, "Working directory")?;
    let roles = normalize_roles(params.roles)?;

    let meta = std::fs::metadata(&working_dir).map_err(AppCommandError::io)?;
    if !meta.is_dir() {
        return Err(AppCommandError::invalid_input(
            "Working directory must be an existing directory",
        ));
    }

    let folder = folders::open_folder_in_workspace_core(emitter, db, working_dir.clone()).await?;

    let project = project_service::create_project(&db.conn, &name, folder.id)
        .await
        .map_err(AppCommandError::from)?;

    let mut manager_role_id = None;
    let mut manager_conversation_id = None;
    let mut manager_title = None;
    let mut created_conversation_ids = Vec::with_capacity(roles.len());
    for (index, role) in roles.iter().enumerate() {
        let inserted = project_service::create_project_role(
            &db.conn,
            project.id,
            project_service::NewProjectRole {
                name: &role.name,
                role_key: &role.role_key,
                agent_type: role.agent_type,
                instructions: role.instructions.as_deref(),
                is_manager: role.is_manager,
                sort_order: index as i32,
            },
        )
        .await
        .map_err(AppCommandError::from)?;

        let role_title = if role.is_manager {
            format!("项目经理 - {name}")
        } else {
            format!("{} - {name}", role.name)
        };
        let conversation_id = conversations::create_conversation_core(
            &db.conn,
            folder.id,
            role.agent_type,
            Some(role_title.clone()),
        )
        .await?;

        // Lock the role-oriented title so later parser auto-title refreshes do
        // not replace the project sidebar/tab label with an agent-generated
        // session title.
        conversations::update_conversation_title_core(
            &db.conn,
            conversation_id,
            role_title.clone(),
        )
        .await?;

        project_service::set_role_conversation(&db.conn, inserted.id, conversation_id)
            .await
            .map_err(AppCommandError::from)?;
        created_conversation_ids.push(conversation_id);

        if role.is_manager {
            manager_role_id = Some(inserted.id);
            manager_conversation_id = Some(conversation_id);
            manager_title = Some(role_title);
        }
    }

    let manager_role_id = manager_role_id.ok_or_else(|| {
        AppCommandError::invalid_input("Exactly one project manager role is required")
    })?;
    let manager_conversation_id = manager_conversation_id.ok_or_else(|| {
        AppCommandError::invalid_input("Exactly one project manager role is required")
    })?;
    let manager_title = manager_title.ok_or_else(|| {
        AppCommandError::invalid_input("Exactly one project manager role is required")
    })?;
    project_service::set_project_manager(
        &db.conn,
        project.id,
        manager_role_id,
        manager_conversation_id,
    )
    .await
    .map_err(AppCommandError::from)?;

    for conversation_id in created_conversation_ids {
        conversations::emit_conversation_upsert(emitter, &db.conn, conversation_id).await;
    }

    Ok(CreateCollaborativeProjectResult {
        project_id: project.id,
        folder_id: folder.id,
        folder,
        manager_role_id,
        manager_conversation_id,
        manager_title,
    })
}

pub async fn list_collaborative_projects_core(
    db: &AppDatabase,
) -> Result<Vec<CollaborativeProjectSummary>, AppCommandError> {
    project_service::list_projects(&db.conn)
        .await
        .map_err(AppCommandError::from)
}

fn role_conversation_title(project_name: &str, role_name: &str) -> String {
    format!("{role_name} - {project_name}")
}

async fn create_role_conversation(
    emitter: &EventEmitter,
    db: &AppDatabase,
    project: &crate::db::entities::project::Model,
    role: &crate::db::entities::project_role::Model,
) -> Result<ProjectRoleConversationResult, AppCommandError> {
    let agent_type = serde_json::from_value(serde_json::Value::String(role.agent_type.clone()))
        .map_err(|e| {
            AppCommandError::invalid_input("Invalid project role agent type")
                .with_detail(e.to_string())
        })?;
    let conversation_title = role_conversation_title(&project.name, &role.name);
    let conversation_id = conversations::create_conversation_core(
        &db.conn,
        project.folder_id,
        agent_type,
        Some(conversation_title.clone()),
    )
    .await?;
    conversations::update_conversation_title_core(
        &db.conn,
        conversation_id,
        conversation_title.clone(),
    )
    .await?;
    project_service::set_role_conversation(&db.conn, role.id, conversation_id)
        .await
        .map_err(AppCommandError::from)?;
    if role.is_manager {
        project_service::set_project_manager(&db.conn, project.id, role.id, conversation_id)
            .await
            .map_err(AppCommandError::from)?;
    }
    conversations::emit_conversation_upsert(emitter, &db.conn, conversation_id).await;

    Ok(ProjectRoleConversationResult {
        project_id: project.id,
        role_id: role.id,
        conversation_id,
        conversation_title,
    })
}

pub async fn ensure_project_role_conversation_core(
    emitter: &EventEmitter,
    db: &AppDatabase,
    params: EnsureProjectRoleConversationParams,
) -> Result<ProjectRoleConversationResult, AppCommandError> {
    let role = project_service::get_project_role(&db.conn, params.role_id)
        .await
        .map_err(AppCommandError::from)?
        .ok_or_else(|| {
            AppCommandError::not_found(format!("Project role {} not found", params.role_id))
        })?;
    let project = project_service::get_project(&db.conn, role.project_id)
        .await
        .map_err(AppCommandError::from)?
        .ok_or_else(|| {
            AppCommandError::not_found(format!(
                "Collaborative project {} not found",
                role.project_id
            ))
        })?;

    if !params.recreate {
        if let Some(conversation_id) = role.conversation_id {
            if let Some(conversation) =
                project_service::get_active_conversation(&db.conn, conversation_id)
                    .await
                    .map_err(AppCommandError::from)?
            {
                return Ok(ProjectRoleConversationResult {
                    project_id: project.id,
                    role_id: role.id,
                    conversation_id,
                    conversation_title: conversation
                        .title
                        .unwrap_or_else(|| role_conversation_title(&project.name, &role.name)),
                });
            }
        }
    }

    create_role_conversation(emitter, db, &project, &role).await
}

pub async fn add_collaborative_project_role_core(
    emitter: &EventEmitter,
    db: &AppDatabase,
    params: AddCollaborativeProjectRoleParams,
) -> Result<AddCollaborativeProjectRoleResult, AppCommandError> {
    let name = clean_required(&params.name, "Role name")?;
    let role_key = clean_required(&params.role_key, "Role key")?;
    validate_role_key(&role_key)?;
    let instructions = params.instructions.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });

    let project = project_service::get_project(&db.conn, params.project_id)
        .await
        .map_err(AppCommandError::from)?
        .ok_or_else(|| {
            AppCommandError::not_found(format!(
                "Collaborative project {} not found",
                params.project_id
            ))
        })?;

    if project_service::project_role_key_exists(&db.conn, project.id, &role_key)
        .await
        .map_err(AppCommandError::from)?
    {
        return Err(AppCommandError::already_exists(format!(
            "Project role key already exists: {role_key}"
        )));
    }

    let sort_order = project_service::count_project_roles(&db.conn, project.id)
        .await
        .map_err(AppCommandError::from)?
        .min(i32::MAX as u64) as i32;

    let inserted = project_service::create_project_role(
        &db.conn,
        project.id,
        project_service::NewProjectRole {
            name: &name,
            role_key: &role_key,
            agent_type: params.agent_type,
            instructions: instructions.as_deref(),
            is_manager: false,
            sort_order,
        },
    )
    .await
    .map_err(AppCommandError::from)?;

    let conversation_title = format!("{name} - {}", project.name);
    let conversation_id = conversations::create_conversation_core(
        &db.conn,
        project.folder_id,
        params.agent_type,
        Some(conversation_title.clone()),
    )
    .await?;
    conversations::update_conversation_title_core(
        &db.conn,
        conversation_id,
        conversation_title.clone(),
    )
    .await?;

    project_service::set_role_conversation(&db.conn, inserted.id, conversation_id)
        .await
        .map_err(AppCommandError::from)?;
    conversations::emit_conversation_upsert(emitter, &db.conn, conversation_id).await;

    Ok(AddCollaborativeProjectRoleResult {
        role: CollaborativeProjectRoleSummary {
            role_id: inserted.id,
            name,
            role_key,
            agent_type: params.agent_type,
            instructions,
            is_manager: false,
            conversation_id: Some(conversation_id),
            conversation_title: Some(conversation_title.clone()),
            sort_order,
        },
        conversation_id,
        conversation_title,
    })
}

pub async fn remove_collaborative_project_core(
    db: &AppDatabase,
    project_id: i32,
) -> Result<RemoveCollaborativeProjectResult, AppCommandError> {
    project_service::soft_delete_project(&db.conn, project_id)
        .await
        .map_err(AppCommandError::from)?
        .ok_or_else(|| {
            AppCommandError::not_found(format!("Collaborative project {project_id} not found"))
        })?;

    Ok(RemoveCollaborativeProjectResult { project_id })
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn create_collaborative_project(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDatabase>,
    name: String,
    working_dir: String,
    roles: Vec<CreateCollaborativeProjectRoleParams>,
) -> Result<CreateCollaborativeProjectResult, AppCommandError> {
    let emitter = EventEmitter::Tauri(app);
    let params = CreateCollaborativeProjectParams {
        name,
        working_dir,
        roles,
    };
    create_collaborative_project_core(&emitter, &db, params).await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn list_collaborative_projects(
    db: tauri::State<'_, AppDatabase>,
) -> Result<Vec<CollaborativeProjectSummary>, AppCommandError> {
    list_collaborative_projects_core(&db).await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn add_collaborative_project_role(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDatabase>,
    project_id: i32,
    name: String,
    role_key: String,
    agent_type: crate::models::AgentType,
    instructions: Option<String>,
) -> Result<AddCollaborativeProjectRoleResult, AppCommandError> {
    let emitter = EventEmitter::Tauri(app);
    let params = AddCollaborativeProjectRoleParams {
        project_id,
        name,
        role_key,
        agent_type,
        instructions,
    };
    add_collaborative_project_role_core(&emitter, &db, params).await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn ensure_project_role_conversation(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDatabase>,
    role_id: i32,
    recreate: bool,
) -> Result<ProjectRoleConversationResult, AppCommandError> {
    let emitter = EventEmitter::Tauri(app);
    ensure_project_role_conversation_core(
        &emitter,
        &db,
        EnsureProjectRoleConversationParams { role_id, recreate },
    )
    .await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn remove_collaborative_project(
    db: tauri::State<'_, AppDatabase>,
    project_id: i32,
) -> Result<RemoveCollaborativeProjectResult, AppCommandError> {
    remove_collaborative_project_core(&db, project_id).await
}
