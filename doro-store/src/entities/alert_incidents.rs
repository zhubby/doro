use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "alert_incidents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub alert_rule_id: Uuid,
    pub host_id: Uuid,
    pub rule_name: String,
    pub severity: String,
    pub metric_source: String,
    pub metric_key: String,
    pub operator: String,
    pub threshold: f32,
    pub observed_value: f32,
    pub status: String,
    pub triggered_at: DateTimeWithTimeZone,
    pub resolved_at: Option<DateTimeWithTimeZone>,
    pub last_observed_at: DateTimeWithTimeZone,
    pub notification_count: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
