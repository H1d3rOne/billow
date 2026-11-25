use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "shared_space_conversation_agent")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub shared_conversation_id: i32,
    pub agent_type: String,
    pub native_session_id: Option<String>,
    pub last_used_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::shared_space_conversation::Entity",
        from = "Column::SharedConversationId",
        to = "super::shared_space_conversation::Column::Id"
    )]
    SharedConversation,
}

impl Related<super::shared_space_conversation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SharedConversation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
