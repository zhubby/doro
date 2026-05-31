use sea_orm::entity::prelude::*;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "websites")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub host_id: Option<Uuid>,
    pub name: String,
    pub primary_domain: String,
    pub aliases: Value,
    pub status: String,
    pub kind: String,
    pub protocol: String,
    pub listen_port: i32,
    pub upstream_url: String,
    pub app_install_id: Option<Uuid>,
    pub tls_certificate_id: Option<Uuid>,
    pub config: Value,
    pub notes: Option<String>,
    pub last_runtime_error: Option<String>,
    pub last_checked_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
