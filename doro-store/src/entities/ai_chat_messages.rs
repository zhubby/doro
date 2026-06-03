use sea_orm::entity::prelude::*;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ai_chat_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub status: String,
    pub content: String,
    pub task_id: Option<Uuid>,
    pub host_id: Option<Uuid>,
    pub ai_provider_id: Option<Uuid>,
    pub model: Option<String>,
    pub metadata: Value,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
