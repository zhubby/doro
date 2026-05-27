use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "refresh_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub expires_at: DateTimeWithTimeZone,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub last_used_at: Option<DateTimeWithTimeZone>,
    pub replaced_by_token_id: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
