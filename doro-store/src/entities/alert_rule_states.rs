use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "alert_rule_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub alert_rule_id: Uuid,
    pub host_id: Uuid,
    pub state: String,
    pub first_matched_at: Option<DateTimeWithTimeZone>,
    pub last_matched_at: Option<DateTimeWithTimeZone>,
    pub last_fired_at: Option<DateTimeWithTimeZone>,
    pub active_incident_id: Option<Uuid>,
    pub last_resolved_at: Option<DateTimeWithTimeZone>,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
