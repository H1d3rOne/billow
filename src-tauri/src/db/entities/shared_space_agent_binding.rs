use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "shared_space_agent_binding")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub space_id: i32,
    pub agent_type: String,
    pub conversation_id: Option<i32>,
    pub native_session_id: Option<String>,
    pub last_used_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
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

    #[sea_orm(has_many = "super::shared_space_message::Entity")]
    Messages,
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

impl Related<super::shared_space_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Messages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
