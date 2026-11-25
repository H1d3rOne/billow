use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "shared_space_context_snapshot")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub space_id: i32,
    pub summary: String,
    pub recent_messages_json: String,
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
}

impl Related<super::shared_space::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SharedSpace.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
