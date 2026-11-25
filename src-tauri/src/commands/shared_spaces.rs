use crate::app_error::AppCommandError;
use crate::commands::{conversations, folders};
use crate::db::service::{conversation_service, shared_space_service};
use crate::db::AppDatabase;
use crate::models::{
    AgentType, AppendSharedSpaceMessageParams, ContentBlock, CreateSharedSpaceConversationParams,
    CreateSharedSpaceConversationResult, CreateSharedSpaceParams, CreateSharedSpaceResult,
    ListSharedSpaceMessagesParams, MessageTurn, RemoveSharedSpaceResult, SharedSpaceMessageSummary,
    SharedSpaceSummary, SwitchSharedSpaceAgentParams, SwitchSharedSpaceAgentResult, TurnRole,
};
use crate::parsers::claude::ClaudeParser;
use crate::parsers::cline::ClineParser;
use crate::parsers::codex::CodexParser;
use crate::parsers::gemini::GeminiParser;
use crate::parsers::hermes::HermesParser;
use crate::parsers::openclaw::OpenClawParser;
use crate::parsers::opencode::OpenCodeParser;
use crate::parsers::{path_eq_for_matching, AgentParser, ParseError};
use crate::web::event_bridge::EventEmitter;

fn clean_required(value: &str, field: &str) -> Result<String, AppCommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppCommandError::invalid_input(format!(
            "{field} is required"
        )));
    }
    Ok(trimmed.to_string())
}

fn agent_type_from_db(raw: &str) -> Result<AgentType, AppCommandError> {
    serde_json::from_value(serde_json::Value::String(raw.to_string())).map_err(|e| {
        AppCommandError::invalid_input("Invalid shared space agent type").with_detail(e.to_string())
    })
}

fn default_shared_space_conversation_title(space_name: &str, index: usize) -> String {
    if index <= 1 {
        format!("共享空间 · {space_name}")
    } else {
        format!("共享空间 · {space_name} · 会话{index}")
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated: String = value.chars().take(max_chars).collect();
    truncated.push_str("…");
    truncated
}

fn turn_role_label(role: &TurnRole) -> &'static str {
    match role {
        TurnRole::User => "user",
        TurnRole::Assistant => "assistant",
        TurnRole::System => "system",
    }
}

fn agent_type_key(agent_type: AgentType) -> String {
    serde_json::to_value(agent_type)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{agent_type:?}"))
}

fn block_text(block: &ContentBlock) -> Option<String> {
    match block {
        ContentBlock::Text { text } => Some(text.clone()),
        ContentBlock::Thinking { text } => Some(format!("[thinking]\n{text}")),
        ContentBlock::ToolUse {
            tool_name,
            input_preview,
            ..
        } => Some(format!(
            "[tool_use: {}]{}",
            tool_name,
            input_preview
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("\n{value}"))
                .unwrap_or_default()
        )),
        ContentBlock::ToolResult {
            output_preview,
            is_error,
            ..
        } => output_preview.as_ref().map(|value| {
            if *is_error {
                format!("[tool_result error]\n{value}")
            } else {
                format!("[tool_result]\n{value}")
            }
        }),
        ContentBlock::Image { .. } => Some("[image]".to_string()),
        ContentBlock::ImageGeneration { revised_prompt, .. } => Some(
            revised_prompt
                .as_ref()
                .map(|value| format!("[image_generation]\n{value}"))
                .unwrap_or_else(|| "[image_generation]".to_string()),
        ),
    }
}

fn turn_body_text(turn: &MessageTurn) -> Option<String> {
    let text = turn
        .blocks
        .iter()
        .filter_map(block_text)
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.trim().is_empty() {
        None
    } else {
        Some(truncate_chars(&text, 1200))
    }
}

fn parser_for_agent(agent_type: AgentType) -> Box<dyn AgentParser> {
    match agent_type {
        AgentType::ClaudeCode => Box::new(ClaudeParser::new()),
        AgentType::Codex => Box::new(CodexParser::new()),
        AgentType::OpenCode => Box::new(OpenCodeParser::new()),
        AgentType::Gemini => Box::new(GeminiParser::new()),
        AgentType::OpenClaw => Box::new(OpenClawParser::new()),
        AgentType::Cline => Box::new(ClineParser::new()),
        AgentType::Hermes => Box::new(HermesParser::new()),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SharedSpaceTimelineItem {
    pub agent_type: AgentType,
    pub native_session_id: String,
    pub turn: MessageTurn,
}

async fn read_turns_for_agent_session(
    agent_type: AgentType,
    external_id: String,
    folder_path: String,
    db_created_at: chrono::DateTime<chrono::Utc>,
) -> Vec<MessageTurn> {
    tokio::task::spawn_blocking(move || {
        let parser = parser_for_agent(agent_type);
        match parser.get_conversation(&external_id) {
            Ok(detail) => detail.turns,
            Err(ParseError::ConversationNotFound(_)) => {
                // Some ACP-backed agents start with a runtime session id that is
                // not the parser's stable file id. Mirror the normal
                // conversation loader's narrow fallback so a shared session can
                // still hand off the previous agent's recent turns.
                if !matches!(
                    agent_type,
                    AgentType::OpenClaw | AgentType::Cline | AgentType::Gemini
                ) {
                    return Vec::new();
                }
                let Ok(all) = parser.list_conversations() else {
                    return Vec::new();
                };
                let matched = all
                    .into_iter()
                    .filter(|candidate| {
                        candidate
                            .folder_path
                            .as_ref()
                            .is_some_and(|path| path_eq_for_matching(path, &folder_path))
                    })
                    .min_by_key(|candidate| {
                        (candidate.started_at - db_created_at)
                            .num_seconds()
                            .unsigned_abs()
                    })
                    .filter(|candidate| {
                        (candidate.started_at - db_created_at)
                            .num_seconds()
                            .unsigned_abs()
                            < 300
                    });
                if let Some(candidate) = matched {
                    if let Ok(detail) = parser.get_conversation(&candidate.id) {
                        return detail.turns;
                    }
                }
                Vec::new()
            }
            Err(_) => Vec::new(),
        }
    })
    .await
    .unwrap_or_default()
}

pub(crate) async fn collect_shared_conversation_timeline(
    conn: &sea_orm::DatabaseConnection,
    shared_conversation: &crate::db::entities::shared_space_conversation::Model,
    folder_path: &str,
) -> Vec<SharedSpaceTimelineItem> {
    let created_at =
        shared_space_service::conversation_active(conn, shared_conversation.conversation_id)
            .await
            .ok()
            .flatten()
            .map(|conversation| conversation.created_at)
            .unwrap_or(shared_conversation.created_at);
    let sessions =
        shared_space_service::list_conversation_agent_sessions(conn, shared_conversation.id)
            .await
            .unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();

    for session in sessions {
        let Some(native_session_id) = session
            .native_session_id
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        let Ok(agent_type) = agent_type_from_db(&session.agent_type) else {
            continue;
        };
        if !seen.insert((agent_type, native_session_id.clone())) {
            continue;
        }

        let mut turns = read_turns_for_agent_session(
            agent_type,
            native_session_id.clone(),
            folder_path.to_string(),
            created_at,
        )
        .await;
        crate::parsers::strip_hidden_shared_space_context_turns(&mut turns);

        for mut turn in turns {
            turn.id = format!(
                "shared-space:{}:{}:{}",
                agent_type_key(agent_type),
                native_session_id,
                turn.id
            );
            out.push(SharedSpaceTimelineItem {
                agent_type,
                native_session_id: native_session_id.clone(),
                turn,
            });
        }
    }

    out.sort_by(|a, b| {
        a.turn
            .timestamp
            .cmp(&b.turn.timestamp)
            .then_with(|| agent_type_key(a.agent_type).cmp(&agent_type_key(b.agent_type)))
            .then_with(|| a.native_session_id.cmp(&b.native_session_id))
            .then_with(|| a.turn.id.cmp(&b.turn.id))
    });
    out
}

async fn default_conversation_title(
    db: &AppDatabase,
    space_id: i32,
    space_name: &str,
    title: Option<String>,
) -> Result<String, AppCommandError> {
    if let Some(value) = title.map(|value| value.trim().to_string()) {
        if !value.is_empty() {
            return Ok(value);
        }
    }
    let existing = shared_space_service::list_shared_conversations(&db.conn, space_id)
        .await
        .map_err(AppCommandError::from)?;
    Ok(default_shared_space_conversation_title(
        space_name,
        existing.len() + 1,
    ))
}

async fn active_or_first_shared_conversation(
    db: &AppDatabase,
    space: &crate::db::entities::shared_space::Model,
) -> Result<crate::db::entities::shared_space_conversation::Model, AppCommandError> {
    shared_space_service::get_active_shared_conversation(&db.conn, space)
        .await
        .map_err(AppCommandError::from)?
        .ok_or_else(|| {
            AppCommandError::not_found(format!("Shared space {} has no conversation", space.id))
        })
}

async fn get_requested_shared_conversation(
    db: &AppDatabase,
    space: &crate::db::entities::shared_space::Model,
    shared_conversation_id: Option<i32>,
) -> Result<crate::db::entities::shared_space_conversation::Model, AppCommandError> {
    match shared_conversation_id {
        Some(id) => {
            let row = shared_space_service::get_shared_conversation(&db.conn, id)
                .await
                .map_err(AppCommandError::from)?
                .ok_or_else(|| {
                    AppCommandError::not_found(format!("Shared conversation {id} not found"))
                })?;
            if row.space_id != space.id {
                return Err(AppCommandError::invalid_input(format!(
                    "Shared conversation {id} does not belong to shared space {}",
                    space.id
                )));
            }
            Ok(row)
        }
        None => active_or_first_shared_conversation(db, space).await,
    }
}

async fn remember_current_native_session(
    db: &AppDatabase,
    conversation_id: i32,
) -> Result<(), AppCommandError> {
    let Some(conversation) = shared_space_service::conversation_active(&db.conn, conversation_id)
        .await
        .map_err(AppCommandError::from)?
    else {
        return Ok(());
    };
    if let Some(external_id) = conversation.external_id {
        shared_space_service::upsert_agent_session_for_conversation(
            &db.conn,
            conversation_id,
            &conversation.agent_type,
            Some(external_id),
        )
        .await
        .map_err(AppCommandError::from)?;
    }
    Ok(())
}

pub(crate) async fn build_shared_space_context_snapshot(
    db: &AppDatabase,
    space_id: i32,
    target_agent_type: AgentType,
    current_shared_conversation_id: Option<i32>,
) -> Result<(String, i32), AppCommandError> {
    let ctx = shared_space_service::get_space_with_folder(&db.conn, space_id)
        .await
        .map_err(AppCommandError::from)?
        .ok_or_else(|| AppCommandError::not_found(format!("Shared space {space_id} not found")))?;
    let shared_conversations = shared_space_service::list_shared_conversations(&db.conn, space_id)
        .await
        .map_err(AppCommandError::from)?;
    let shared_messages = shared_space_service::list_messages(&db.conn, space_id, 20)
        .await
        .unwrap_or_default();
    let current_shared_conversation_id =
        current_shared_conversation_id.or(ctx.space.active_shared_conversation_id);

    let mut prompt = String::new();
    prompt.push_str("你正在接手 Billow 共享空间中的同一个项目。请基于下面的共享上下文继续工作，不要把它当成一个全新项目。\n\n");
    prompt.push_str("## 共享空间\n");
    prompt.push_str(&format!("- 名称：{}\n", ctx.space.name));
    prompt.push_str(&format!("- 工作目录：{}\n", ctx.folder.path));
    prompt.push_str(&format!("- 当前 Agent：{}\n", target_agent_type));

    if !shared_conversations.is_empty() {
        prompt.push_str("\n## 共享会话\n");
        for shared_conversation in &shared_conversations {
            let active_agent = agent_type_from_db(&shared_conversation.active_agent_type)?;
            let title = if let Some(title) = shared_conversation.title.clone() {
                title
            } else {
                shared_space_service::conversation_active(
                    &db.conn,
                    shared_conversation.conversation_id,
                )
                .await
                .ok()
                .flatten()
                .and_then(|row| row.title)
                .unwrap_or_else(|| format!("conversation #{}", shared_conversation.conversation_id))
            };
            let active_marker = if Some(shared_conversation.id) == current_shared_conversation_id {
                "，当前共享会话"
            } else {
                ""
            };
            prompt.push_str(&format!(
                "- {}（shared_conversation #{}, conversation #{}, 当前 Agent：{}{active_marker}）\n",
                title,
                shared_conversation.id,
                shared_conversation.conversation_id,
                active_agent
            ));

            let sessions = shared_space_service::list_conversation_agent_sessions(
                &db.conn,
                shared_conversation.id,
            )
            .await
            .unwrap_or_default();
            if !sessions.is_empty() {
                prompt.push_str("  - 已使用 Agent：");
                let labels = sessions
                    .iter()
                    .filter_map(|session| {
                        agent_type_from_db(&session.agent_type).ok().map(|agent| {
                            match session.native_session_id.as_deref() {
                                Some(native) if !native.is_empty() => {
                                    format!("{}({})", agent, truncate_chars(native, 18))
                                }
                                _ => format!("{}(未启动原生会话)", agent),
                            }
                        })
                    })
                    .collect::<Vec<_>>()
                    .join("、");
                prompt.push_str(&labels);
                prompt.push('\n');
            }
        }
    }

    if !shared_messages.is_empty() {
        prompt.push_str("\n## Billow 共享消息\n");
        for message in &shared_messages {
            prompt.push_str(&format!(
                "- {} @ {}{}：{}\n",
                message.role,
                message.created_at.to_rfc3339(),
                message
                    .source_agent_type
                    .map(|agent| format!(" / {agent}"))
                    .unwrap_or_default(),
                truncate_chars(&message.content.replace('\n', " "), 800)
            ));
        }
    }

    const TIMELINE_TURN_LIMIT: usize = 24;

    let mut recent_json_items = Vec::new();
    for shared_conversation in &shared_conversations {
        let timeline =
            collect_shared_conversation_timeline(&db.conn, shared_conversation, &ctx.folder.path)
                .await;
        if timeline.is_empty() {
            continue;
        }
        let title = shared_conversation
            .title
            .clone()
            .unwrap_or_else(|| format!("conversation #{}", shared_conversation.conversation_id));
        let active_marker = if Some(shared_conversation.id) == current_shared_conversation_id {
            "（当前共享会话）"
        } else {
            ""
        };
        let mut recent_timeline = timeline
            .iter()
            .rev()
            .take(TIMELINE_TURN_LIMIT)
            .collect::<Vec<_>>();
        recent_timeline.reverse();

        prompt.push_str(&format!(
            "\n## 最近共享时间线：{}{}\n",
            title, active_marker
        ));
        prompt.push_str("以下记录按 Billow 页面同一条共享时间线排序，包含不同 Agent 在该共享会话中的连续上下文。\n");

        let mut json_timeline = Vec::new();
        for item in recent_timeline {
            let Some(body) = turn_body_text(&item.turn) else {
                continue;
            };
            prompt.push_str(&format!(
                "\n---\n[{}][{}][{}][{}]\n{}\n",
                item.turn.timestamp.to_rfc3339(),
                item.agent_type,
                item.native_session_id,
                turn_role_label(&item.turn.role),
                body
            ));
            json_timeline.push(serde_json::json!({
                "agentType": item.agent_type,
                "nativeSessionId": item.native_session_id,
                "turnId": item.turn.id,
                "role": turn_role_label(&item.turn.role),
                "timestamp": item.turn.timestamp.to_rfc3339(),
                "text": body,
            }));
        }
        if json_timeline.is_empty() {
            continue;
        }
        recent_json_items.push(serde_json::json!({
            "sharedConversationId": shared_conversation.id,
            "conversationId": shared_conversation.conversation_id,
            "activeAgentType": agent_type_from_db(&shared_conversation.active_agent_type)?,
            "timelineOrder": "timestamp, agentType, nativeSessionId, turnId",
            "timeline": json_timeline,
        }));
    }

    prompt.push_str("\n## 接手要求\n");
    prompt.push_str("- 围绕当前共享会话继续工作，不要新建一条无关会话。\n");
    prompt.push_str("- 先确认当前代码和运行状态，再继续开发。\n");
    prompt.push_str("- 如果上文与代码实际状态冲突，以当前工作目录中的真实代码和运行结果为准。\n");
    prompt.push_str("- 保持共享空间连续性：重要决策、状态变化和交接信息应写回 Billow 共享空间。\n");

    let summary = truncate_chars(&prompt, 8000);
    let snapshot = shared_space_service::upsert_context_snapshot(
        &db.conn,
        space_id,
        &summary,
        &serde_json::to_string(&recent_json_items).unwrap_or_else(|_| "[]".to_string()),
    )
    .await
    .map_err(AppCommandError::from)?;

    Ok((summary, snapshot.id))
}

pub async fn create_shared_space_core(
    emitter: &EventEmitter,
    db: &AppDatabase,
    params: CreateSharedSpaceParams,
) -> Result<CreateSharedSpaceResult, AppCommandError> {
    let name = clean_required(&params.name, "Shared space name")?;
    let working_dir = clean_required(&params.working_dir, "Working directory")?;

    let meta = std::fs::metadata(&working_dir).map_err(AppCommandError::io)?;
    if !meta.is_dir() {
        return Err(AppCommandError::invalid_input(
            "Working directory must be an existing directory",
        ));
    }

    let folder = folders::open_folder_in_workspace_core(emitter, db, working_dir.clone()).await?;
    let space = shared_space_service::create_space(&db.conn, &name, folder.id)
        .await
        .map_err(AppCommandError::from)?;
    let title = default_shared_space_conversation_title(&name, 1);
    let conversation_id = conversations::create_conversation_core(
        &db.conn,
        folder.id,
        params.agent_type,
        Some(title.clone()),
    )
    .await?;
    conversations::update_conversation_title_core(&db.conn, conversation_id, title.clone()).await?;
    let shared_conversation = shared_space_service::create_shared_conversation(
        &db.conn,
        space.id,
        conversation_id,
        Some(title.clone()),
        params.agent_type,
    )
    .await
    .map_err(AppCommandError::from)?;
    shared_space_service::ensure_conversation_agent_session(
        &db.conn,
        shared_conversation.id,
        params.agent_type,
    )
    .await
    .map_err(AppCommandError::from)?;
    shared_space_service::set_active_shared_conversation(
        &db.conn,
        space.id,
        shared_conversation.id,
    )
    .await
    .map_err(AppCommandError::from)?;
    conversations::emit_conversation_upsert(emitter, &db.conn, conversation_id).await;
    let (_, context_snapshot_id) = build_shared_space_context_snapshot(
        db,
        space.id,
        params.agent_type,
        Some(shared_conversation.id),
    )
    .await?;

    Ok(CreateSharedSpaceResult {
        space_id: space.id,
        folder_id: folder.id,
        folder,
        active_agent_type: params.agent_type,
        active_shared_conversation_id: shared_conversation.id,
        active_conversation_id: conversation_id,
        conversation_title: title,
        context_snapshot_id: Some(context_snapshot_id),
    })
}

pub async fn create_shared_space_conversation_core(
    emitter: &EventEmitter,
    db: &AppDatabase,
    params: CreateSharedSpaceConversationParams,
) -> Result<CreateSharedSpaceConversationResult, AppCommandError> {
    let ctx = shared_space_service::get_space_with_folder(&db.conn, params.space_id)
        .await
        .map_err(AppCommandError::from)?
        .ok_or_else(|| {
            AppCommandError::not_found(format!("Shared space {} not found", params.space_id))
        })?;
    let agent_type = if let Some(agent_type) = params.agent_type {
        agent_type
    } else {
        match active_or_first_shared_conversation(db, &ctx.space).await {
            Ok(row) => agent_type_from_db(&row.active_agent_type).unwrap_or(AgentType::ClaudeCode),
            Err(_) => AgentType::ClaudeCode,
        }
    };
    let title = default_conversation_title(db, ctx.space.id, &ctx.space.name, params.title).await?;
    let conversation_id = conversations::create_conversation_core(
        &db.conn,
        ctx.space.folder_id,
        agent_type,
        Some(title.clone()),
    )
    .await?;
    conversations::update_conversation_title_core(&db.conn, conversation_id, title.clone()).await?;
    let shared_conversation = shared_space_service::create_shared_conversation(
        &db.conn,
        ctx.space.id,
        conversation_id,
        Some(title.clone()),
        agent_type,
    )
    .await
    .map_err(AppCommandError::from)?;
    shared_space_service::ensure_conversation_agent_session(
        &db.conn,
        shared_conversation.id,
        agent_type,
    )
    .await
    .map_err(AppCommandError::from)?;
    shared_space_service::set_active_shared_conversation(
        &db.conn,
        ctx.space.id,
        shared_conversation.id,
    )
    .await
    .map_err(AppCommandError::from)?;
    conversations::emit_conversation_upsert(emitter, &db.conn, conversation_id).await;
    let (_, context_snapshot_id) = build_shared_space_context_snapshot(
        db,
        ctx.space.id,
        agent_type,
        Some(shared_conversation.id),
    )
    .await?;

    Ok(CreateSharedSpaceConversationResult {
        space_id: ctx.space.id,
        shared_conversation_id: shared_conversation.id,
        agent_type,
        conversation_id,
        conversation_title: title,
        context_snapshot_id: Some(context_snapshot_id),
    })
}

pub async fn list_shared_spaces_core(
    db: &AppDatabase,
) -> Result<Vec<SharedSpaceSummary>, AppCommandError> {
    shared_space_service::list_spaces(&db.conn)
        .await
        .map_err(AppCommandError::from)
}

pub async fn switch_shared_space_agent_core(
    emitter: &EventEmitter,
    db: &AppDatabase,
    params: SwitchSharedSpaceAgentParams,
) -> Result<SwitchSharedSpaceAgentResult, AppCommandError> {
    let ctx = shared_space_service::get_space_with_folder(&db.conn, params.space_id)
        .await
        .map_err(AppCommandError::from)?
        .ok_or_else(|| {
            AppCommandError::not_found(format!("Shared space {} not found", params.space_id))
        })?;

    let shared_conversation =
        get_requested_shared_conversation(db, &ctx.space, params.shared_conversation_id).await?;
    let conversation_id = shared_conversation.conversation_id;
    remember_current_native_session(db, conversation_id).await?;

    let agent_session = shared_space_service::ensure_conversation_agent_session(
        &db.conn,
        shared_conversation.id,
        params.agent_type,
    )
    .await
    .map_err(AppCommandError::from)?;
    shared_space_service::set_shared_conversation_agent(
        &db.conn,
        shared_conversation.id,
        params.agent_type,
    )
    .await
    .map_err(AppCommandError::from)?;
    shared_space_service::set_active_shared_conversation(
        &db.conn,
        ctx.space.id,
        shared_conversation.id,
    )
    .await
    .map_err(AppCommandError::from)?;
    conversation_service::update_agent_type_and_external_id(
        &db.conn,
        conversation_id,
        params.agent_type,
        agent_session.native_session_id.clone(),
    )
    .await
    .map_err(AppCommandError::from)?;

    conversations::emit_conversation_upsert(emitter, &db.conn, conversation_id).await;
    let (_, context_snapshot_id) = build_shared_space_context_snapshot(
        db,
        ctx.space.id,
        params.agent_type,
        Some(shared_conversation.id),
    )
    .await?;
    let conversation_title = if let Some(title) = shared_conversation.title {
        title
    } else {
        shared_space_service::conversation_active(&db.conn, conversation_id)
            .await
            .ok()
            .flatten()
            .and_then(|row| row.title)
            .unwrap_or_else(|| format!("conversation #{conversation_id}"))
    };

    Ok(SwitchSharedSpaceAgentResult {
        space_id: ctx.space.id,
        shared_conversation_id: shared_conversation.id,
        agent_type: params.agent_type,
        conversation_id,
        conversation_title,
        context_snapshot_id: Some(context_snapshot_id),
    })
}

pub async fn remove_shared_space_core(
    db: &AppDatabase,
    space_id: i32,
) -> Result<RemoveSharedSpaceResult, AppCommandError> {
    shared_space_service::soft_delete_space(&db.conn, space_id)
        .await
        .map_err(AppCommandError::from)?
        .ok_or_else(|| AppCommandError::not_found(format!("Shared space {space_id} not found")))?;
    Ok(RemoveSharedSpaceResult { space_id })
}

pub async fn append_shared_space_message_core(
    db: &AppDatabase,
    params: AppendSharedSpaceMessageParams,
) -> Result<SharedSpaceMessageSummary, AppCommandError> {
    let role = clean_required(&params.role, "Message role")?;
    let content = clean_required(&params.content, "Message content")?;
    let row = shared_space_service::create_message(
        &db.conn,
        shared_space_service::NewSharedSpaceMessage {
            space_id: params.space_id,
            binding_id: params.binding_id,
            role: &role,
            content: &content,
            source_agent_type: params.source_agent_type,
            native_message_id: params.native_message_id.as_deref(),
        },
    )
    .await
    .map_err(AppCommandError::from)?;
    Ok(SharedSpaceMessageSummary {
        message_id: row.id,
        space_id: row.space_id,
        binding_id: row.binding_id,
        role: row.role,
        content: row.content,
        source_agent_type: row
            .source_agent_type
            .as_deref()
            .map(|raw| serde_json::from_value(serde_json::Value::String(raw.to_string())))
            .transpose()
            .map_err(|e| {
                AppCommandError::invalid_input("Invalid shared space agent type")
                    .with_detail(e.to_string())
            })?,
        native_message_id: row.native_message_id,
        created_at: row.created_at,
    })
}

pub async fn list_shared_space_messages_core(
    db: &AppDatabase,
    params: ListSharedSpaceMessagesParams,
) -> Result<Vec<SharedSpaceMessageSummary>, AppCommandError> {
    shared_space_service::list_messages(&db.conn, params.space_id, params.limit.unwrap_or(100))
        .await
        .map_err(AppCommandError::from)
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn create_shared_space(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDatabase>,
    name: String,
    working_dir: String,
    agent_type: AgentType,
) -> Result<CreateSharedSpaceResult, AppCommandError> {
    let emitter = EventEmitter::Tauri(app);
    create_shared_space_core(
        &emitter,
        &db,
        CreateSharedSpaceParams {
            name,
            working_dir,
            agent_type,
        },
    )
    .await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn create_shared_space_conversation(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDatabase>,
    space_id: i32,
    title: Option<String>,
    agent_type: Option<AgentType>,
) -> Result<CreateSharedSpaceConversationResult, AppCommandError> {
    let emitter = EventEmitter::Tauri(app);
    create_shared_space_conversation_core(
        &emitter,
        &db,
        CreateSharedSpaceConversationParams {
            space_id,
            title,
            agent_type,
        },
    )
    .await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn list_shared_spaces(
    db: tauri::State<'_, AppDatabase>,
) -> Result<Vec<SharedSpaceSummary>, AppCommandError> {
    list_shared_spaces_core(&db).await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn switch_shared_space_agent(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDatabase>,
    space_id: i32,
    agent_type: AgentType,
    shared_conversation_id: Option<i32>,
) -> Result<SwitchSharedSpaceAgentResult, AppCommandError> {
    let emitter = EventEmitter::Tauri(app);
    switch_shared_space_agent_core(
        &emitter,
        &db,
        SwitchSharedSpaceAgentParams {
            space_id,
            agent_type,
            shared_conversation_id,
        },
    )
    .await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn remove_shared_space(
    db: tauri::State<'_, AppDatabase>,
    space_id: i32,
) -> Result<RemoveSharedSpaceResult, AppCommandError> {
    remove_shared_space_core(&db, space_id).await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn append_shared_space_message(
    db: tauri::State<'_, AppDatabase>,
    space_id: i32,
    binding_id: Option<i32>,
    role: String,
    content: String,
    source_agent_type: Option<AgentType>,
    native_message_id: Option<String>,
) -> Result<SharedSpaceMessageSummary, AppCommandError> {
    append_shared_space_message_core(
        &db,
        AppendSharedSpaceMessageParams {
            space_id,
            binding_id,
            role,
            content,
            source_agent_type,
            native_message_id,
        },
    )
    .await
}

#[cfg(feature = "tauri-runtime")]
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn list_shared_space_messages(
    db: tauri::State<'_, AppDatabase>,
    space_id: i32,
    limit: Option<u64>,
) -> Result<Vec<SharedSpaceMessageSummary>, AppCommandError> {
    list_shared_space_messages_core(&db, ListSharedSpaceMessagesParams { space_id, limit }).await
}
