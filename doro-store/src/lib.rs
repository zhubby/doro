use chrono::DateTime;
use chrono::Utc;
use doro_config::StoreBackend;
use doro_config::StoreConfig;
use doro_protocol::AgentCapability;
use doro_protocol::AppSummary;
use doro_protocol::ApprovalRequest;
use doro_protocol::ApprovalStatus;
use doro_protocol::CapabilityName;
use doro_protocol::CapabilityRisk;
use doro_protocol::Host;
use doro_protocol::HostContainer;
use doro_protocol::HostStatus;
use doro_protocol::MetricSnapshot;
use doro_protocol::Task;
use doro_protocol::TaskStatus;
use doro_protocol::TaskStep;
use sea_orm::ActiveModelTrait;
use sea_orm::ColumnTrait;
use sea_orm::ConnectOptions;
use sea_orm::ConnectionTrait;
use sea_orm::Database;
use sea_orm::DatabaseBackend;
use sea_orm::DatabaseConnection;
use sea_orm::DbErr;
use sea_orm::EntityTrait;
use sea_orm::Order;
use sea_orm::QueryFilter;
use sea_orm::QueryOrder;
use sea_orm::QuerySelect;
use sea_orm::Set;
use sea_orm::Statement;
use sea_orm::TransactionTrait;
use serde_json::Value;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

const HOST_ONLINE_TTL_SECONDS: i64 = 90;

pub mod entities;
mod migrations;

#[derive(Debug, Clone)]
pub struct Store {
    connection: DatabaseConnection,
    backend: DatabaseBackend,
}

#[derive(Debug, Clone)]
pub struct NewTask {
    pub id: Uuid,
    pub host_id: Option<Uuid>,
    pub title: String,
    pub prompt: Option<String>,
    pub status: TaskStatus,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub steps: Vec<TaskStep>,
}

#[derive(Debug, Clone)]
pub struct NewApproval {
    pub id: Uuid,
    pub task_id: Uuid,
    pub step_id: Uuid,
    pub reason: String,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AgentRegistration {
    pub agent_id: Uuid,
    pub host_id: Uuid,
    pub enrollment_token: String,
    pub hostname: String,
    pub system_profile: Value,
    pub capabilities: Vec<AgentCapability>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AgentHeartbeat {
    pub agent_id: Uuid,
    pub host_id: Uuid,
    pub capabilities: Vec<AgentCapability>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewAgentEvent {
    pub agent_id: Option<Uuid>,
    pub host_id: Option<Uuid>,
    pub event_type: String,
    pub event_json: Value,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewMetricSnapshot {
    pub host_id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub disk_percent: f32,
    pub load_average: f32,
    pub extra: Value,
}

#[derive(Debug, Clone)]
pub struct NewContainerObservation {
    pub host_id: Uuid,
    pub runtime: String,
    pub container_ref: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Value,
    pub labels: Value,
    pub created_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub password_hash: String,
    pub role: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct NewRefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredRefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewEnrollmentToken {
    pub id: Uuid,
    pub label: String,
    pub token: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredEnrollmentToken {
    pub id: Uuid,
    pub label: String,
    pub token_hash: String,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub used_at: Option<DateTime<Utc>>,
    pub used_by_agent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl Store {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let connection = Database::connect(database_url).await?;
        Ok(Self {
            connection,
            backend: DatabaseBackend::Postgres,
        })
    }

    pub async fn connect_with_config(config: &StoreConfig) -> anyhow::Result<Self> {
        let mut options = ConnectOptions::new(config.database_url.clone());
        options
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .connect_timeout(Duration::from_secs(config.connect_timeout_seconds))
            .idle_timeout(Duration::from_secs(config.idle_timeout_seconds));

        let connection = Database::connect(options).await?;
        Ok(Self {
            connection,
            backend: database_backend(config.backend),
        })
    }

    pub fn from_connection(connection: DatabaseConnection, backend: DatabaseBackend) -> Self {
        Self {
            connection,
            backend,
        }
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        self.execute_sql_batch(migrations::SCHEMA_MIGRATIONS.sql)
            .await?;

        for migration in migrations::all() {
            if self.migration_applied(migration.id).await? {
                continue;
            }
            self.execute_sql_batch(migration.sql).await?;
            self.record_migration(migration.id).await?;
        }

        Ok(())
    }

    pub fn connection(&self) -> &DatabaseConnection {
        &self.connection
    }

    pub fn hosts(&self) -> HostRepository<'_> {
        HostRepository { store: self }
    }

    pub fn agents(&self) -> AgentRepository<'_> {
        AgentRepository { store: self }
    }

    pub fn tasks(&self) -> TaskRepository<'_> {
        TaskRepository { store: self }
    }

    pub fn approvals(&self) -> ApprovalRepository<'_> {
        ApprovalRepository { store: self }
    }

    pub fn events(&self) -> EventRepository<'_> {
        EventRepository { store: self }
    }

    pub fn metrics(&self) -> MetricRepository<'_> {
        MetricRepository { store: self }
    }

    pub fn containers(&self) -> ContainerRepository<'_> {
        ContainerRepository { store: self }
    }

    pub fn settings(&self) -> SettingsRepository<'_> {
        SettingsRepository { store: self }
    }

    pub fn apps(&self) -> AppRepository<'_> {
        AppRepository { store: self }
    }

    pub fn users(&self) -> UserRepository<'_> {
        UserRepository { store: self }
    }

    pub fn refresh_tokens(&self) -> RefreshTokenRepository<'_> {
        RefreshTokenRepository { store: self }
    }

    pub fn enrollment_tokens(&self) -> EnrollmentTokenRepository<'_> {
        EnrollmentTokenRepository { store: self }
    }

    async fn execute_sql(&self, sql: &str) -> Result<(), DbErr> {
        let statement = Statement::from_string(self.backend, sql.to_string());
        self.connection.execute_raw(statement).await?;
        Ok(())
    }

    async fn execute_sql_batch(&self, sql: &str) -> Result<(), DbErr> {
        for statement in migrations::split_sql_statements(sql) {
            self.execute_sql(&statement).await?;
        }
        Ok(())
    }

    async fn migration_applied(&self, id: &str) -> Result<bool, DbErr> {
        let sql = format!(
            "SELECT 1 AS applied FROM doro_schema_migrations WHERE id = '{}' LIMIT 1;",
            id.replace('\'', "''")
        );
        let statement = Statement::from_string(self.backend, sql);
        self.connection
            .query_one_raw(statement)
            .await
            .map(|row| row.is_some())
    }

    async fn record_migration(&self, id: &str) -> Result<(), DbErr> {
        let sql = format!(
            "INSERT INTO doro_schema_migrations (id) VALUES ('{}') ON CONFLICT (id) DO NOTHING;",
            id.replace('\'', "''")
        );
        self.execute_sql(&sql).await
    }
}

pub struct HostRepository<'a> {
    store: &'a Store,
}

impl HostRepository<'_> {
    pub async fn list(&self) -> Result<Vec<Host>, DbErr> {
        let hosts = entities::hosts::Entity::find()
            .order_by(entities::hosts::Column::Hostname, Order::Asc)
            .all(self.store.connection())
            .await?;
        let mut items = Vec::with_capacity(hosts.len());
        for host in hosts {
            items.push(self.to_protocol(host).await?);
        }
        Ok(items)
    }

    pub async fn delete(&self, host_id: Uuid) -> Result<bool, DbErr> {
        let result = entities::hosts::Entity::delete_by_id(host_id)
            .exec(self.store.connection())
            .await?;
        Ok(result.rows_affected > 0)
    }

    pub async fn update(
        &self,
        host_id: Uuid,
        display_name: String,
        labels: Vec<String>,
    ) -> Result<Host, DbErr> {
        let display_name = display_name.trim();
        if display_name.is_empty() {
            return Err(DbErr::Custom("display_name is required".to_string()));
        }

        let normalized_labels = normalize_labels(labels);
        let now = Utc::now();
        let result = entities::hosts::Entity::update_many()
            .col_expr(
                entities::hosts::Column::DisplayName,
                sea_orm::sea_query::Expr::value(display_name),
            )
            .col_expr(
                entities::hosts::Column::Labels,
                sea_orm::sea_query::Expr::value(json!(normalized_labels)),
            )
            .col_expr(
                entities::hosts::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(entities::hosts::Column::Id.eq(host_id))
            .exec(self.store.connection())
            .await?;

        if result.rows_affected == 0 {
            return Err(DbErr::RecordNotFound(format!("host {host_id} not found")));
        }

        let host = entities::hosts::Entity::find_by_id(host_id)
            .one(self.store.connection())
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(format!("host {host_id} not found")))?;

        self.to_protocol(host).await
    }

    pub async fn upsert_observed(
        &self,
        host_id: Uuid,
        hostname: String,
        observed_at: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        let now = Utc::now();
        let model = entities::hosts::ActiveModel {
            id: Set(host_id),
            hostname: Set(hostname.clone()),
            display_name: Set(hostname),
            status: Set(serialize_host_status(HostStatus::Online)),
            labels: Set(json!(["agent"])),
            system_profile: Set(json!({})),
            last_seen_at: Set(Some(observed_at.into())),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };
        entities::hosts::Entity::insert(model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(entities::hosts::Column::Id)
                    .update_columns([
                        entities::hosts::Column::Hostname,
                        entities::hosts::Column::Status,
                        entities::hosts::Column::LastSeenAt,
                        entities::hosts::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(self.store.connection())
            .await?;
        Ok(())
    }

    async fn to_protocol(&self, host: entities::hosts::Model) -> Result<Host, DbErr> {
        let capabilities = entities::agent_capabilities::Entity::find()
            .filter(entities::agent_capabilities::Column::HostId.eq(host.id))
            .order_by(entities::agent_capabilities::Column::Name, Order::Asc)
            .all(self.store.connection())
            .await?
            .into_iter()
            .filter_map(|capability| {
                Some(AgentCapability {
                    name: parse_capability_name(&capability.name)?,
                    risk: parse_capability_risk(&capability.risk)?,
                    description: capability.description,
                })
            })
            .collect();

        let status = current_host_status(&host);
        let last_seen_at = host.last_seen_at.map(Into::into);

        Ok(Host {
            id: host.id,
            hostname: host.hostname,
            display_name: host.display_name,
            labels: json_array_strings(host.labels),
            status,
            last_seen_at,
            capabilities,
            system_profile: host.system_profile,
        })
    }
}

pub struct AgentRepository<'a> {
    store: &'a Store,
}

impl AgentRepository<'_> {
    pub async fn register(&self, registration: AgentRegistration) -> Result<(), DbErr> {
        let transaction = self.store.connection().begin().await?;
        let token = find_active_enrollment_token(
            &transaction,
            &registration.enrollment_token,
            registration.observed_at,
        )
        .await?;
        upsert_host(
            &transaction,
            registration.host_id,
            registration.hostname,
            registration.system_profile.clone(),
            registration.observed_at,
        )
        .await?;
        upsert_agent(
            &transaction,
            registration.agent_id,
            registration.host_id,
            registration.observed_at,
            "enrolled",
        )
        .await?;
        replace_capabilities(
            &transaction,
            registration.agent_id,
            registration.host_id,
            registration.capabilities,
            registration.observed_at,
        )
        .await?;
        consume_enrollment_token(
            &transaction,
            token.id,
            registration.agent_id,
            registration.observed_at,
        )
        .await?;
        insert_agent_event(
            &transaction,
            NewAgentEvent {
                agent_id: Some(registration.agent_id),
                host_id: Some(registration.host_id),
                event_type: "agent_enrolled".to_string(),
                event_json: json!({
                    "agent_id": registration.agent_id,
                    "host_id": registration.host_id,
                    "system_profile": registration.system_profile
                }),
                recorded_at: registration.observed_at,
            },
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn heartbeat(&self, heartbeat: AgentHeartbeat) -> Result<(), DbErr> {
        let transaction = self.store.connection().begin().await?;
        ensure_host_exists(&transaction, heartbeat.host_id).await?;
        upsert_agent(
            &transaction,
            heartbeat.agent_id,
            heartbeat.host_id,
            heartbeat.observed_at,
            "online",
        )
        .await?;
        entities::hosts::Entity::update_many()
            .col_expr(
                entities::hosts::Column::Status,
                sea_orm::sea_query::Expr::value(serialize_host_status(HostStatus::Online)),
            )
            .col_expr(
                entities::hosts::Column::LastSeenAt,
                sea_orm::sea_query::Expr::value(heartbeat.observed_at),
            )
            .col_expr(
                entities::hosts::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(entities::hosts::Column::Id.eq(heartbeat.host_id))
            .exec(&transaction)
            .await?;
        replace_capabilities(
            &transaction,
            heartbeat.agent_id,
            heartbeat.host_id,
            heartbeat.capabilities,
            heartbeat.observed_at,
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn mark_online(
        &self,
        agent_id: Uuid,
        host_id: Uuid,
        observed_at: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        let transaction = self.store.connection().begin().await?;
        ensure_host_exists(&transaction, host_id).await?;
        upsert_agent(&transaction, agent_id, host_id, observed_at, "online").await?;
        entities::hosts::Entity::update_many()
            .col_expr(
                entities::hosts::Column::Status,
                sea_orm::sea_query::Expr::value(serialize_host_status(HostStatus::Online)),
            )
            .col_expr(
                entities::hosts::Column::LastSeenAt,
                sea_orm::sea_query::Expr::value(observed_at),
            )
            .col_expr(
                entities::hosts::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(entities::hosts::Column::Id.eq(host_id))
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn mark_offline(
        &self,
        agent_id: Uuid,
        host_id: Uuid,
        observed_at: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        let transaction = self.store.connection().begin().await?;
        entities::agents::Entity::update_many()
            .col_expr(
                entities::agents::Column::Status,
                sea_orm::sea_query::Expr::value("offline"),
            )
            .col_expr(
                entities::agents::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(entities::agents::Column::Id.eq(agent_id))
            .exec(&transaction)
            .await?;
        entities::hosts::Entity::update_many()
            .col_expr(
                entities::hosts::Column::Status,
                sea_orm::sea_query::Expr::value(serialize_host_status(HostStatus::Offline)),
            )
            .col_expr(
                entities::hosts::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(observed_at),
            )
            .filter(entities::hosts::Column::Id.eq(host_id))
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

pub struct TaskRepository<'a> {
    store: &'a Store,
}

impl TaskRepository<'_> {
    pub async fn list(&self) -> Result<Vec<Task>, DbErr> {
        let tasks = entities::tasks::Entity::find()
            .order_by(entities::tasks::Column::CreatedAt, Order::Desc)
            .all(self.store.connection())
            .await?;
        let mut items = Vec::with_capacity(tasks.len());
        for task in tasks {
            items.push(self.to_protocol(task).await?);
        }
        Ok(items)
    }

    pub async fn create_with_steps(&self, new_task: NewTask) -> Result<Task, DbErr> {
        let transaction = self.store.connection().begin().await?;
        let now = Utc::now();
        let task_model = entities::tasks::ActiveModel {
            id: Set(new_task.id),
            host_id: Set(new_task.host_id),
            title: Set(new_task.title.clone()),
            prompt: Set(new_task.prompt),
            status: Set(serialize_task_status(new_task.status)),
            created_by: Set(new_task.created_by),
            created_at: Set(new_task.created_at.into()),
            queued_at: Set(if new_task.status == TaskStatus::Queued {
                Some(now.into())
            } else {
                None
            }),
            started_at: Set(None),
            finished_at: Set(None),
            error_message: Set(None),
            metadata: Set(json!({})),
        };
        task_model.insert(&transaction).await?;

        for (position, step) in new_task.steps.iter().enumerate() {
            entities::task_steps::ActiveModel {
                id: Set(step.id),
                task_id: Set(new_task.id),
                position: Set(position as i32),
                capability: Set(serialize_capability_name(step.capability)),
                risk: Set(serialize_capability_risk(step.risk)),
                summary: Set(step.summary.clone()),
                payload: Set(step.payload.clone()),
                status: Set("pending".to_string()),
                created_at: Set(new_task.created_at.into()),
            }
            .insert(&transaction)
            .await?;

            if step.risk >= CapabilityRisk::High {
                entities::approvals::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    task_id: Set(new_task.id),
                    step_id: Set(step.id),
                    reason: Set(format!("step requires {:?} capability approval", step.risk)),
                    status: Set(serialize_approval_status(ApprovalStatus::Pending)),
                    requested_at: Set(new_task.created_at.into()),
                    expires_at: Set((new_task.created_at + chrono::Duration::hours(24)).into()),
                    resolved_at: Set(None),
                    resolved_by: Set(None),
                    decision_note: Set(None),
                }
                .insert(&transaction)
                .await?;
            }
        }

        transaction.commit().await?;
        Ok(Task {
            id: new_task.id,
            host_id: new_task.host_id,
            title: new_task.title,
            status: new_task.status,
            created_at: new_task.created_at,
            steps: new_task.steps,
        })
    }

    async fn to_protocol(&self, task: entities::tasks::Model) -> Result<Task, DbErr> {
        let steps = entities::task_steps::Entity::find()
            .filter(entities::task_steps::Column::TaskId.eq(task.id))
            .order_by(entities::task_steps::Column::Position, Order::Asc)
            .all(self.store.connection())
            .await?
            .into_iter()
            .filter_map(|step| {
                Some(TaskStep {
                    id: step.id,
                    capability: parse_capability_name(&step.capability)?,
                    risk: parse_capability_risk(&step.risk)?,
                    summary: step.summary,
                    payload: step.payload,
                })
            })
            .collect();

        Ok(Task {
            id: task.id,
            host_id: task.host_id,
            title: task.title,
            status: parse_task_status(&task.status).unwrap_or(TaskStatus::Draft),
            created_at: task.created_at.into(),
            steps,
        })
    }
}

pub struct ApprovalRepository<'a> {
    store: &'a Store,
}

impl ApprovalRepository<'_> {
    pub async fn list(&self) -> Result<Vec<ApprovalRequest>, DbErr> {
        self.expire_pending(Utc::now()).await?;
        let approvals = entities::approvals::Entity::find()
            .order_by(entities::approvals::Column::RequestedAt, Order::Desc)
            .all(self.store.connection())
            .await?;
        Ok(approvals
            .into_iter()
            .map(approval_model_to_protocol)
            .collect())
    }

    pub async fn create(&self, approval: NewApproval) -> Result<ApprovalRequest, DbErr> {
        let step = entities::task_steps::Entity::find()
            .filter(entities::task_steps::Column::Id.eq(approval.step_id))
            .filter(entities::task_steps::Column::TaskId.eq(approval.task_id))
            .one(self.store.connection())
            .await?;
        if step.is_none() {
            return Err(DbErr::RecordNotFound(
                "task step not found for approval".to_string(),
            ));
        }

        let model = entities::approvals::ActiveModel {
            id: Set(approval.id),
            task_id: Set(approval.task_id),
            step_id: Set(approval.step_id),
            reason: Set(approval.reason),
            status: Set(serialize_approval_status(ApprovalStatus::Pending)),
            requested_at: Set(approval.requested_at.into()),
            expires_at: Set(approval.expires_at.into()),
            resolved_at: Set(None),
            resolved_by: Set(None),
            decision_note: Set(None),
        }
        .insert(self.store.connection())
        .await?;

        Ok(approval_model_to_protocol(model))
    }

    pub async fn delete(&self, approval_id: Uuid) -> Result<bool, DbErr> {
        let result = entities::approvals::Entity::delete_by_id(approval_id)
            .exec(self.store.connection())
            .await?;
        Ok(result.rows_affected > 0)
    }

    pub async fn approve(
        &self,
        approval_id: Uuid,
        resolved_by: String,
        decision_note: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<ApprovalRequest, DbErr> {
        self.resolve(
            approval_id,
            ApprovalStatus::Approved,
            resolved_by,
            decision_note,
            now,
        )
        .await
    }

    pub async fn deny(
        &self,
        approval_id: Uuid,
        resolved_by: String,
        decision_note: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<ApprovalRequest, DbErr> {
        self.resolve(
            approval_id,
            ApprovalStatus::Denied,
            resolved_by,
            decision_note,
            now,
        )
        .await
    }

    async fn resolve(
        &self,
        approval_id: Uuid,
        status: ApprovalStatus,
        resolved_by: String,
        decision_note: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<ApprovalRequest, DbErr> {
        self.expire_pending(now).await?;
        let approval = entities::approvals::Entity::find_by_id(approval_id)
            .one(self.store.connection())
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("approval not found".to_string()))?;
        match parse_approval_status(&approval.status).unwrap_or(ApprovalStatus::Pending) {
            ApprovalStatus::Pending => {}
            ApprovalStatus::Expired => return Err(DbErr::Custom("approval expired".to_string())),
            ApprovalStatus::Approved | ApprovalStatus::Denied => {
                return Err(DbErr::Custom("approval already resolved".to_string()));
            }
        }

        let model = entities::approvals::ActiveModel {
            id: Set(approval.id),
            task_id: Set(approval.task_id),
            step_id: Set(approval.step_id),
            reason: Set(approval.reason),
            status: Set(serialize_approval_status(status)),
            requested_at: Set(approval.requested_at),
            expires_at: Set(approval.expires_at),
            resolved_at: Set(Some(now.into())),
            resolved_by: Set(Some(resolved_by)),
            decision_note: Set(decision_note),
        }
        .update(self.store.connection())
        .await?;

        Ok(approval_model_to_protocol(model))
    }

    async fn expire_pending(&self, now: DateTime<Utc>) -> Result<(), DbErr> {
        entities::approvals::Entity::update_many()
            .col_expr(
                entities::approvals::Column::Status,
                sea_orm::sea_query::Expr::value(serialize_approval_status(ApprovalStatus::Expired)),
            )
            .col_expr(
                entities::approvals::Column::ResolvedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .col_expr(
                entities::approvals::Column::ResolvedBy,
                sea_orm::sea_query::Expr::value("system"),
            )
            .col_expr(
                entities::approvals::Column::DecisionNote,
                sea_orm::sea_query::Expr::value("approval expired"),
            )
            .filter(
                entities::approvals::Column::Status
                    .eq(serialize_approval_status(ApprovalStatus::Pending)),
            )
            .filter(entities::approvals::Column::ExpiresAt.lte(now))
            .exec(self.store.connection())
            .await?;
        Ok(())
    }
}

fn approval_model_to_protocol(approval: entities::approvals::Model) -> ApprovalRequest {
    ApprovalRequest {
        id: approval.id,
        task_id: approval.task_id,
        step_id: approval.step_id,
        reason: approval.reason,
        status: parse_approval_status(&approval.status).unwrap_or(ApprovalStatus::Pending),
        requested_at: approval.requested_at.into(),
        expires_at: approval.expires_at.into(),
        resolved_at: approval.resolved_at.map(Into::into),
        resolved_by: approval.resolved_by,
        decision_note: approval.decision_note,
    }
}

pub struct EventRepository<'a> {
    store: &'a Store,
}

impl EventRepository<'_> {
    pub async fn record(&self, event: NewAgentEvent) -> Result<(), DbErr> {
        insert_agent_event(self.store.connection(), event).await
    }
}

pub struct MetricRepository<'a> {
    store: &'a Store,
}

impl MetricRepository<'_> {
    pub async fn record(&self, snapshot: NewMetricSnapshot) -> Result<(), DbErr> {
        entities::metric_snapshots::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            host_id: Set(snapshot.host_id),
            captured_at: Set(snapshot.captured_at.into()),
            cpu_percent: Set(snapshot.cpu_percent),
            memory_percent: Set(snapshot.memory_percent),
            disk_percent: Set(snapshot.disk_percent),
            load_average: Set(snapshot.load_average),
            extra: Set(snapshot.extra),
        }
        .insert(self.store.connection())
        .await?;
        Ok(())
    }

    pub async fn latest_for_host(&self, host_id: Uuid) -> Result<Option<MetricSnapshot>, DbErr> {
        let snapshot = entities::metric_snapshots::Entity::find()
            .filter(entities::metric_snapshots::Column::HostId.eq(host_id))
            .order_by(entities::metric_snapshots::Column::CapturedAt, Order::Desc)
            .one(self.store.connection())
            .await?;
        Ok(snapshot.map(|snapshot| MetricSnapshot {
            host_id: snapshot.host_id,
            captured_at: snapshot.captured_at.into(),
            cpu_percent: snapshot.cpu_percent,
            memory_percent: snapshot.memory_percent,
            disk_percent: snapshot.disk_percent,
            load_average: snapshot.load_average,
            extra: snapshot.extra,
        }))
    }

    pub async fn recent_for_host(
        &self,
        host_id: Uuid,
        limit: u64,
    ) -> Result<Vec<MetricSnapshot>, DbErr> {
        let snapshots = entities::metric_snapshots::Entity::find()
            .filter(entities::metric_snapshots::Column::HostId.eq(host_id))
            .order_by(entities::metric_snapshots::Column::CapturedAt, Order::Desc)
            .limit(limit)
            .all(self.store.connection())
            .await?;
        let mut snapshots = snapshots
            .into_iter()
            .map(|snapshot| MetricSnapshot {
                host_id: snapshot.host_id,
                captured_at: snapshot.captured_at.into(),
                cpu_percent: snapshot.cpu_percent,
                memory_percent: snapshot.memory_percent,
                disk_percent: snapshot.disk_percent,
                load_average: snapshot.load_average,
                extra: snapshot.extra,
            })
            .collect::<Vec<_>>();
        snapshots.reverse();
        Ok(snapshots)
    }

    pub fn from_protocol(snapshot: MetricSnapshot) -> NewMetricSnapshot {
        NewMetricSnapshot {
            host_id: snapshot.host_id,
            captured_at: snapshot.captured_at,
            cpu_percent: snapshot.cpu_percent,
            memory_percent: snapshot.memory_percent,
            disk_percent: snapshot.disk_percent,
            load_average: snapshot.load_average,
            extra: snapshot.extra,
        }
    }
}

pub struct ContainerRepository<'a> {
    store: &'a Store,
}

impl ContainerRepository<'_> {
    pub async fn upsert_many(&self, containers: Vec<NewContainerObservation>) -> Result<(), DbErr> {
        for container in containers {
            entities::containers::Entity::insert(entities::containers::ActiveModel {
                id: Set(Uuid::new_v4()),
                host_id: Set(container.host_id),
                runtime: Set(container.runtime),
                container_ref: Set(container.container_ref),
                name: Set(container.name),
                image: Set(container.image),
                status: Set(container.status),
                ports: Set(container.ports),
                labels: Set(container.labels),
                created_at: Set(container.created_at.map(Into::into)),
                observed_at: Set(container.observed_at.into()),
            })
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    entities::containers::Column::HostId,
                    entities::containers::Column::Runtime,
                    entities::containers::Column::ContainerRef,
                ])
                .update_columns([
                    entities::containers::Column::Name,
                    entities::containers::Column::Image,
                    entities::containers::Column::Status,
                    entities::containers::Column::Ports,
                    entities::containers::Column::Labels,
                    entities::containers::Column::CreatedAt,
                    entities::containers::Column::ObservedAt,
                ])
                .to_owned(),
            )
            .exec(self.store.connection())
            .await?;
        }
        Ok(())
    }

    pub async fn list_by_host(&self, host_id: Uuid) -> Result<Vec<HostContainer>, DbErr> {
        let rows = entities::containers::Entity::find()
            .filter(entities::containers::Column::HostId.eq(host_id))
            .order_by(entities::containers::Column::Name, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(|container| HostContainer {
                id: container.id,
                host_id: container.host_id,
                runtime: container.runtime,
                container_ref: container.container_ref,
                name: container.name,
                image: container.image,
                status: container.status,
                ports: container.ports,
                labels: container.labels,
                created_at: container.created_at.map(Into::into),
                observed_at: container.observed_at.into(),
            })
            .collect())
    }
}

pub struct SettingsRepository<'a> {
    store: &'a Store,
}

impl SettingsRepository<'_> {
    pub async fn get_json(&self, key: &str) -> Result<Option<Value>, DbErr> {
        let setting = entities::settings::Entity::find_by_id(key.to_string())
            .one(self.store.connection())
            .await?;
        Ok(setting.map(|setting| setting.value))
    }

    pub async fn upsert_json(
        &self,
        key: &str,
        value: Value,
        description: Option<String>,
    ) -> Result<(), DbErr> {
        entities::settings::Entity::insert(entities::settings::ActiveModel {
            key: Set(key.to_string()),
            value: Set(value),
            description: Set(description),
            updated_at: Set(Utc::now().into()),
        })
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(entities::settings::Column::Key)
                .update_columns([
                    entities::settings::Column::Value,
                    entities::settings::Column::Description,
                    entities::settings::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(self.store.connection())
        .await?;
        Ok(())
    }
}

pub struct AppRepository<'a> {
    store: &'a Store,
}

pub struct UserRepository<'a> {
    store: &'a Store,
}

impl UserRepository<'_> {
    pub async fn registration_open(&self) -> Result<bool, DbErr> {
        let active_user = entities::users::Entity::find()
            .filter(entities::users::Column::Status.eq("active"))
            .one(self.store.connection())
            .await?;
        Ok(active_user.is_none())
    }

    pub async fn create_first_admin(&self, user: NewUser) -> Result<StoredUser, DbErr> {
        let transaction = self.store.connection().begin().await?;
        let active_user = entities::users::Entity::find()
            .filter(entities::users::Column::Status.eq("active"))
            .one(&transaction)
            .await?;
        if active_user.is_some() {
            return Err(DbErr::Custom("registration is closed".to_string()));
        }

        let model = entities::users::ActiveModel {
            id: Set(user.id),
            username: Set(user.username),
            display_name: Set(user.display_name),
            password_hash: Set(user.password_hash),
            role: Set(user.role),
            status: Set("active".to_string()),
            created_at: Set(user.created_at.into()),
            updated_at: Set(user.created_at.into()),
            last_login_at: Set(None),
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        Ok(stored_user(model))
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<StoredUser>, DbErr> {
        let user = entities::users::Entity::find()
            .filter(entities::users::Column::Username.eq(username))
            .one(self.store.connection())
            .await?;
        Ok(user.map(stored_user))
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<StoredUser>, DbErr> {
        let user = entities::users::Entity::find_by_id(id)
            .one(self.store.connection())
            .await?;
        Ok(user.map(stored_user))
    }

    pub async fn mark_login(&self, id: Uuid, at: DateTime<Utc>) -> Result<(), DbErr> {
        entities::users::Entity::update_many()
            .col_expr(
                entities::users::Column::LastLoginAt,
                sea_orm::sea_query::Expr::value(at),
            )
            .col_expr(
                entities::users::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(entities::users::Column::Id.eq(id))
            .exec(self.store.connection())
            .await?;
        Ok(())
    }
}

pub struct RefreshTokenRepository<'a> {
    store: &'a Store,
}

pub struct EnrollmentTokenRepository<'a> {
    store: &'a Store,
}

impl EnrollmentTokenRepository<'_> {
    pub async fn create(&self, token: NewEnrollmentToken) -> Result<StoredEnrollmentToken, DbErr> {
        let model = entities::enrollment_tokens::ActiveModel {
            id: Set(token.id),
            label: Set(token.label),
            token_hash: Set(hash_token(&token.token)),
            status: Set("active".to_string()),
            expires_at: Set(token.expires_at.map(Into::into)),
            used_at: Set(None),
            used_by_agent_id: Set(None),
            created_at: Set(token.created_at.into()),
        }
        .insert(self.store.connection())
        .await?;
        Ok(stored_enrollment_token(model))
    }

    pub async fn find_by_token(&self, token: &str) -> Result<Option<StoredEnrollmentToken>, DbErr> {
        let row = entities::enrollment_tokens::Entity::find()
            .filter(entities::enrollment_tokens::Column::TokenHash.eq(hash_token(token)))
            .one(self.store.connection())
            .await?;
        Ok(row.map(stored_enrollment_token))
    }

    pub async fn consume(
        &self,
        token: &str,
        agent_id: Uuid,
        at: DateTime<Utc>,
    ) -> Result<StoredEnrollmentToken, DbErr> {
        let transaction = self.store.connection().begin().await?;
        let model = find_active_enrollment_token(&transaction, token, at).await?;
        consume_enrollment_token(&transaction, model.id, agent_id, at).await?;
        transaction.commit().await?;
        Ok(stored_enrollment_token(model))
    }
}

impl RefreshTokenRepository<'_> {
    pub async fn create(&self, token: NewRefreshToken) -> Result<StoredRefreshToken, DbErr> {
        let model = entities::refresh_tokens::ActiveModel {
            id: Set(token.id),
            user_id: Set(token.user_id),
            token_hash: Set(hash_token(&token.token)),
            status: Set("active".to_string()),
            created_at: Set(token.created_at.into()),
            expires_at: Set(token.expires_at.into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            replaced_by_token_id: Set(None),
        }
        .insert(self.store.connection())
        .await?;
        Ok(stored_refresh_token(model))
    }

    pub async fn find_by_token(&self, token: &str) -> Result<Option<StoredRefreshToken>, DbErr> {
        let row = entities::refresh_tokens::Entity::find()
            .filter(entities::refresh_tokens::Column::TokenHash.eq(hash_token(token)))
            .one(self.store.connection())
            .await?;
        Ok(row.map(stored_refresh_token))
    }

    pub async fn rotate(
        &self,
        old_token_id: Uuid,
        new_token: NewRefreshToken,
        at: DateTime<Utc>,
    ) -> Result<StoredRefreshToken, DbErr> {
        let transaction = self.store.connection().begin().await?;
        let model = entities::refresh_tokens::ActiveModel {
            id: Set(new_token.id),
            user_id: Set(new_token.user_id),
            token_hash: Set(hash_token(&new_token.token)),
            status: Set("active".to_string()),
            created_at: Set(new_token.created_at.into()),
            expires_at: Set(new_token.expires_at.into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            replaced_by_token_id: Set(None),
        }
        .insert(&transaction)
        .await?;
        entities::refresh_tokens::Entity::update_many()
            .col_expr(
                entities::refresh_tokens::Column::Status,
                sea_orm::sea_query::Expr::value("revoked"),
            )
            .col_expr(
                entities::refresh_tokens::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(at),
            )
            .col_expr(
                entities::refresh_tokens::Column::LastUsedAt,
                sea_orm::sea_query::Expr::value(at),
            )
            .col_expr(
                entities::refresh_tokens::Column::ReplacedByTokenId,
                sea_orm::sea_query::Expr::value(new_token.id),
            )
            .filter(entities::refresh_tokens::Column::Id.eq(old_token_id))
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(stored_refresh_token(model))
    }

    pub async fn revoke(&self, token: &str, at: DateTime<Utc>) -> Result<(), DbErr> {
        entities::refresh_tokens::Entity::update_many()
            .col_expr(
                entities::refresh_tokens::Column::Status,
                sea_orm::sea_query::Expr::value("revoked"),
            )
            .col_expr(
                entities::refresh_tokens::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(at),
            )
            .filter(entities::refresh_tokens::Column::TokenHash.eq(hash_token(token)))
            .exec(self.store.connection())
            .await?;
        Ok(())
    }

    pub async fn revoke_all_for_user(&self, user_id: Uuid, at: DateTime<Utc>) -> Result<(), DbErr> {
        entities::refresh_tokens::Entity::update_many()
            .col_expr(
                entities::refresh_tokens::Column::Status,
                sea_orm::sea_query::Expr::value("revoked"),
            )
            .col_expr(
                entities::refresh_tokens::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(at),
            )
            .filter(entities::refresh_tokens::Column::UserId.eq(user_id))
            .filter(entities::refresh_tokens::Column::Status.eq("active"))
            .exec(self.store.connection())
            .await?;
        Ok(())
    }
}

impl AppRepository<'_> {
    pub async fn list(&self) -> Result<Vec<AppSummary>, DbErr> {
        let apps = entities::apps::Entity::find()
            .order_by(entities::apps::Column::Name, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(apps
            .into_iter()
            .map(|app| AppSummary {
                id: app.key,
                name: app.name,
                category: app.category,
                status: app.status,
            })
            .collect())
    }
}

async fn upsert_host<C>(
    connection: &C,
    host_id: Uuid,
    hostname: String,
    system_profile: Value,
    observed_at: DateTime<Utc>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    entities::hosts::Entity::insert(entities::hosts::ActiveModel {
        id: Set(host_id),
        hostname: Set(hostname.clone()),
        display_name: Set(hostname),
        status: Set(serialize_host_status(HostStatus::Online)),
        labels: Set(json!(["agent"])),
        system_profile: Set(system_profile),
        last_seen_at: Set(Some(observed_at.into())),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    })
    .on_conflict(
        sea_orm::sea_query::OnConflict::column(entities::hosts::Column::Id)
            .update_columns([
                entities::hosts::Column::Hostname,
                entities::hosts::Column::Status,
                entities::hosts::Column::SystemProfile,
                entities::hosts::Column::LastSeenAt,
                entities::hosts::Column::UpdatedAt,
            ])
            .to_owned(),
    )
    .exec(connection)
    .await?;
    Ok(())
}

async fn upsert_agent<C>(
    connection: &C,
    agent_id: Uuid,
    host_id: Uuid,
    observed_at: DateTime<Utc>,
    status: &str,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    entities::agents::Entity::insert(entities::agents::ActiveModel {
        id: Set(agent_id),
        host_id: Set(host_id),
        status: Set(status.to_string()),
        version: Set(None),
        protocol_version: Set(Some(doro_protocol::PROTOCOL_VERSION.to_string())),
        last_seen_at: Set(Some(observed_at.into())),
        metadata: Set(json!({})),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    })
    .on_conflict(
        sea_orm::sea_query::OnConflict::column(entities::agents::Column::Id)
            .update_columns([
                entities::agents::Column::HostId,
                entities::agents::Column::Status,
                entities::agents::Column::ProtocolVersion,
                entities::agents::Column::LastSeenAt,
                entities::agents::Column::UpdatedAt,
            ])
            .to_owned(),
    )
    .exec(connection)
    .await?;
    Ok(())
}

async fn ensure_host_exists<C>(connection: &C, host_id: Uuid) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let exists = entities::hosts::Entity::find_by_id(host_id)
        .one(connection)
        .await?
        .is_some();
    if !exists {
        return Err(DbErr::Custom(format!(
            "agent host {host_id} is not enrolled"
        )));
    }
    Ok(())
}

async fn replace_capabilities<C>(
    connection: &C,
    agent_id: Uuid,
    host_id: Uuid,
    capabilities: Vec<AgentCapability>,
    declared_at: DateTime<Utc>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    for capability in capabilities {
        entities::agent_capabilities::Entity::insert(entities::agent_capabilities::ActiveModel {
            id: Set(Uuid::new_v4()),
            agent_id: Set(agent_id),
            host_id: Set(host_id),
            name: Set(serialize_capability_name(capability.name)),
            risk: Set(serialize_capability_risk(capability.risk)),
            description: Set(capability.description),
            declared_at: Set(declared_at.into()),
        })
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns([
                entities::agent_capabilities::Column::AgentId,
                entities::agent_capabilities::Column::Name,
            ])
            .update_columns([
                entities::agent_capabilities::Column::HostId,
                entities::agent_capabilities::Column::Risk,
                entities::agent_capabilities::Column::Description,
                entities::agent_capabilities::Column::DeclaredAt,
            ])
            .to_owned(),
        )
        .exec(connection)
        .await?;
    }
    Ok(())
}

fn database_backend(backend: StoreBackend) -> DatabaseBackend {
    match backend {
        StoreBackend::Postgres => DatabaseBackend::Postgres,
    }
}

fn stored_user(user: entities::users::Model) -> StoredUser {
    StoredUser {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        password_hash: user.password_hash,
        role: user.role,
        status: user.status,
    }
}

fn stored_refresh_token(token: entities::refresh_tokens::Model) -> StoredRefreshToken {
    StoredRefreshToken {
        id: token.id,
        user_id: token.user_id,
        status: token.status,
        expires_at: token.expires_at.into(),
        revoked_at: token.revoked_at.map(Into::into),
    }
}

fn stored_enrollment_token(token: entities::enrollment_tokens::Model) -> StoredEnrollmentToken {
    StoredEnrollmentToken {
        id: token.id,
        label: token.label,
        token_hash: token.token_hash,
        status: token.status,
        expires_at: token.expires_at.map(Into::into),
        used_at: token.used_at.map(Into::into),
        used_by_agent_id: token.used_by_agent_id,
        created_at: token.created_at.into(),
    }
}

async fn find_active_enrollment_token<C>(
    connection: &C,
    token: &str,
    at: DateTime<Utc>,
) -> Result<entities::enrollment_tokens::Model, DbErr>
where
    C: ConnectionTrait,
{
    let Some(model) = entities::enrollment_tokens::Entity::find()
        .filter(entities::enrollment_tokens::Column::TokenHash.eq(hash_token(token)))
        .one(connection)
        .await?
    else {
        return Err(DbErr::Custom("enrollment token is invalid".to_string()));
    };

    if model.status != "active" || model.used_at.is_some() || model.used_by_agent_id.is_some() {
        return Err(DbErr::Custom("enrollment token is not active".to_string()));
    }

    if model
        .expires_at
        .map(DateTime::<Utc>::from)
        .is_some_and(|expires_at| expires_at <= at)
    {
        return Err(DbErr::Custom("enrollment token is expired".to_string()));
    }

    Ok(model)
}

async fn consume_enrollment_token<C>(
    connection: &C,
    token_id: Uuid,
    agent_id: Uuid,
    at: DateTime<Utc>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    entities::enrollment_tokens::Entity::update_many()
        .col_expr(
            entities::enrollment_tokens::Column::Status,
            sea_orm::sea_query::Expr::value("used"),
        )
        .col_expr(
            entities::enrollment_tokens::Column::UsedAt,
            sea_orm::sea_query::Expr::value(at),
        )
        .col_expr(
            entities::enrollment_tokens::Column::UsedByAgentId,
            sea_orm::sea_query::Expr::value(agent_id),
        )
        .filter(entities::enrollment_tokens::Column::Id.eq(token_id))
        .exec(connection)
        .await?;
    Ok(())
}

async fn insert_agent_event<C>(connection: &C, event: NewAgentEvent) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    entities::agent_events::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        host_id: Set(event.host_id),
        agent_id: Set(event.agent_id),
        event_type: Set(event.event_type),
        event_json: Set(event.event_json),
        recorded_at: Set(event.recorded_at.into()),
    }
    .insert(connection)
    .await?;
    Ok(())
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn json_array_strings(value: Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_labels(labels: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for label in labels {
        let trimmed = label.trim();
        if trimmed.is_empty() || normalized.iter().any(|existing| existing == trimmed) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

fn serialize_host_status(status: HostStatus) -> String {
    match status {
        HostStatus::Pending => "pending",
        HostStatus::Online => "online",
        HostStatus::Degraded => "degraded",
        HostStatus::Offline => "offline",
    }
    .to_string()
}

fn parse_host_status(value: &str) -> Option<HostStatus> {
    match value {
        "pending" => Some(HostStatus::Pending),
        "online" => Some(HostStatus::Online),
        "degraded" => Some(HostStatus::Degraded),
        "offline" => Some(HostStatus::Offline),
        _ => None,
    }
}

fn current_host_status(host: &entities::hosts::Model) -> HostStatus {
    let status = parse_host_status(&host.status).unwrap_or(HostStatus::Pending);
    if status != HostStatus::Online {
        return status;
    }

    let Some(last_seen_at) = host.last_seen_at.map(DateTime::<Utc>::from) else {
        return HostStatus::Offline;
    };

    if Utc::now().signed_duration_since(last_seen_at).num_seconds() > HOST_ONLINE_TTL_SECONDS {
        return HostStatus::Offline;
    }

    HostStatus::Online
}

fn serialize_task_status(status: TaskStatus) -> String {
    match status {
        TaskStatus::Draft => "draft",
        TaskStatus::WaitingApproval => "waiting_approval",
        TaskStatus::Queued => "queued",
        TaskStatus::Running => "running",
        TaskStatus::Succeeded => "succeeded",
        TaskStatus::Failed => "failed",
        TaskStatus::Cancelled => "cancelled",
    }
    .to_string()
}

fn parse_task_status(value: &str) -> Option<TaskStatus> {
    match value {
        "draft" => Some(TaskStatus::Draft),
        "waiting_approval" => Some(TaskStatus::WaitingApproval),
        "queued" => Some(TaskStatus::Queued),
        "running" => Some(TaskStatus::Running),
        "succeeded" => Some(TaskStatus::Succeeded),
        "failed" => Some(TaskStatus::Failed),
        "cancelled" => Some(TaskStatus::Cancelled),
        _ => None,
    }
}

fn serialize_approval_status(status: ApprovalStatus) -> String {
    match status {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Denied => "denied",
        ApprovalStatus::Expired => "expired",
    }
    .to_string()
}

fn parse_approval_status(value: &str) -> Option<ApprovalStatus> {
    match value {
        "pending" => Some(ApprovalStatus::Pending),
        "approved" => Some(ApprovalStatus::Approved),
        "denied" => Some(ApprovalStatus::Denied),
        "expired" => Some(ApprovalStatus::Expired),
        _ => None,
    }
}

fn serialize_capability_name(name: CapabilityName) -> String {
    match name {
        CapabilityName::MetricsRead => "metrics_read",
        CapabilityName::LogsRead => "logs_read",
        CapabilityName::ServicesManage => "services_manage",
        CapabilityName::ContainersManage => "containers_manage",
        CapabilityName::FilesRead => "files_read",
        CapabilityName::FilesWrite => "files_write",
        CapabilityName::ShellExecute => "shell_execute",
        CapabilityName::NetworkExpose => "network_expose",
        CapabilityName::DatabaseRestore => "database_restore",
    }
    .to_string()
}

fn parse_capability_name(value: &str) -> Option<CapabilityName> {
    match normalize_enum_token(value).as_str() {
        "metrics_read" => Some(CapabilityName::MetricsRead),
        "logs_read" => Some(CapabilityName::LogsRead),
        "services_manage" => Some(CapabilityName::ServicesManage),
        "containers_manage" => Some(CapabilityName::ContainersManage),
        "files_read" => Some(CapabilityName::FilesRead),
        "files_write" => Some(CapabilityName::FilesWrite),
        "shell_execute" => Some(CapabilityName::ShellExecute),
        "network_expose" => Some(CapabilityName::NetworkExpose),
        "database_restore" => Some(CapabilityName::DatabaseRestore),
        _ => None,
    }
}

fn serialize_capability_risk(risk: CapabilityRisk) -> String {
    match risk {
        CapabilityRisk::Low => "low",
        CapabilityRisk::Medium => "medium",
        CapabilityRisk::High => "high",
    }
    .to_string()
}

fn parse_capability_risk(value: &str) -> Option<CapabilityRisk> {
    match normalize_enum_token(value).as_str() {
        "low" => Some(CapabilityRisk::Low),
        "medium" => Some(CapabilityRisk::Medium),
        "high" => Some(CapabilityRisk::High),
        _ => None,
    }
}

fn normalize_enum_token(value: &str) -> String {
    let mut token = String::new();
    for (index, character) in value.chars().enumerate() {
        if character == '-' || character == ' ' {
            token.push('_');
        } else if character.is_uppercase() {
            if index > 0 {
                token.push('_');
            }
            token.extend(character.to_lowercase());
        } else {
            token.push(character);
        }
    }
    token
}

pub fn parse_uuid(value: &str) -> Result<Uuid, uuid::Error> {
    Uuid::from_str(value)
}

pub fn parse_agent_capability(
    name: &str,
    risk: &str,
    description: String,
) -> Option<AgentCapability> {
    Some(AgentCapability {
        name: parse_capability_name(name)?,
        risk: parse_capability_risk(risk)?,
        description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::MockDatabase;
    use sea_orm::MockExecResult;

    #[tokio::test]
    async fn migrate_executes_versioned_postgres_schema_statements() -> anyhow::Result<()> {
        let exec_count = migrations::split_sql_statements(migrations::SCHEMA_MIGRATIONS.sql).len()
            + migrations::all()
                .iter()
                .map(|migration| migrations::split_sql_statements(migration.sql).len() + 1)
                .sum::<usize>();
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results(
                (0..migrations::all().len()).map(|_| Vec::<entities::settings::Model>::new()),
            )
            .append_exec_results((0..exec_count).map(|_| mock_exec_result()))
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        store.migrate().await?;

        Ok(())
    }

    #[test]
    fn migration_sql_uses_postgres_native_types() {
        let sql = migrations::all()
            .iter()
            .map(|migration| migration.sql)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(sql.contains("UUID PRIMARY KEY"));
        assert!(sql.contains("JSONB NOT NULL"));
        assert!(sql.contains("TIMESTAMPTZ"));
        assert!(sql.contains("PRIMARY KEY (captured_at, id)"));
        assert!(sql.contains("PRIMARY KEY (recorded_at, id)"));
        assert!(sql.contains("idx_metric_snapshots_host_captured_at"));
        assert!(sql.contains("CREATE EXTENSION IF NOT EXISTS timescaledb"));
        assert!(sql.contains("create_hypertable(\n    'metric_snapshots'"));
        assert!(sql.contains("create_hypertable(\n    'agent_events'"));
        assert!(sql.contains("add_retention_policy(\n    'metric_snapshots'"));
        assert!(sql.contains("add_retention_policy(\n    'agent_events'"));
        assert!(sql.contains("INTERVAL '30 days'"));
        assert!(!sql.contains("AUTOINCREMENT"));
        assert!(!sql.contains("sqlite_master"));
    }

    #[test]
    fn parses_proto_debug_capability_names() {
        assert_eq!(
            parse_capability_name("ShellExecute"),
            Some(CapabilityName::ShellExecute)
        );
        assert_eq!(
            parse_capability_name("metrics_read"),
            Some(CapabilityName::MetricsRead)
        );
    }

    #[test]
    fn splits_migration_batches_into_single_statements() {
        let statements =
            migrations::split_sql_statements("CREATE TABLE a (id int);\nCREATE TABLE b (id int);");

        assert_eq!(statements.len(), 2);
        assert!(statements[0].ends_with(';'));
    }

    #[test]
    fn online_host_expires_when_last_seen_is_stale() {
        let mut host = host_model("online", Some(Utc::now().into()));
        assert_eq!(current_host_status(&host), HostStatus::Online);

        host.last_seen_at =
            Some((Utc::now() - chrono::Duration::seconds(HOST_ONLINE_TTL_SECONDS + 1)).into());
        assert_eq!(current_host_status(&host), HostStatus::Offline);
    }

    #[tokio::test]
    async fn deletes_host_and_reports_whether_row_existed() -> anyhow::Result<()> {
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let deleted = store.hosts().delete(Uuid::new_v4()).await?;

        assert!(deleted);
        Ok(())
    }

    #[tokio::test]
    async fn creates_approval_for_matching_task_step() -> anyhow::Result<()> {
        let task_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let approval_id = Uuid::new_v4();
        let requested_at = Utc::now();
        let expires_at = requested_at + chrono::Duration::hours(24);
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[task_step_model(task_id, step_id)]])
            .append_query_results([[approval_model_with_ids(
                approval_id,
                task_id,
                step_id,
                ApprovalStatus::Pending,
                requested_at,
                expires_at,
            )]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let approval = store
            .approvals()
            .create(NewApproval {
                id: approval_id,
                task_id,
                step_id,
                reason: "high risk step".to_string(),
                requested_at,
                expires_at,
            })
            .await?;

        assert_eq!(approval.task_id, task_id);
        assert_eq!(approval.step_id, step_id);
        assert_eq!(approval.status, ApprovalStatus::Pending);
        assert_eq!(approval.expires_at, expires_at);
        Ok(())
    }

    #[tokio::test]
    async fn lists_approvals_after_refreshing_expired_pending_rows() -> anyhow::Result<()> {
        let model = approval_model(
            ApprovalStatus::Expired,
            Utc::now() - chrono::Duration::hours(25),
            Utc::now() - chrono::Duration::hours(1),
        );
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results([mock_exec_result()])
            .append_query_results([[model]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let approvals = store.approvals().list().await?;

        assert_eq!(approvals.len(), 1);
        assert_eq!(approvals[0].status, ApprovalStatus::Expired);
        Ok(())
    }

    #[tokio::test]
    async fn approves_pending_approval() -> anyhow::Result<()> {
        let now = Utc::now();
        let model = approval_model(
            ApprovalStatus::Pending,
            now - chrono::Duration::minutes(5),
            now + chrono::Duration::hours(1),
        );
        let approval_id = model.id;
        let mut resolved_model = model.clone();
        resolved_model.status = serialize_approval_status(ApprovalStatus::Approved);
        resolved_model.resolved_at = Some(now.into());
        resolved_model.resolved_by = Some("admin".to_string());
        resolved_model.decision_note = Some("ok".to_string());
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[model]])
            .append_query_results([[resolved_model]])
            .append_exec_results([mock_exec_result()])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let approval = store
            .approvals()
            .approve(
                approval_id,
                "admin".to_string(),
                Some("ok".to_string()),
                now,
            )
            .await?;

        assert_eq!(approval.status, ApprovalStatus::Approved);
        assert_eq!(approval.resolved_by.as_deref(), Some("admin"));
        assert_eq!(approval.decision_note.as_deref(), Some("ok"));
        Ok(())
    }

    #[tokio::test]
    async fn denies_resolution_for_expired_approval() {
        let now = Utc::now();
        let model = approval_model(
            ApprovalStatus::Expired,
            now - chrono::Duration::hours(25),
            now - chrono::Duration::hours(1),
        );
        let approval_id = model.id;
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results([mock_exec_result()])
            .append_query_results([[model]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let error = match store
            .approvals()
            .deny(approval_id, "admin".to_string(), None, now)
            .await
        {
            Ok(_) => panic!("expired approval should not resolve"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("approval expired"));
    }

    #[tokio::test]
    async fn delete_approval_reports_missing_row() -> anyhow::Result<()> {
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 0,
            }])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let deleted = store.approvals().delete(Uuid::new_v4()).await?;

        assert!(!deleted);
        Ok(())
    }

    #[test]
    fn normalize_labels_trims_and_deduplicates_values() {
        let labels = normalize_labels(vec![
            " agent ".to_string(),
            "".to_string(),
            "infra".to_string(),
            "agent".to_string(),
            " edge ".to_string(),
        ]);

        assert_eq!(labels, vec!["agent", "infra", "edge"]);
    }

    #[test]
    fn enrollment_token_hash_does_not_store_plaintext() {
        let token = "enroll-secret";
        let hash = hash_token(token);

        assert_ne!(hash, token);
        assert_eq!(hash.len(), 64);
    }

    #[tokio::test]
    async fn rejects_used_enrollment_token() {
        let model = enrollment_token_model(
            "active",
            None,
            Some(Utc::now().into()),
            Some(Uuid::new_v4()),
        );
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[model]])
            .into_connection();

        let error = match find_active_enrollment_token(&connection, "token", Utc::now()).await {
            Ok(_) => panic!("used token should be rejected"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("not active"));
    }

    #[tokio::test]
    async fn rejects_expired_enrollment_token() {
        let model = enrollment_token_model(
            "active",
            Some((Utc::now() - chrono::Duration::seconds(1)).into()),
            None,
            None,
        );
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[model]])
            .into_connection();

        let error = match find_active_enrollment_token(&connection, "token", Utc::now()).await {
            Ok(_) => panic!("expired token should be rejected"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("expired"));
    }

    #[tokio::test]
    async fn consumes_active_enrollment_token() -> anyhow::Result<()> {
        let model = enrollment_token_model("active", None, None, None);
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[model]])
            .append_exec_results([mock_exec_result()])
            .into_connection();

        let store = Store::from_connection(connection, DatabaseBackend::Postgres);
        let consumed = store
            .enrollment_tokens()
            .consume("token", Uuid::new_v4(), Utc::now())
            .await?;

        assert_eq!(consumed.status, "active");
        Ok(())
    }

    fn mock_exec_result() -> MockExecResult {
        MockExecResult {
            last_insert_id: 0,
            rows_affected: 0,
        }
    }

    fn host_model(
        status: &str,
        last_seen_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    ) -> entities::hosts::Model {
        entities::hosts::Model {
            id: Uuid::new_v4(),
            hostname: "homelab-node".to_string(),
            display_name: "homelab-node".to_string(),
            status: status.to_string(),
            labels: json!(["agent"]),
            system_profile: json!({}),
            last_seen_at,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    fn enrollment_token_model(
        status: &str,
        expires_at: Option<chrono::DateTime<chrono::FixedOffset>>,
        used_at: Option<chrono::DateTime<chrono::FixedOffset>>,
        used_by_agent_id: Option<Uuid>,
    ) -> entities::enrollment_tokens::Model {
        entities::enrollment_tokens::Model {
            id: Uuid::new_v4(),
            label: "local-agent".to_string(),
            token_hash: hash_token("token"),
            status: status.to_string(),
            expires_at,
            used_at,
            used_by_agent_id,
            created_at: Utc::now().into(),
        }
    }

    fn task_step_model(task_id: Uuid, step_id: Uuid) -> entities::task_steps::Model {
        entities::task_steps::Model {
            id: step_id,
            task_id,
            position: 0,
            capability: "shell_execute".to_string(),
            risk: "high".to_string(),
            summary: "execute command".to_string(),
            payload: json!({}),
            status: "pending".to_string(),
            created_at: Utc::now().into(),
        }
    }

    fn approval_model(
        status: ApprovalStatus,
        requested_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> entities::approvals::Model {
        approval_model_with_ids(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            status,
            requested_at,
            expires_at,
        )
    }

    fn approval_model_with_ids(
        id: Uuid,
        task_id: Uuid,
        step_id: Uuid,
        status: ApprovalStatus,
        requested_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> entities::approvals::Model {
        entities::approvals::Model {
            id,
            task_id,
            step_id,
            reason: "high risk step".to_string(),
            status: serialize_approval_status(status),
            requested_at: requested_at.into(),
            expires_at: expires_at.into(),
            resolved_at: None,
            resolved_by: None,
            decision_note: None,
        }
    }
}
