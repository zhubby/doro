use sea_orm::entity::prelude::*;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "cron_jobs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub host_id: Option<Uuid>,
    pub name: String,
    pub schedule: String,
    pub status: String,
    pub task_template: Value,
    pub kind: String,
    pub required_capability: String,
    pub label_selector: Value,
    pub next_run_at: Option<DateTimeWithTimeZone>,
    pub last_run_at: Option<DateTimeWithTimeZone>,
    pub last_run_status: Option<String>,
    pub approval_task_id: Option<Uuid>,
    pub approved_at: Option<DateTimeWithTimeZone>,
    pub approved_by: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
