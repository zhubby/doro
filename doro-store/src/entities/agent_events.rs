use sea_orm::entity::prelude::*;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "agent_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub recorded_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i64,
    pub host_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub event_type: String,
    pub event_json: Value,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
