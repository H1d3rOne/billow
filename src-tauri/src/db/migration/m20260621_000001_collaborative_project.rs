use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const IDX_PROJECT_FOLDER: &str = "idx_project_folder_id";
const IDX_PROJECT_MANAGER_CONVERSATION: &str = "idx_project_manager_conversation_id";
const IDX_PROJECT_ROLE_PROJECT: &str = "idx_project_role_project_id";
const IDX_PROJECT_ROLE_CONVERSATION: &str = "idx_project_role_conversation_id";
const IDX_PROJECT_ROLE_KEY: &str = "idx_project_role_project_role_key";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Project::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Project::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Project::Name).string().not_null())
                    .col(ColumnDef::new(Project::FolderId).integer().not_null())
                    .col(ColumnDef::new(Project::ManagerRoleId).integer().null())
                    .col(
                        ColumnDef::new(Project::ManagerConversationId)
                            .integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Project::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Project::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Project::DeletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_project_folder")
                            .from(Project::Table, Project::FolderId)
                            .to(Folder::Table, Folder::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_project_manager_conversation")
                            .from(Project::Table, Project::ManagerConversationId)
                            .to(Conversation::Table, Conversation::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProjectRole::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProjectRole::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ProjectRole::ProjectId).integer().not_null())
                    .col(ColumnDef::new(ProjectRole::Name).string().not_null())
                    .col(ColumnDef::new(ProjectRole::RoleKey).string().not_null())
                    .col(ColumnDef::new(ProjectRole::AgentType).string().not_null())
                    .col(ColumnDef::new(ProjectRole::Instructions).text().null())
                    .col(
                        ColumnDef::new(ProjectRole::IsManager)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(ProjectRole::ConversationId).integer().null())
                    .col(
                        ColumnDef::new(ProjectRole::SortOrder)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProjectRole::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProjectRole::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_project_role_project")
                            .from(ProjectRole::Table, ProjectRole::ProjectId)
                            .to(Project::Table, Project::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_project_role_conversation")
                            .from(ProjectRole::Table, ProjectRole::ConversationId)
                            .to(Conversation::Table, Conversation::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_PROJECT_FOLDER)
                    .table(Project::Table)
                    .col(Project::FolderId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_PROJECT_MANAGER_CONVERSATION)
                    .table(Project::Table)
                    .col(Project::ManagerConversationId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_PROJECT_ROLE_PROJECT)
                    .table(ProjectRole::Table)
                    .col(ProjectRole::ProjectId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_PROJECT_ROLE_CONVERSATION)
                    .table(ProjectRole::Table)
                    .col(ProjectRole::ConversationId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(IDX_PROJECT_ROLE_KEY)
                    .table(ProjectRole::Table)
                    .col(ProjectRole::ProjectId)
                    .col(ProjectRole::RoleKey)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_PROJECT_ROLE_KEY)
                    .table(ProjectRole::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_PROJECT_ROLE_CONVERSATION)
                    .table(ProjectRole::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_PROJECT_ROLE_PROJECT)
                    .table(ProjectRole::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_PROJECT_MANAGER_CONVERSATION)
                    .table(Project::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(IDX_PROJECT_FOLDER)
                    .table(Project::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ProjectRole::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Project::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Project {
    Table,
    Id,
    Name,
    FolderId,
    ManagerRoleId,
    ManagerConversationId,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(DeriveIden)]
enum ProjectRole {
    Table,
    Id,
    ProjectId,
    Name,
    RoleKey,
    AgentType,
    Instructions,
    IsManager,
    ConversationId,
    SortOrder,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Folder {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Conversation {
    Table,
    Id,
}
