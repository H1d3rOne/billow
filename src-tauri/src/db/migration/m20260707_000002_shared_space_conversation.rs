use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DbBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

const IDX_SHARED_SPACE_ACTIVE_CONVERSATION: &str = "idx_shared_space_active_shared_conversation_id";
const IDX_SHARED_SPACE_CONVERSATION_SPACE: &str = "idx_shared_space_conversation_space_id";
const IDX_SHARED_SPACE_CONVERSATION_CONVERSATION: &str =
    "idx_shared_space_conversation_conversation_id";
const IDX_SHARED_SPACE_CONVERSATION_AGENT_CONV: &str =
    "idx_shared_space_conversation_agent_conversation_id";
const IDX_SHARED_SPACE_CONVERSATION_AGENT_UNIQUE: &str =
    "idx_shared_space_conversation_agent_unique";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SharedSpace::Table)
                    .add_column(
                        ColumnDef::new(SharedSpace::ActiveSharedConversationId)
                            .integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SharedSpaceConversation::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SharedSpaceConversation::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversation::SpaceId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversation::ConversationId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversation::Title)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversation::ActiveAgentType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversation::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversation::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversation::DeletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_shared_space_conversation_space")
                            .from(
                                SharedSpaceConversation::Table,
                                SharedSpaceConversation::SpaceId,
                            )
                            .to(SharedSpace::Table, SharedSpace::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_shared_space_conversation_conversation")
                            .from(
                                SharedSpaceConversation::Table,
                                SharedSpaceConversation::ConversationId,
                            )
                            .to(Conversation::Table, Conversation::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SharedSpaceConversationAgent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SharedSpaceConversationAgent::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversationAgent::SharedConversationId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversationAgent::AgentType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversationAgent::NativeSessionId)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversationAgent::LastUsedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversationAgent::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SharedSpaceConversationAgent::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_shared_space_conversation_agent_conversation")
                            .from(
                                SharedSpaceConversationAgent::Table,
                                SharedSpaceConversationAgent::SharedConversationId,
                            )
                            .to(SharedSpaceConversation::Table, SharedSpaceConversation::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_SHARED_SPACE_ACTIVE_CONVERSATION)
                    .table(SharedSpace::Table)
                    .col(SharedSpace::ActiveSharedConversationId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_SHARED_SPACE_CONVERSATION_SPACE)
                    .table(SharedSpaceConversation::Table)
                    .col(SharedSpaceConversation::SpaceId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_SHARED_SPACE_CONVERSATION_CONVERSATION)
                    .table(SharedSpaceConversation::Table)
                    .col(SharedSpaceConversation::ConversationId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_SHARED_SPACE_CONVERSATION_AGENT_CONV)
                    .table(SharedSpaceConversationAgent::Table)
                    .col(SharedSpaceConversationAgent::SharedConversationId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_SHARED_SPACE_CONVERSATION_AGENT_UNIQUE)
                    .table(SharedSpaceConversationAgent::Table)
                    .col(SharedSpaceConversationAgent::SharedConversationId)
                    .col(SharedSpaceConversationAgent::AgentType)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Best-effort migration from the previous per-agent binding model:
        // each old binding conversation becomes one shared-space conversation,
        // and the old binding's native session id becomes that conversation's
        // first per-agent native session record.
        let conn = manager.get_connection();
        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            "INSERT OR IGNORE INTO shared_space_conversation \
             (space_id, conversation_id, title, active_agent_type, created_at, updated_at, deleted_at) \
             SELECT b.space_id, b.conversation_id, c.title, b.agent_type, b.created_at, b.updated_at, NULL \
             FROM shared_space_agent_binding b \
             JOIN conversation c ON c.id = b.conversation_id \
             WHERE b.conversation_id IS NOT NULL \
             GROUP BY b.conversation_id"
                .to_string(),
        ))
        .await?;
        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            "INSERT OR IGNORE INTO shared_space_conversation_agent \
             (shared_conversation_id, agent_type, native_session_id, last_used_at, created_at, updated_at) \
             SELECT sc.id, b.agent_type, \
                    COALESCE(b.native_session_id, CASE WHEN c.agent_type = b.agent_type THEN c.external_id ELSE NULL END), \
                    b.last_used_at, b.created_at, b.updated_at \
             FROM shared_space_agent_binding b \
             JOIN shared_space_conversation sc ON sc.conversation_id = b.conversation_id \
             JOIN conversation c ON c.id = b.conversation_id \
             WHERE b.conversation_id IS NOT NULL"
                .to_string(),
        ))
        .await?;
        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            "UPDATE shared_space \
             SET active_shared_conversation_id = ( \
               SELECT sc.id \
               FROM shared_space_agent_binding b \
               JOIN shared_space_conversation sc ON sc.conversation_id = b.conversation_id \
               WHERE b.id = shared_space.active_agent_binding_id \
               LIMIT 1 \
             ) \
             WHERE active_agent_binding_id IS NOT NULL"
                .to_string(),
        ))
        .await?;
        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            "UPDATE shared_space \
             SET active_shared_conversation_id = ( \
               SELECT sc.id \
               FROM shared_space_conversation sc \
               WHERE sc.space_id = shared_space.id AND sc.deleted_at IS NULL \
               ORDER BY sc.updated_at DESC, sc.id DESC \
               LIMIT 1 \
             ) \
             WHERE active_shared_conversation_id IS NULL"
                .to_string(),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_SHARED_SPACE_CONVERSATION_AGENT_UNIQUE)
                    .table(SharedSpaceConversationAgent::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_SHARED_SPACE_CONVERSATION_AGENT_CONV)
                    .table(SharedSpaceConversationAgent::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_SHARED_SPACE_CONVERSATION_CONVERSATION)
                    .table(SharedSpaceConversation::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_SHARED_SPACE_CONVERSATION_SPACE)
                    .table(SharedSpaceConversation::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_SHARED_SPACE_ACTIVE_CONVERSATION)
                    .table(SharedSpace::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(SharedSpaceConversationAgent::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(SharedSpaceConversation::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(SharedSpace::Table)
                    .drop_column(SharedSpace::ActiveSharedConversationId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SharedSpace {
    Table,
    Id,
    ActiveSharedConversationId,
}

#[derive(DeriveIden)]
enum SharedSpaceConversation {
    Table,
    Id,
    SpaceId,
    ConversationId,
    Title,
    ActiveAgentType,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(DeriveIden)]
enum SharedSpaceConversationAgent {
    Table,
    Id,
    SharedConversationId,
    AgentType,
    NativeSessionId,
    LastUsedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Conversation {
    Table,
    Id,
}
