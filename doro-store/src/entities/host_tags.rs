use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "host_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub host_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tag: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
