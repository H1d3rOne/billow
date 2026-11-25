use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "shared_space_message")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub space_id: i32,
    pub binding_id: Option<i32>,
    pub role: String,
    pub content: String,
    pub source_agent_type: Option<String>,
    pub native_message_id: Option<String>,
    pub created_at: DateTimeUtc,
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
        belongs_to = "super::shared_space_agent_binding::Entity",
        from = "Column::BindingId",
        to = "super::shared_space_agent_binding::Column::Id"
    )]
    Binding,
}

impl Related<super::shared_space::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SharedSpace.def()
    }
}

impl Related<super::shared_space_agent_binding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Binding.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
