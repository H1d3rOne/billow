use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "shared_space_conversation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub space_id: i32,
    pub conversation_id: i32,
    pub title: Option<String>,
    pub active_agent_type: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub deleted_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::shared_space::Entity",
        from = "Column::SpaceId",
        to = "super::shared_space::Column::Id"
    )]
    SharedSpace,

    #[sea_orm(
        belongs_to = "super::conversation::Entity",
        from = "Column::ConversationId",
        to = "super::conversation::Column::Id"
    )]
    Conversation,

    #[sea_orm(has_many = "super::shared_space_conversation_agent::Entity")]
    AgentSessions,
}

impl Related<super::shared_space::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SharedSpace.def()
    }
}

impl Related<super::conversation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Conversation.def()
    }
}

impl Related<super::shared_space_conversation_agent::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AgentSessions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
