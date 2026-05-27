use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "operation_logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub source: String,
    pub actor: Option<String>,
    pub method: String,
    pub path: String,
    pub status_code: i32,
    pub latency_ms: i32,
    pub message: Option<String>,
    pub detail: Value,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
