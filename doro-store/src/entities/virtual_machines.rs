use sea_orm::entity::prelude::*;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "virtual_machines")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub host_id: Uuid,
    pub provider: String,
    pub vm_ref: String,
    pub name: String,
    pub status: String,
    pub image: String,
    pub cpu_cores: i32,
    pub memory_mib: i32,
    pub disk_gb: i32,
    pub networks: Value,
    pub console: Option<Value>,
    pub metadata: Value,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub observed_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
