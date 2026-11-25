use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "shared_space")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub folder_id: i32,
    pub active_agent_binding_id: Option<i32>,
    pub active_shared_conversation_id: Option<i32>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub deleted_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::folder::Entity",
        from = "Column::FolderId",
        to = "super::folder::Column::Id"
    )]
    Folder,

    #[sea_orm(has_many = "super::shared_space_agent_binding::Entity")]
    Bindings,

    #[sea_orm(has_many = "super::shared_space_conversation::Entity")]
    Conversations,

    #[sea_orm(has_many = "super::shared_space_message::Entity")]
    Messages,

    #[sea_orm(has_many = "super::shared_space_context_snapshot::Entity")]
    ContextSnapshots,
}

impl Related<super::folder::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Folder.def()
    }
}

impl Related<super::shared_space_agent_binding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bindings.def()
    }
}

impl Related<super::shared_space_conversation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Conversations.def()
    }
}

impl Related<super::shared_space_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Messages.def()
    }
}

impl Related<super::shared_space_context_snapshot::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContextSnapshots.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
