use chrono::DateTime;
use chrono::Utc;
use doro_config::StoreBackend;
use doro_config::StoreConfig;
use doro_protocol::AgentCapability;
use doro_protocol::AiChatEvent;
use doro_protocol::AiChatEventKind;
use doro_protocol::AiChatMessage;
use doro_protocol::AiChatMessageRole;
use doro_protocol::AiChatMessageStatus;
use doro_protocol::AiConversation;
use doro_protocol::AiModelProvider;
use doro_protocol::AppSummary;
use doro_protocol::ApprovalRequest;
use doro_protocol::ApprovalStatus;
use doro_protocol::CapabilityName;
use doro_protocol::CapabilityRisk;
use doro_protocol::Host;
use doro_protocol::HostContainer;
use doro_protocol::HostStatus;
use doro_protocol::MetricSnapshot;
use doro_protocol::ScheduledTask;
use doro_protocol::ScheduledTaskKind;
use doro_protocol::ScheduledTaskRun;
use doro_protocol::ScheduledTaskRunStatus;
use doro_protocol::ScheduledTaskStatus;
use doro_protocol::Task;
use doro_protocol::TaskStatus;
use doro_protocol::TaskStep;
use doro_protocol::TaskStepStatus;
use doro_protocol::VirtualMachine;
use doro_protocol::VirtualMachineImage;
use doro_protocol::VirtualMachineNetwork;
use doro_protocol::VirtualMachineStatus;
use doro_protocol::VirtualMachineTemplate;
use doro_protocol::Website;
use doro_protocol::WebsiteKind;
use doro_protocol::WebsiteProtocol;
use doro_protocol::WebsiteProxyTarget;
use doro_protocol::WebsiteStatus;
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
    pub metadata: Value,
    pub create_step_approvals: bool,
    pub steps: Vec<TaskStep>,
}

#[derive(Debug, Clone)]
pub struct NewTaskRun {
    pub id: Uuid,
    pub task_id: Uuid,
    pub step_id: Option<Uuid>,
    pub agent_id: Uuid,
    pub status: String,
    pub command_id: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result_json: Value,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewScheduledTask {
    pub id: Uuid,
    pub name: String,
    pub kind: ScheduledTaskKind,
    pub schedule: String,
    pub status: ScheduledTaskStatus,
    pub required_capability: CapabilityName,
    pub label_selector: Vec<String>,
    pub task_template: Value,
    pub next_run_at: Option<DateTime<Utc>>,
    pub approval_task_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct ScheduledTaskChanges {
    pub name: Option<String>,
    pub schedule: Option<String>,
    pub status: Option<ScheduledTaskStatus>,
    pub label_selector: Option<Vec<String>>,
    pub task_template: Option<Value>,
    pub next_run_at: Option<Option<DateTime<Utc>>>,
    pub last_run_at: Option<Option<DateTime<Utc>>>,
    pub last_run_status: Option<Option<ScheduledTaskRunStatus>>,
    pub approval_task_id: Option<Option<Uuid>>,
    pub approved_at: Option<Option<DateTime<Utc>>>,
    pub approved_by: Option<Option<String>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewScheduledTaskRun {
    pub id: Uuid,
    pub scheduled_task_id: Uuid,
    pub task_id: Option<Uuid>,
    pub status: ScheduledTaskRunStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewAiModelProvider {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub default_model: String,
    pub timeout_seconds: u32,
    pub api_key_secret: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct AiModelProviderChanges {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub timeout_seconds: Option<u32>,
    pub api_key_secret: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAiModelProviderSecret {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub default_model: String,
    pub timeout_seconds: u32,
    pub api_key_secret: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct NewAiConversation {
    pub id: Uuid,
    pub title: String,
    pub host_id: Uuid,
    pub ai_provider_id: Uuid,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewAiChatMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: AiChatMessageRole,
    pub status: AiChatMessageStatus,
    pub content: String,
    pub task_id: Option<Uuid>,
    pub host_id: Option<Uuid>,
    pub ai_provider_id: Option<Uuid>,
    pub model: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewAiChatEvent {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub message_id: Uuid,
    pub kind: AiChatEventKind,
    pub content: Option<String>,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct AiChatMessageChanges {
    pub content: Option<String>,
    pub status: Option<AiChatMessageStatus>,
    pub task_id: Option<Option<Uuid>>,
    pub metadata: Option<Value>,
    pub updated_at: Option<DateTime<Utc>>,
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
    pub system_profile: Option<Value>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewAgentEvent {
    pub agent_id: Option<Uuid>,
    pub host_id: Option<Uuid>,
    pub external_event_id: Option<String>,
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
pub struct NewVirtualMachineObservation {
    pub host_id: Uuid,
    pub provider: String,
    pub vm_ref: String,
    pub name: String,
    pub status: VirtualMachineStatus,
    pub image: String,
    pub cpu_cores: u16,
    pub memory_mib: u32,
    pub disk_gb: u32,
    pub networks: Vec<VirtualMachineNetwork>,
    pub console: Option<Value>,
    pub metadata: Value,
    pub created_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewWebsite {
    pub id: Uuid,
    pub host_id: Uuid,
    pub name: String,
    pub primary_domain: String,
    pub aliases: Vec<String>,
    pub status: WebsiteStatus,
    pub kind: WebsiteKind,
    pub protocol: WebsiteProtocol,
    pub listen_port: u16,
    pub upstream_url: String,
    pub app_install_id: Option<Uuid>,
    pub tls_certificate_id: Option<Uuid>,
    pub config: Value,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WebsiteChanges {
    pub host_id: Uuid,
    pub name: String,
    pub primary_domain: String,
    pub aliases: Vec<String>,
    pub listen_port: u16,
    pub upstream_url: String,
    pub notes: Option<String>,
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

    pub fn scheduled_tasks(&self) -> ScheduledTaskRepository<'_> {
        ScheduledTaskRepository { store: self }
    }

    pub fn approvals(&self) -> ApprovalRepository<'_> {
        ApprovalRepository { store: self }
    }

    pub fn ai_model_providers(&self) -> AiModelProviderRepository<'_> {
        AiModelProviderRepository { store: self }
    }

    pub fn ai_chats(&self) -> AiChatRepository<'_> {
        AiChatRepository { store: self }
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

    pub fn virtual_machines(&self) -> VirtualMachineRepository<'_> {
        VirtualMachineRepository { store: self }
    }

    pub fn websites(&self) -> WebsiteRepository<'_> {
        WebsiteRepository { store: self }
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
                external_event_id: None,
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
        let mut host_update = entities::hosts::Entity::update_many()
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
            );
        if let Some(system_profile) = heartbeat.system_profile {
            host_update = host_update.col_expr(
                entities::hosts::Column::SystemProfile,
                sea_orm::sea_query::Expr::value(system_profile),
            );
        }
        host_update
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
            metadata: Set(new_task.metadata.clone()),
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
                status: Set(serialize_task_step_status(step.status)),
                created_at: Set(new_task.created_at.into()),
            }
            .insert(&transaction)
            .await?;

            if new_task.create_step_approvals && step.risk >= CapabilityRisk::High {
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
                    status: parse_task_step_status(&step.status).unwrap_or(TaskStepStatus::Pending),
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

    pub async fn update_status(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        finished_at: Option<DateTime<Utc>>,
        error_message: Option<String>,
    ) -> Result<(), DbErr> {
        let Some(model) = entities::tasks::Entity::find_by_id(task_id)
            .one(self.store.connection())
            .await?
        else {
            return Err(DbErr::RecordNotFound("task not found".to_string()));
        };
        let mut active: entities::tasks::ActiveModel = model.into();
        active.status = Set(serialize_task_status(status));
        if status == TaskStatus::Running {
            active.started_at = Set(Some(Utc::now().into()));
        }
        if let Some(finished_at) = finished_at {
            active.finished_at = Set(Some(finished_at.into()));
        }
        active.error_message = Set(error_message);
        active.update(self.store.connection()).await?;
        Ok(())
    }

    pub async fn update_step_status(&self, step_id: Uuid, status: &str) -> Result<(), DbErr> {
        let Some(model) = entities::task_steps::Entity::find_by_id(step_id)
            .one(self.store.connection())
            .await?
        else {
            return Err(DbErr::RecordNotFound("task step not found".to_string()));
        };
        let mut active: entities::task_steps::ActiveModel = model.into();
        active.status = Set(status.to_string());
        active.update(self.store.connection()).await?;
        Ok(())
    }

    pub async fn update_first_step_status_for_task(
        &self,
        task_id: Uuid,
        status: &str,
    ) -> Result<(), DbErr> {
        let Some(model) = entities::task_steps::Entity::find()
            .filter(entities::task_steps::Column::TaskId.eq(task_id))
            .order_by(entities::task_steps::Column::Position, Order::Asc)
            .one(self.store.connection())
            .await?
        else {
            return Ok(());
        };
        let mut active: entities::task_steps::ActiveModel = model.into();
        active.status = Set(status.to_string());
        active.update(self.store.connection()).await?;
        Ok(())
    }

    pub async fn append_step_with_approval(
        &self,
        task_id: Uuid,
        step: TaskStep,
        reason: String,
        requested_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<ApprovalRequest, DbErr> {
        let transaction = self.store.connection().begin().await?;
        let task = entities::tasks::Entity::find_by_id(task_id)
            .one(&transaction)
            .await?;
        if task.is_none() {
            return Err(DbErr::RecordNotFound("task not found".to_string()));
        }

        let next_position = entities::task_steps::Entity::find()
            .filter(entities::task_steps::Column::TaskId.eq(task_id))
            .order_by(entities::task_steps::Column::Position, Order::Desc)
            .one(&transaction)
            .await?
            .map(|model| model.position + 1)
            .unwrap_or_default();

        entities::task_steps::ActiveModel {
            id: Set(step.id),
            task_id: Set(task_id),
            position: Set(next_position),
            capability: Set(serialize_capability_name(step.capability)),
            risk: Set(serialize_capability_risk(step.risk)),
            summary: Set(step.summary),
            payload: Set(step.payload),
            status: Set(serialize_task_step_status(step.status)),
            created_at: Set(requested_at.into()),
        }
        .insert(&transaction)
        .await?;

        let approval = entities::approvals::ActiveModel {
            id: Set(Uuid::new_v4()),
            task_id: Set(task_id),
            step_id: Set(step.id),
            reason: Set(reason),
            status: Set(serialize_approval_status(ApprovalStatus::Pending)),
            requested_at: Set(requested_at.into()),
            expires_at: Set(expires_at.into()),
            resolved_at: Set(None),
            resolved_by: Set(None),
            decision_note: Set(None),
        }
        .insert(&transaction)
        .await?;

        transaction.commit().await?;
        Ok(approval_model_to_protocol(approval))
    }

    pub async fn create_run(&self, run: NewTaskRun) -> Result<(), DbErr> {
        entities::task_runs::ActiveModel {
            id: Set(run.id),
            task_id: Set(run.task_id),
            step_id: Set(run.step_id),
            agent_id: Set(run.agent_id),
            status: Set(run.status),
            command_id: Set(run.command_id),
            started_at: Set(run.started_at.map(Into::into)),
            finished_at: Set(run.finished_at.map(Into::into)),
            result_json: Set(run.result_json),
            error_message: Set(run.error_message),
        }
        .insert(self.store.connection())
        .await?;
        Ok(())
    }

    pub async fn finish_run(
        &self,
        run_id: Uuid,
        status: String,
        command_id: Option<String>,
        finished_at: DateTime<Utc>,
        result_json: Value,
        error_message: Option<String>,
    ) -> Result<(), DbErr> {
        let Some(model) = entities::task_runs::Entity::find_by_id(run_id)
            .one(self.store.connection())
            .await?
        else {
            return Err(DbErr::RecordNotFound("task run not found".to_string()));
        };
        let mut active: entities::task_runs::ActiveModel = model.into();
        active.status = Set(status);
        active.command_id = Set(command_id);
        active.finished_at = Set(Some(finished_at.into()));
        active.result_json = Set(result_json);
        active.error_message = Set(error_message);
        active.update(self.store.connection()).await?;
        Ok(())
    }

    pub async fn finish_latest_run_for_task(
        &self,
        task_id: Uuid,
        status: String,
        command_id: Option<String>,
        finished_at: DateTime<Utc>,
        result_json: Value,
        error_message: Option<String>,
    ) -> Result<(), DbErr> {
        let Some(model) = entities::task_runs::Entity::find()
            .filter(entities::task_runs::Column::TaskId.eq(task_id))
            .order_by(entities::task_runs::Column::StartedAt, Order::Desc)
            .one(self.store.connection())
            .await?
        else {
            return Ok(());
        };
        let mut active: entities::task_runs::ActiveModel = model.into();
        active.status = Set(status);
        active.command_id = Set(command_id);
        active.finished_at = Set(Some(finished_at.into()));
        active.result_json = Set(result_json);
        active.error_message = Set(error_message);
        active.update(self.store.connection()).await?;
        Ok(())
    }
}

pub struct ScheduledTaskRepository<'a> {
    store: &'a Store,
}

impl ScheduledTaskRepository<'_> {
    pub async fn list(&self) -> Result<Vec<ScheduledTask>, DbErr> {
        let tasks = entities::cron_jobs::Entity::find()
            .order_by(entities::cron_jobs::Column::CreatedAt, Order::Desc)
            .all(self.store.connection())
            .await?;
        Ok(tasks
            .into_iter()
            .filter_map(scheduled_task_model_to_protocol)
            .collect())
    }

    pub async fn due(&self, now: DateTime<Utc>) -> Result<Vec<ScheduledTask>, DbErr> {
        let tasks = entities::cron_jobs::Entity::find()
            .filter(
                entities::cron_jobs::Column::Status
                    .eq(serialize_scheduled_task_status(ScheduledTaskStatus::Active)),
            )
            .filter(entities::cron_jobs::Column::NextRunAt.lte(now))
            .order_by(entities::cron_jobs::Column::NextRunAt, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(tasks
            .into_iter()
            .filter_map(scheduled_task_model_to_protocol)
            .collect())
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<ScheduledTask>, DbErr> {
        Ok(entities::cron_jobs::Entity::find_by_id(id)
            .one(self.store.connection())
            .await?
            .and_then(scheduled_task_model_to_protocol))
    }

    pub async fn find_by_approval_task(
        &self,
        task_id: Uuid,
    ) -> Result<Option<ScheduledTask>, DbErr> {
        Ok(entities::cron_jobs::Entity::find()
            .filter(entities::cron_jobs::Column::ApprovalTaskId.eq(task_id))
            .one(self.store.connection())
            .await?
            .and_then(scheduled_task_model_to_protocol))
    }

    pub async fn create(&self, task: NewScheduledTask) -> Result<ScheduledTask, DbErr> {
        let model = entities::cron_jobs::ActiveModel {
            id: Set(task.id),
            host_id: Set(None),
            name: Set(task.name),
            schedule: Set(task.schedule),
            status: Set(serialize_scheduled_task_status(task.status)),
            task_template: Set(task.task_template),
            kind: Set(serialize_scheduled_task_kind(task.kind)),
            required_capability: Set(serialize_capability_name(task.required_capability)),
            label_selector: Set(json!(task.label_selector)),
            next_run_at: Set(task.next_run_at.map(Into::into)),
            last_run_at: Set(None),
            last_run_status: Set(None),
            approval_task_id: Set(task.approval_task_id),
            approved_at: Set(None),
            approved_by: Set(None),
            created_at: Set(task.created_at.into()),
            updated_at: Set(task.created_at.into()),
        }
        .insert(self.store.connection())
        .await?;
        scheduled_task_model_to_protocol(model)
            .ok_or_else(|| DbErr::Custom("stored scheduled task is invalid".to_string()))
    }

    pub async fn update(
        &self,
        id: Uuid,
        changes: ScheduledTaskChanges,
    ) -> Result<ScheduledTask, DbErr> {
        let Some(model) = entities::cron_jobs::Entity::find_by_id(id)
            .one(self.store.connection())
            .await?
        else {
            return Err(DbErr::RecordNotFound(
                "scheduled task not found".to_string(),
            ));
        };
        let mut active: entities::cron_jobs::ActiveModel = model.into();
        if let Some(name) = changes.name {
            active.name = Set(name);
        }
        if let Some(schedule) = changes.schedule {
            active.schedule = Set(schedule);
        }
        if let Some(status) = changes.status {
            active.status = Set(serialize_scheduled_task_status(status));
        }
        if let Some(label_selector) = changes.label_selector {
            active.label_selector = Set(json!(label_selector));
        }
        if let Some(task_template) = changes.task_template {
            active.task_template = Set(task_template);
        }
        if let Some(next_run_at) = changes.next_run_at {
            active.next_run_at = Set(next_run_at.map(Into::into));
        }
        if let Some(last_run_at) = changes.last_run_at {
            active.last_run_at = Set(last_run_at.map(Into::into));
        }
        if let Some(last_run_status) = changes.last_run_status {
            active.last_run_status = Set(last_run_status.map(serialize_scheduled_task_run_status));
        }
        if let Some(approval_task_id) = changes.approval_task_id {
            active.approval_task_id = Set(approval_task_id);
        }
        if let Some(approved_at) = changes.approved_at {
            active.approved_at = Set(approved_at.map(Into::into));
        }
        if let Some(approved_by) = changes.approved_by {
            active.approved_by = Set(approved_by);
        }
        active.updated_at = Set(changes.updated_at.unwrap_or_else(Utc::now).into());
        let model = active.update(self.store.connection()).await?;
        scheduled_task_model_to_protocol(model)
            .ok_or_else(|| DbErr::Custom("stored scheduled task is invalid".to_string()))
    }

    pub async fn delete(&self, id: Uuid) -> Result<bool, DbErr> {
        let result = entities::cron_jobs::Entity::delete_by_id(id)
            .exec(self.store.connection())
            .await?;
        Ok(result.rows_affected > 0)
    }

    pub async fn create_run(&self, run: NewScheduledTaskRun) -> Result<ScheduledTaskRun, DbErr> {
        let model = entities::cron_job_runs::ActiveModel {
            id: Set(run.id),
            cron_job_id: Set(run.scheduled_task_id),
            task_id: Set(run.task_id),
            status: Set(serialize_scheduled_task_run_status(run.status)),
            started_at: Set(run.started_at.into()),
            finished_at: Set(run.finished_at.map(Into::into)),
            message: Set(run.message),
        }
        .insert(self.store.connection())
        .await?;
        Ok(scheduled_task_run_model_to_protocol(model))
    }

    pub async fn finish_run(
        &self,
        run_id: Uuid,
        status: ScheduledTaskRunStatus,
        finished_at: DateTime<Utc>,
        message: Option<String>,
    ) -> Result<ScheduledTaskRun, DbErr> {
        let Some(model) = entities::cron_job_runs::Entity::find_by_id(run_id)
            .one(self.store.connection())
            .await?
        else {
            return Err(DbErr::RecordNotFound(
                "scheduled task run not found".to_string(),
            ));
        };
        let mut active: entities::cron_job_runs::ActiveModel = model.into();
        active.status = Set(serialize_scheduled_task_run_status(status));
        active.finished_at = Set(Some(finished_at.into()));
        active.message = Set(message);
        let model = active.update(self.store.connection()).await?;
        Ok(scheduled_task_run_model_to_protocol(model))
    }

    pub async fn list_runs(&self, scheduled_task_id: Uuid) -> Result<Vec<ScheduledTaskRun>, DbErr> {
        let runs = entities::cron_job_runs::Entity::find()
            .filter(entities::cron_job_runs::Column::CronJobId.eq(scheduled_task_id))
            .order_by(entities::cron_job_runs::Column::StartedAt, Order::Desc)
            .all(self.store.connection())
            .await?;
        Ok(runs
            .into_iter()
            .map(scheduled_task_run_model_to_protocol)
            .collect())
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

pub struct AiModelProviderRepository<'a> {
    store: &'a Store,
}

impl AiModelProviderRepository<'_> {
    pub async fn list(&self) -> Result<Vec<AiModelProvider>, DbErr> {
        let rows = entities::ai_model_providers::Entity::find()
            .order_by(entities::ai_model_providers::Column::Name, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(ai_provider_model_to_protocol)
            .collect())
    }

    pub async fn get(&self, provider_id: Uuid) -> Result<Option<AiModelProvider>, DbErr> {
        Ok(
            entities::ai_model_providers::Entity::find_by_id(provider_id)
                .one(self.store.connection())
                .await?
                .map(ai_provider_model_to_protocol),
        )
    }

    pub async fn get_secret(
        &self,
        provider_id: Uuid,
    ) -> Result<Option<StoredAiModelProviderSecret>, DbErr> {
        Ok(
            entities::ai_model_providers::Entity::find_by_id(provider_id)
                .one(self.store.connection())
                .await?
                .map(ai_provider_model_to_secret),
        )
    }

    pub async fn create(&self, provider: NewAiModelProvider) -> Result<AiModelProvider, DbErr> {
        let name = required_trimmed(provider.name, "ai provider name is required")?;
        let base_url =
            normalize_required_url(provider.base_url, "ai provider base_url is required")?;
        let default_model = required_trimmed(
            provider.default_model,
            "ai provider default_model is required",
        )?;
        let api_key_secret =
            required_trimmed(provider.api_key_secret, "ai provider api_key is required")?;
        let timeout_seconds = validate_timeout_seconds(provider.timeout_seconds)?;
        if self.name_exists(None, &name).await? {
            return Err(DbErr::Custom("ai provider name already exists".to_string()));
        }

        let model = entities::ai_model_providers::ActiveModel {
            id: Set(provider.id),
            name: Set(name),
            base_url: Set(base_url),
            default_model: Set(default_model),
            timeout_seconds: Set(timeout_seconds),
            api_key_secret: Set(api_key_secret),
            enabled: Set(provider.enabled),
            created_at: Set(provider.created_at.into()),
            updated_at: Set(provider.created_at.into()),
        }
        .insert(self.store.connection())
        .await?;

        Ok(ai_provider_model_to_protocol(model))
    }

    pub async fn update(
        &self,
        provider_id: Uuid,
        changes: AiModelProviderChanges,
    ) -> Result<Option<AiModelProvider>, DbErr> {
        let Some(model) = entities::ai_model_providers::Entity::find_by_id(provider_id)
            .one(self.store.connection())
            .await?
        else {
            return Ok(None);
        };

        let name = match changes.name {
            Some(name) => required_trimmed(name, "ai provider name is required")?,
            None => model.name.clone(),
        };
        let base_url = match changes.base_url {
            Some(base_url) => normalize_required_url(base_url, "ai provider base_url is required")?,
            None => model.base_url.clone(),
        };
        let default_model = match changes.default_model {
            Some(default_model) => {
                required_trimmed(default_model, "ai provider default_model is required")?
            }
            None => model.default_model.clone(),
        };
        let timeout_seconds = match changes.timeout_seconds {
            Some(timeout_seconds) => validate_timeout_seconds(timeout_seconds)?,
            None => model.timeout_seconds,
        };
        let api_key_secret = match changes.api_key_secret {
            Some(api_key_secret) => {
                required_trimmed(api_key_secret, "ai provider api_key is required")?
            }
            None => model.api_key_secret.clone(),
        };
        if self.name_exists(Some(provider_id), &name).await? {
            return Err(DbErr::Custom("ai provider name already exists".to_string()));
        }

        let mut active: entities::ai_model_providers::ActiveModel = model.into();
        active.name = Set(name);
        active.base_url = Set(base_url);
        active.default_model = Set(default_model);
        active.timeout_seconds = Set(timeout_seconds);
        active.api_key_secret = Set(api_key_secret);
        if let Some(enabled) = changes.enabled {
            active.enabled = Set(enabled);
        }
        active.updated_at = Set(Utc::now().into());

        Ok(Some(ai_provider_model_to_protocol(
            active.update(self.store.connection()).await?,
        )))
    }

    pub async fn delete(&self, provider_id: Uuid) -> Result<bool, DbErr> {
        let result = entities::ai_model_providers::Entity::delete_by_id(provider_id)
            .exec(self.store.connection())
            .await?;
        Ok(result.rows_affected > 0)
    }

    async fn name_exists(&self, exclude_id: Option<Uuid>, name: &str) -> Result<bool, DbErr> {
        let rows = entities::ai_model_providers::Entity::find()
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .any(|row| Some(row.id) != exclude_id && row.name.eq_ignore_ascii_case(name)))
    }
}

fn ai_provider_model_to_protocol(provider: entities::ai_model_providers::Model) -> AiModelProvider {
    let has_api_key = !provider.api_key_secret.trim().is_empty();
    AiModelProvider {
        id: provider.id,
        name: provider.name,
        base_url: provider.base_url,
        default_model: provider.default_model,
        timeout_seconds: provider.timeout_seconds.max(0) as u32,
        enabled: provider.enabled,
        has_api_key,
        api_key_hint: if has_api_key {
            Some(api_key_hint(&provider.api_key_secret))
        } else {
            None
        },
        created_at: provider.created_at.into(),
        updated_at: provider.updated_at.into(),
    }
}

fn ai_provider_model_to_secret(
    provider: entities::ai_model_providers::Model,
) -> StoredAiModelProviderSecret {
    StoredAiModelProviderSecret {
        id: provider.id,
        name: provider.name,
        base_url: provider.base_url,
        default_model: provider.default_model,
        timeout_seconds: provider.timeout_seconds.max(0) as u32,
        api_key_secret: provider.api_key_secret,
        enabled: provider.enabled,
    }
}

pub struct AiChatRepository<'a> {
    store: &'a Store,
}

impl AiChatRepository<'_> {
    pub async fn list_conversations(&self) -> Result<Vec<AiConversation>, DbErr> {
        let rows = entities::ai_conversations::Entity::find()
            .order_by(entities::ai_conversations::Column::UpdatedAt, Order::Desc)
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(ai_conversation_model_to_protocol)
            .collect())
    }

    pub async fn get_conversation(
        &self,
        conversation_id: Uuid,
    ) -> Result<Option<AiConversation>, DbErr> {
        Ok(
            entities::ai_conversations::Entity::find_by_id(conversation_id)
                .one(self.store.connection())
                .await?
                .map(ai_conversation_model_to_protocol),
        )
    }

    pub async fn create_conversation(
        &self,
        conversation: NewAiConversation,
    ) -> Result<AiConversation, DbErr> {
        let title = required_trimmed(conversation.title, "ai conversation title is required")?;
        let created_by = required_trimmed(
            conversation.created_by,
            "ai conversation created_by is required",
        )?;
        let model = entities::ai_conversations::ActiveModel {
            id: Set(conversation.id),
            title: Set(title),
            host_id: Set(Some(conversation.host_id)),
            ai_provider_id: Set(Some(conversation.ai_provider_id)),
            created_by: Set(created_by),
            created_at: Set(conversation.created_at.into()),
            updated_at: Set(conversation.created_at.into()),
        }
        .insert(self.store.connection())
        .await?;
        Ok(ai_conversation_model_to_protocol(model))
    }

    pub async fn delete_conversation(&self, conversation_id: Uuid) -> Result<bool, DbErr> {
        let result = entities::ai_conversations::Entity::delete_by_id(conversation_id)
            .exec(self.store.connection())
            .await?;
        Ok(result.rows_affected > 0)
    }

    pub async fn list_messages(&self, conversation_id: Uuid) -> Result<Vec<AiChatMessage>, DbErr> {
        let rows = entities::ai_chat_messages::Entity::find()
            .filter(entities::ai_chat_messages::Column::ConversationId.eq(conversation_id))
            .order_by(entities::ai_chat_messages::Column::CreatedAt, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(ai_chat_message_model_to_protocol)
            .collect())
    }

    pub async fn list_events(&self, conversation_id: Uuid) -> Result<Vec<AiChatEvent>, DbErr> {
        let rows = entities::ai_chat_events::Entity::find()
            .filter(entities::ai_chat_events::Column::ConversationId.eq(conversation_id))
            .order_by(entities::ai_chat_events::Column::CreatedAt, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(ai_chat_event_model_to_protocol)
            .collect())
    }

    pub async fn list_message_events(&self, message_id: Uuid) -> Result<Vec<AiChatEvent>, DbErr> {
        let rows = entities::ai_chat_events::Entity::find()
            .filter(entities::ai_chat_events::Column::MessageId.eq(message_id))
            .order_by(entities::ai_chat_events::Column::CreatedAt, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(ai_chat_event_model_to_protocol)
            .collect())
    }

    pub async fn create_message(&self, message: NewAiChatMessage) -> Result<AiChatMessage, DbErr> {
        let model = entities::ai_chat_messages::ActiveModel {
            id: Set(message.id),
            conversation_id: Set(message.conversation_id),
            role: Set(serialize_ai_chat_message_role(message.role)),
            status: Set(serialize_ai_chat_message_status(message.status)),
            content: Set(message.content),
            task_id: Set(message.task_id),
            host_id: Set(message.host_id),
            ai_provider_id: Set(message.ai_provider_id),
            model: Set(message.model),
            metadata: Set(message.metadata),
            created_at: Set(message.created_at.into()),
            updated_at: Set(message.created_at.into()),
        }
        .insert(self.store.connection())
        .await?;
        self.touch_conversation(message.conversation_id, message.created_at)
            .await?;
        Ok(ai_chat_message_model_to_protocol(model))
    }

    pub async fn update_message(
        &self,
        message_id: Uuid,
        changes: AiChatMessageChanges,
    ) -> Result<Option<AiChatMessage>, DbErr> {
        let Some(model) = entities::ai_chat_messages::Entity::find_by_id(message_id)
            .one(self.store.connection())
            .await?
        else {
            return Ok(None);
        };
        let mut active: entities::ai_chat_messages::ActiveModel = model.into();
        if let Some(content) = changes.content {
            active.content = Set(content);
        }
        if let Some(status) = changes.status {
            active.status = Set(serialize_ai_chat_message_status(status));
        }
        if let Some(task_id) = changes.task_id {
            active.task_id = Set(task_id);
        }
        if let Some(metadata) = changes.metadata {
            active.metadata = Set(metadata);
        }
        let updated_at = changes.updated_at.unwrap_or_else(Utc::now);
        active.updated_at = Set(updated_at.into());
        let model = active.update(self.store.connection()).await?;
        self.touch_conversation(model.conversation_id, updated_at)
            .await?;
        Ok(Some(ai_chat_message_model_to_protocol(model)))
    }

    pub async fn append_message_content(
        &self,
        message_id: Uuid,
        delta: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<AiChatMessage>, DbErr> {
        let Some(model) = entities::ai_chat_messages::Entity::find_by_id(message_id)
            .one(self.store.connection())
            .await?
        else {
            return Ok(None);
        };
        let mut content = model.content.clone();
        let mut active: entities::ai_chat_messages::ActiveModel = model.into();
        content.push_str(delta);
        active.content = Set(content);
        active.updated_at = Set(updated_at.into());
        let model = active.update(self.store.connection()).await?;
        self.touch_conversation(model.conversation_id, updated_at)
            .await?;
        Ok(Some(ai_chat_message_model_to_protocol(model)))
    }

    pub async fn record_event(&self, event: NewAiChatEvent) -> Result<AiChatEvent, DbErr> {
        let model = entities::ai_chat_events::ActiveModel {
            id: Set(event.id),
            conversation_id: Set(event.conversation_id),
            message_id: Set(event.message_id),
            kind: Set(serialize_ai_chat_event_kind(event.kind)),
            content: Set(event.content),
            payload: Set(event.payload),
            created_at: Set(event.created_at.into()),
        }
        .insert(self.store.connection())
        .await?;
        self.touch_conversation(event.conversation_id, event.created_at)
            .await?;
        Ok(ai_chat_event_model_to_protocol(model))
    }

    pub async fn message_for_task(&self, task_id: Uuid) -> Result<Option<AiChatMessage>, DbErr> {
        Ok(entities::ai_chat_messages::Entity::find()
            .filter(entities::ai_chat_messages::Column::TaskId.eq(task_id))
            .filter(entities::ai_chat_messages::Column::Role.eq("assistant"))
            .one(self.store.connection())
            .await?
            .map(ai_chat_message_model_to_protocol))
    }

    async fn touch_conversation(
        &self,
        conversation_id: Uuid,
        updated_at: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        let Some(model) = entities::ai_conversations::Entity::find_by_id(conversation_id)
            .one(self.store.connection())
            .await?
        else {
            return Ok(());
        };
        let mut active: entities::ai_conversations::ActiveModel = model.into();
        active.updated_at = Set(updated_at.into());
        active.update(self.store.connection()).await?;
        Ok(())
    }
}

fn ai_conversation_model_to_protocol(
    conversation: entities::ai_conversations::Model,
) -> AiConversation {
    AiConversation {
        id: conversation.id,
        title: conversation.title,
        host_id: conversation.host_id,
        ai_provider_id: conversation.ai_provider_id,
        created_by: conversation.created_by,
        created_at: conversation.created_at.into(),
        updated_at: conversation.updated_at.into(),
    }
}

fn ai_chat_message_model_to_protocol(message: entities::ai_chat_messages::Model) -> AiChatMessage {
    AiChatMessage {
        id: message.id,
        conversation_id: message.conversation_id,
        role: parse_ai_chat_message_role(&message.role),
        status: parse_ai_chat_message_status(&message.status),
        content: message.content,
        task_id: message.task_id,
        host_id: message.host_id,
        ai_provider_id: message.ai_provider_id,
        model: message.model,
        metadata: message.metadata,
        created_at: message.created_at.into(),
        updated_at: message.updated_at.into(),
    }
}

fn ai_chat_event_model_to_protocol(event: entities::ai_chat_events::Model) -> AiChatEvent {
    AiChatEvent {
        id: event.id,
        conversation_id: event.conversation_id,
        message_id: event.message_id,
        kind: parse_ai_chat_event_kind(&event.kind),
        content: event.content,
        payload: event.payload,
        created_at: event.created_at.into(),
    }
}

fn serialize_ai_chat_message_role(role: AiChatMessageRole) -> String {
    match role {
        AiChatMessageRole::User => "user",
        AiChatMessageRole::Assistant => "assistant",
        AiChatMessageRole::Tool => "tool",
    }
    .to_string()
}

fn parse_ai_chat_message_role(role: &str) -> AiChatMessageRole {
    match role {
        "assistant" => AiChatMessageRole::Assistant,
        "tool" => AiChatMessageRole::Tool,
        _ => AiChatMessageRole::User,
    }
}

fn serialize_ai_chat_message_status(status: AiChatMessageStatus) -> String {
    match status {
        AiChatMessageStatus::Pending => "pending",
        AiChatMessageStatus::Running => "running",
        AiChatMessageStatus::WaitingApproval => "waiting_approval",
        AiChatMessageStatus::Succeeded => "succeeded",
        AiChatMessageStatus::Failed => "failed",
    }
    .to_string()
}

fn parse_ai_chat_message_status(status: &str) -> AiChatMessageStatus {
    match status {
        "running" => AiChatMessageStatus::Running,
        "waiting_approval" => AiChatMessageStatus::WaitingApproval,
        "succeeded" => AiChatMessageStatus::Succeeded,
        "failed" => AiChatMessageStatus::Failed,
        _ => AiChatMessageStatus::Pending,
    }
}

fn serialize_ai_chat_event_kind(kind: AiChatEventKind) -> String {
    match kind {
        AiChatEventKind::TextDelta => "text_delta",
        AiChatEventKind::ToolCall => "tool_call",
        AiChatEventKind::ApprovalRequired => "approval_required",
        AiChatEventKind::ToolResult => "tool_result",
        AiChatEventKind::Done => "done",
        AiChatEventKind::Error => "error",
    }
    .to_string()
}

fn parse_ai_chat_event_kind(kind: &str) -> AiChatEventKind {
    match kind {
        "tool_call" => AiChatEventKind::ToolCall,
        "approval_required" => AiChatEventKind::ApprovalRequired,
        "tool_result" => AiChatEventKind::ToolResult,
        "done" => AiChatEventKind::Done,
        "error" => AiChatEventKind::Error,
        _ => AiChatEventKind::TextDelta,
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

fn scheduled_task_model_to_protocol(task: entities::cron_jobs::Model) -> Option<ScheduledTask> {
    Some(ScheduledTask {
        id: task.id,
        name: task.name,
        kind: parse_scheduled_task_kind(&task.kind)?,
        schedule: task.schedule,
        status: parse_scheduled_task_status(&task.status)?,
        required_capability: parse_capability_name(&task.required_capability)?,
        label_selector: json_array_strings(task.label_selector),
        task_template: task.task_template,
        next_run_at: task.next_run_at.map(Into::into),
        last_run_at: task.last_run_at.map(Into::into),
        last_run_status: task
            .last_run_status
            .as_deref()
            .and_then(parse_scheduled_task_run_status),
        approval_task_id: task.approval_task_id,
        approved_at: task.approved_at.map(Into::into),
        approved_by: task.approved_by,
        created_at: task.created_at.into(),
        updated_at: task.updated_at.into(),
    })
}

fn scheduled_task_run_model_to_protocol(run: entities::cron_job_runs::Model) -> ScheduledTaskRun {
    ScheduledTaskRun {
        id: run.id,
        scheduled_task_id: run.cron_job_id,
        task_id: run.task_id,
        status: parse_scheduled_task_run_status(&run.status)
            .unwrap_or(ScheduledTaskRunStatus::Failed),
        started_at: run.started_at.into(),
        finished_at: run.finished_at.map(Into::into),
        message: run.message,
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

    pub async fn list(&self) -> Result<Vec<HostContainer>, DbErr> {
        let rows = entities::containers::Entity::find()
            .order_by(entities::containers::Column::Name, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows.into_iter().map(container_model_to_protocol).collect())
    }

    pub async fn list_by_host(&self, host_id: Uuid) -> Result<Vec<HostContainer>, DbErr> {
        let rows = entities::containers::Entity::find()
            .filter(entities::containers::Column::HostId.eq(host_id))
            .order_by(entities::containers::Column::Name, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows.into_iter().map(container_model_to_protocol).collect())
    }
}

fn container_model_to_protocol(container: entities::containers::Model) -> HostContainer {
    HostContainer {
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
    }
}

pub struct WebsiteRepository<'a> {
    store: &'a Store,
}

impl WebsiteRepository<'_> {
    pub async fn list(&self) -> Result<Vec<Website>, DbErr> {
        let rows = entities::websites::Entity::find()
            .order_by(entities::websites::Column::PrimaryDomain, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows.into_iter().map(website_model_to_protocol).collect())
    }

    pub async fn running(&self) -> Result<Vec<Website>, DbErr> {
        let rows = entities::websites::Entity::find()
            .filter(entities::websites::Column::Status.eq("running"))
            .order_by(entities::websites::Column::PrimaryDomain, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows.into_iter().map(website_model_to_protocol).collect())
    }

    pub async fn running_by_host(&self, host_id: Uuid) -> Result<Vec<Website>, DbErr> {
        let rows = entities::websites::Entity::find()
            .filter(entities::websites::Column::HostId.eq(host_id))
            .filter(entities::websites::Column::Status.eq("running"))
            .order_by(entities::websites::Column::PrimaryDomain, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows.into_iter().map(website_model_to_protocol).collect())
    }

    pub async fn get(&self, website_id: Uuid) -> Result<Option<Website>, DbErr> {
        Ok(entities::websites::Entity::find_by_id(website_id)
            .one(self.store.connection())
            .await?
            .map(website_model_to_protocol))
    }

    pub async fn create(&self, website: NewWebsite) -> Result<Website, DbErr> {
        if self
            .domain_listen_exists(
                None,
                website.host_id,
                &website.primary_domain,
                website.listen_port,
            )
            .await?
        {
            return Err(DbErr::Custom(
                "website domain already exists for listen port".to_string(),
            ));
        }

        let model = entities::websites::ActiveModel {
            id: Set(website.id),
            host_id: Set(Some(website.host_id)),
            name: Set(website.name),
            primary_domain: Set(website.primary_domain),
            aliases: Set(serde_json::to_value(website.aliases).unwrap_or_else(|_| json!([]))),
            status: Set(serialize_website_status(website.status)),
            kind: Set(serialize_website_kind(website.kind)),
            protocol: Set(serialize_website_protocol(website.protocol)),
            listen_port: Set(i32::from(website.listen_port)),
            upstream_url: Set(website.upstream_url),
            app_install_id: Set(website.app_install_id),
            tls_certificate_id: Set(website.tls_certificate_id),
            config: Set(website.config),
            notes: Set(website.notes),
            last_runtime_error: Set(None),
            last_checked_at: Set(None),
            created_at: Set(website.created_at.into()),
            updated_at: Set(website.created_at.into()),
        }
        .insert(self.store.connection())
        .await?;

        Ok(website_model_to_protocol(model))
    }

    pub async fn update_stopped(
        &self,
        website_id: Uuid,
        changes: WebsiteChanges,
    ) -> Result<Option<Website>, DbErr> {
        let Some(model) = entities::websites::Entity::find_by_id(website_id)
            .one(self.store.connection())
            .await?
        else {
            return Ok(None);
        };
        if model.host_id.is_none() {
            return Err(DbErr::Custom(
                "website must be bound to a host before configuration changes".to_string(),
            ));
        }
        if parse_website_status(&model.status) != Some(WebsiteStatus::Stopped) {
            return Err(DbErr::Custom(
                "website must be stopped before configuration changes".to_string(),
            ));
        }
        if self
            .domain_listen_exists(
                Some(website_id),
                changes.host_id,
                &changes.primary_domain,
                changes.listen_port,
            )
            .await?
        {
            return Err(DbErr::Custom(
                "website domain already exists for listen port".to_string(),
            ));
        }

        let mut active: entities::websites::ActiveModel = model.into();
        active.host_id = Set(Some(changes.host_id));
        active.name = Set(changes.name);
        active.primary_domain = Set(changes.primary_domain);
        active.aliases = Set(serde_json::to_value(changes.aliases).unwrap_or_else(|_| json!([])));
        active.listen_port = Set(i32::from(changes.listen_port));
        active.upstream_url = Set(changes.upstream_url);
        active.notes = Set(changes.notes);
        active.last_runtime_error = Set(None);
        active.updated_at = Set(Utc::now().into());
        Ok(Some(website_model_to_protocol(
            active.update(self.store.connection()).await?,
        )))
    }

    pub async fn set_status(
        &self,
        website_id: Uuid,
        status: WebsiteStatus,
        runtime_error: Option<String>,
    ) -> Result<Option<Website>, DbErr> {
        let Some(model) = entities::websites::Entity::find_by_id(website_id)
            .one(self.store.connection())
            .await?
        else {
            return Ok(None);
        };
        let now = Utc::now();
        let mut active: entities::websites::ActiveModel = model.into();
        active.status = Set(serialize_website_status(status));
        active.last_runtime_error = Set(runtime_error);
        active.last_checked_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        Ok(Some(website_model_to_protocol(
            active.update(self.store.connection()).await?,
        )))
    }

    pub async fn delete(&self, website_id: Uuid) -> Result<bool, DbErr> {
        let result = entities::websites::Entity::delete_by_id(website_id)
            .exec(self.store.connection())
            .await?;
        Ok(result.rows_affected > 0)
    }

    async fn domain_listen_exists(
        &self,
        exclude_id: Option<Uuid>,
        host_id: Uuid,
        primary_domain: &str,
        listen_port: u16,
    ) -> Result<bool, DbErr> {
        let rows = entities::websites::Entity::find()
            .filter(entities::websites::Column::HostId.eq(host_id))
            .filter(entities::websites::Column::ListenPort.eq(i32::from(listen_port)))
            .all(self.store.connection())
            .await?;
        Ok(rows.into_iter().any(|row| {
            row.host_id == Some(host_id)
                && Some(row.id) != exclude_id
                && row.primary_domain.eq_ignore_ascii_case(primary_domain)
        }))
    }
}

fn website_model_to_protocol(website: entities::websites::Model) -> Website {
    Website {
        id: website.id,
        host_id: website.host_id,
        name: website.name,
        primary_domain: website.primary_domain,
        aliases: serde_json::from_value(website.aliases).unwrap_or_default(),
        status: parse_website_status(&website.status).unwrap_or(WebsiteStatus::Warning),
        kind: parse_website_kind(&website.kind).unwrap_or(WebsiteKind::ReverseProxy),
        protocol: parse_website_protocol(&website.protocol).unwrap_or(WebsiteProtocol::Http),
        listen_port: website.listen_port.max(0).min(u16::MAX as i32) as u16,
        upstream: WebsiteProxyTarget {
            url: website.upstream_url,
        },
        app_install_id: website.app_install_id,
        tls_certificate_id: website.tls_certificate_id,
        config: website.config,
        notes: website.notes,
        last_runtime_error: website.last_runtime_error,
        last_checked_at: website.last_checked_at.map(Into::into),
        created_at: website.created_at.into(),
        updated_at: website.updated_at.into(),
    }
}

pub struct VirtualMachineRepository<'a> {
    store: &'a Store,
}

impl VirtualMachineRepository<'_> {
    pub async fn upsert_many(
        &self,
        virtual_machines: Vec<NewVirtualMachineObservation>,
    ) -> Result<(), DbErr> {
        for vm in virtual_machines {
            entities::virtual_machines::Entity::insert(entities::virtual_machines::ActiveModel {
                id: Set(Uuid::new_v4()),
                host_id: Set(vm.host_id),
                provider: Set(vm.provider),
                vm_ref: Set(vm.vm_ref),
                name: Set(vm.name),
                status: Set(serialize_virtual_machine_status(vm.status)),
                image: Set(vm.image),
                cpu_cores: Set(i32::from(vm.cpu_cores)),
                memory_mib: Set(vm.memory_mib.min(i32::MAX as u32) as i32),
                disk_gb: Set(vm.disk_gb.min(i32::MAX as u32) as i32),
                networks: Set(serde_json::to_value(vm.networks).unwrap_or_else(|_| json!([]))),
                console: Set(vm.console),
                metadata: Set(vm.metadata),
                created_at: Set(vm.created_at.map(Into::into)),
                observed_at: Set(vm.observed_at.into()),
            })
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    entities::virtual_machines::Column::HostId,
                    entities::virtual_machines::Column::Provider,
                    entities::virtual_machines::Column::VmRef,
                ])
                .update_columns([
                    entities::virtual_machines::Column::Name,
                    entities::virtual_machines::Column::Status,
                    entities::virtual_machines::Column::Image,
                    entities::virtual_machines::Column::CpuCores,
                    entities::virtual_machines::Column::MemoryMib,
                    entities::virtual_machines::Column::DiskGb,
                    entities::virtual_machines::Column::Networks,
                    entities::virtual_machines::Column::Console,
                    entities::virtual_machines::Column::Metadata,
                    entities::virtual_machines::Column::CreatedAt,
                    entities::virtual_machines::Column::ObservedAt,
                ])
                .to_owned(),
            )
            .exec(self.store.connection())
            .await?;
        }
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<VirtualMachine>, DbErr> {
        let rows = entities::virtual_machines::Entity::find()
            .order_by(entities::virtual_machines::Column::Name, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(virtual_machine_model_to_protocol)
            .collect())
    }

    pub async fn list_by_host(&self, host_id: Uuid) -> Result<Vec<VirtualMachine>, DbErr> {
        let rows = entities::virtual_machines::Entity::find()
            .filter(entities::virtual_machines::Column::HostId.eq(host_id))
            .order_by(entities::virtual_machines::Column::Name, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(virtual_machine_model_to_protocol)
            .collect())
    }

    pub async fn images(&self) -> Result<Vec<VirtualMachineImage>, DbErr> {
        let rows = entities::virtual_machine_images::Entity::find()
            .order_by(entities::virtual_machine_images::Column::Name, Order::Asc)
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(|image| VirtualMachineImage {
                host_id: image.host_id,
                id: image.image_ref,
                name: image.name,
                path: image.path,
                os_family: image.os_family,
                architecture: image.architecture,
            })
            .collect())
    }

    pub async fn templates(&self) -> Result<Vec<VirtualMachineTemplate>, DbErr> {
        let rows = entities::virtual_machine_templates::Entity::find()
            .order_by(
                entities::virtual_machine_templates::Column::Name,
                Order::Asc,
            )
            .all(self.store.connection())
            .await?;
        Ok(rows
            .into_iter()
            .map(|template| VirtualMachineTemplate {
                id: template.template_ref,
                name: template.name,
                image_id: template.image_ref,
                cpu_cores: template.cpu_cores.max(0).min(u16::MAX as i32) as u16,
                memory_mib: template.memory_mib.max(0) as u32,
                disk_gb: template.disk_gb.max(0) as u32,
                description: template.description,
            })
            .collect())
    }
}

fn virtual_machine_model_to_protocol(vm: entities::virtual_machines::Model) -> VirtualMachine {
    VirtualMachine {
        id: vm.id,
        host_id: vm.host_id,
        vm_ref: vm.vm_ref,
        name: vm.name,
        status: parse_virtual_machine_status(&vm.status).unwrap_or(VirtualMachineStatus::Unknown),
        provider: vm.provider,
        image: vm.image,
        cpu_cores: vm.cpu_cores.max(0).min(u16::MAX as i32) as u16,
        memory_mib: vm.memory_mib.max(0) as u32,
        disk_gb: vm.disk_gb.max(0) as u32,
        networks: serde_json::from_value(vm.networks).unwrap_or_default(),
        console: vm.console,
        metadata: vm.metadata,
        created_at: vm.created_at.map(Into::into),
        observed_at: vm.observed_at.into(),
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

    pub async fn update_display_name(
        &self,
        id: Uuid,
        display_name: String,
        at: DateTime<Utc>,
    ) -> Result<Option<StoredUser>, DbErr> {
        let Some(model) = entities::users::Entity::find_by_id(id)
            .one(self.store.connection())
            .await?
        else {
            return Ok(None);
        };

        let mut active: entities::users::ActiveModel = model.into();
        active.display_name = Set(display_name);
        active.updated_at = Set(at.into());
        let model = active.update(self.store.connection()).await?;
        Ok(Some(stored_user(model)))
    }

    pub async fn update_password_hash_and_revoke_refresh_tokens(
        &self,
        id: Uuid,
        password_hash: String,
        at: DateTime<Utc>,
    ) -> Result<Option<StoredUser>, DbErr> {
        let transaction = self.store.connection().begin().await?;
        let Some(model) = entities::users::Entity::find_by_id(id)
            .one(&transaction)
            .await?
        else {
            return Ok(None);
        };

        let mut active: entities::users::ActiveModel = model.into();
        active.password_hash = Set(password_hash);
        active.updated_at = Set(at.into());
        let model = active.update(&transaction).await?;
        entities::refresh_tokens::Entity::update_many()
            .col_expr(
                entities::refresh_tokens::Column::Status,
                sea_orm::sea_query::Expr::value("revoked"),
            )
            .col_expr(
                entities::refresh_tokens::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(at),
            )
            .filter(entities::refresh_tokens::Column::UserId.eq(id))
            .filter(entities::refresh_tokens::Column::Status.eq("active"))
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(Some(stored_user(model)))
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
    if let Some(external_event_id) = event.external_event_id.as_deref() {
        let duplicate = entities::agent_events::Entity::find()
            .filter(entities::agent_events::Column::ExternalEventId.eq(external_event_id))
            .filter(entities::agent_events::Column::AgentId.eq(event.agent_id))
            .filter(entities::agent_events::Column::HostId.eq(event.host_id))
            .one(connection)
            .await?;
        if duplicate.is_some() {
            return Ok(());
        }
    }

    entities::agent_events::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        host_id: Set(event.host_id),
        agent_id: Set(event.agent_id),
        external_event_id: Set(event.external_event_id),
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

fn required_trimmed(value: String, message: &str) -> Result<String, DbErr> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DbErr::Custom(message.to_string()));
    }
    Ok(trimmed.to_string())
}

fn normalize_required_url(value: String, message: &str) -> Result<String, DbErr> {
    let url = required_trimmed(value, message)?;
    Ok(url.trim_end_matches('/').to_string())
}

fn validate_timeout_seconds(value: u32) -> Result<i32, DbErr> {
    if value == 0 {
        return Err(DbErr::Custom(
            "ai provider timeout_seconds must be greater than zero".to_string(),
        ));
    }
    Ok(value.min(i32::MAX as u32) as i32)
}

fn api_key_hint(secret: &str) -> String {
    let secret = secret.trim();
    let suffix = secret
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    if suffix.chars().count() < 4 {
        "****".to_string()
    } else {
        format!("...{suffix}")
    }
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

fn serialize_task_step_status(status: TaskStepStatus) -> String {
    match status {
        TaskStepStatus::Pending => "pending",
        TaskStepStatus::WaitingApproval => "waiting_approval",
        TaskStepStatus::Running => "running",
        TaskStepStatus::Succeeded => "succeeded",
        TaskStepStatus::Failed => "failed",
        TaskStepStatus::Cancelled => "cancelled",
    }
    .to_string()
}

fn parse_task_step_status(value: &str) -> Option<TaskStepStatus> {
    match value {
        "pending" => Some(TaskStepStatus::Pending),
        "waiting_approval" => Some(TaskStepStatus::WaitingApproval),
        "running" => Some(TaskStepStatus::Running),
        "succeeded" => Some(TaskStepStatus::Succeeded),
        "failed" => Some(TaskStepStatus::Failed),
        "cancelled" => Some(TaskStepStatus::Cancelled),
        _ => None,
    }
}

fn serialize_scheduled_task_kind(kind: ScheduledTaskKind) -> String {
    match kind {
        ScheduledTaskKind::Script => "script",
        ScheduledTaskKind::AgentRun => "agent_run",
    }
    .to_string()
}

fn parse_scheduled_task_kind(value: &str) -> Option<ScheduledTaskKind> {
    match value {
        "script" => Some(ScheduledTaskKind::Script),
        "agent_run" => Some(ScheduledTaskKind::AgentRun),
        _ => None,
    }
}

fn serialize_scheduled_task_status(status: ScheduledTaskStatus) -> String {
    match status {
        ScheduledTaskStatus::PendingApproval => "pending_approval",
        ScheduledTaskStatus::Active => "active",
        ScheduledTaskStatus::Paused => "paused",
    }
    .to_string()
}

fn parse_scheduled_task_status(value: &str) -> Option<ScheduledTaskStatus> {
    match value {
        "pending_approval" => Some(ScheduledTaskStatus::PendingApproval),
        "active" => Some(ScheduledTaskStatus::Active),
        "paused" => Some(ScheduledTaskStatus::Paused),
        _ => None,
    }
}

fn serialize_scheduled_task_run_status(status: ScheduledTaskRunStatus) -> String {
    match status {
        ScheduledTaskRunStatus::Running => "running",
        ScheduledTaskRunStatus::Succeeded => "succeeded",
        ScheduledTaskRunStatus::Failed => "failed",
        ScheduledTaskRunStatus::Skipped => "skipped",
    }
    .to_string()
}

fn parse_scheduled_task_run_status(value: &str) -> Option<ScheduledTaskRunStatus> {
    match value {
        "running" => Some(ScheduledTaskRunStatus::Running),
        "succeeded" => Some(ScheduledTaskRunStatus::Succeeded),
        "failed" => Some(ScheduledTaskRunStatus::Failed),
        "skipped" => Some(ScheduledTaskRunStatus::Skipped),
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

fn serialize_website_status(status: WebsiteStatus) -> String {
    match status {
        WebsiteStatus::Stopped => "stopped",
        WebsiteStatus::Running => "running",
        WebsiteStatus::Warning => "warning",
    }
    .to_string()
}

fn parse_website_status(value: &str) -> Option<WebsiteStatus> {
    match normalize_enum_token(value).as_str() {
        "stopped" => Some(WebsiteStatus::Stopped),
        "running" => Some(WebsiteStatus::Running),
        "warning" => Some(WebsiteStatus::Warning),
        _ => None,
    }
}

fn serialize_website_kind(kind: WebsiteKind) -> String {
    match kind {
        WebsiteKind::ReverseProxy => "reverse_proxy",
        WebsiteKind::StaticSite => "static_site",
        WebsiteKind::TcpProxy => "tcp_proxy",
        WebsiteKind::UdpProxy => "udp_proxy",
    }
    .to_string()
}

fn parse_website_kind(value: &str) -> Option<WebsiteKind> {
    match normalize_enum_token(value).as_str() {
        "reverse_proxy" => Some(WebsiteKind::ReverseProxy),
        "static_site" => Some(WebsiteKind::StaticSite),
        "tcp_proxy" => Some(WebsiteKind::TcpProxy),
        "udp_proxy" => Some(WebsiteKind::UdpProxy),
        _ => None,
    }
}

fn serialize_website_protocol(protocol: WebsiteProtocol) -> String {
    match protocol {
        WebsiteProtocol::Http => "http",
        WebsiteProtocol::Https => "https",
        WebsiteProtocol::Tcp => "tcp",
        WebsiteProtocol::Udp => "udp",
    }
    .to_string()
}

fn parse_website_protocol(value: &str) -> Option<WebsiteProtocol> {
    match normalize_enum_token(value).as_str() {
        "http" => Some(WebsiteProtocol::Http),
        "https" => Some(WebsiteProtocol::Https),
        "tcp" => Some(WebsiteProtocol::Tcp),
        "udp" => Some(WebsiteProtocol::Udp),
        _ => None,
    }
}

fn serialize_capability_name(name: CapabilityName) -> String {
    match name {
        CapabilityName::MetricsRead => "metrics_read",
        CapabilityName::LogsRead => "logs_read",
        CapabilityName::AgentRun => "agent_run",
        CapabilityName::ServicesManage => "services_manage",
        CapabilityName::ContainersManage => "containers_manage",
        CapabilityName::VirtualMachinesManage => "virtual_machines_manage",
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
        "agent_run" => Some(CapabilityName::AgentRun),
        "services_manage" => Some(CapabilityName::ServicesManage),
        "containers_manage" => Some(CapabilityName::ContainersManage),
        "virtual_machines_manage" => Some(CapabilityName::VirtualMachinesManage),
        "files_read" => Some(CapabilityName::FilesRead),
        "files_write" => Some(CapabilityName::FilesWrite),
        "shell_execute" => Some(CapabilityName::ShellExecute),
        "network_expose" => Some(CapabilityName::NetworkExpose),
        "database_restore" => Some(CapabilityName::DatabaseRestore),
        _ => None,
    }
}

fn serialize_virtual_machine_status(status: VirtualMachineStatus) -> String {
    match status {
        VirtualMachineStatus::Unknown => "unknown",
        VirtualMachineStatus::Stopped => "stopped",
        VirtualMachineStatus::Starting => "starting",
        VirtualMachineStatus::Running => "running",
        VirtualMachineStatus::Paused => "paused",
        VirtualMachineStatus::Stopping => "stopping",
        VirtualMachineStatus::Failed => "failed",
    }
    .to_string()
}

fn parse_virtual_machine_status(value: &str) -> Option<VirtualMachineStatus> {
    match value {
        "unknown" => Some(VirtualMachineStatus::Unknown),
        "stopped" => Some(VirtualMachineStatus::Stopped),
        "starting" => Some(VirtualMachineStatus::Starting),
        "running" => Some(VirtualMachineStatus::Running),
        "paused" => Some(VirtualMachineStatus::Paused),
        "stopping" => Some(VirtualMachineStatus::Stopping),
        "failed" => Some(VirtualMachineStatus::Failed),
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
        assert!(sql.contains("ALTER COLUMN host_id DROP NOT NULL"));
        assert!(sql.contains("idx_websites_primary_domain_listen_port"));
        assert!(sql.contains("external_event_id TEXT"));
        assert!(
            sql.contains("CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_events_external_event_id")
        );
        assert!(sql.contains("ON agent_events(recorded_at, agent_id, host_id, external_event_id)"));
        assert!(!sql.contains("AUTOINCREMENT"));
        assert!(!sql.contains("sqlite_master"));
    }

    #[tokio::test]
    async fn agent_event_record_is_idempotent_for_replayed_external_event_id() -> anyhow::Result<()>
    {
        let agent_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let recorded_at = Utc::now();
        let duplicate = entities::agent_events::Model {
            recorded_at: recorded_at.into(),
            id: 1,
            host_id: Some(host_id),
            agent_id: Some(agent_id),
            external_event_id: Some("event-1".to_string()),
            event_type: "heartbeat".to_string(),
            event_json: json!({}),
        };
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[duplicate]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        store
            .events()
            .record(NewAgentEvent {
                agent_id: Some(agent_id),
                host_id: Some(host_id),
                external_event_id: Some("event-1".to_string()),
                event_type: "heartbeat".to_string(),
                event_json: json!({}),
                recorded_at,
            })
            .await?;

        Ok(())
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

    #[tokio::test]
    async fn creates_ai_model_provider_without_exposing_secret() -> anyhow::Result<()> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let inserted = ai_model_provider_model(id, "OpenAI", "sk-secret-value", true);
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<entities::ai_model_providers::Model>::new()])
            .append_query_results([[inserted]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let provider = store
            .ai_model_providers()
            .create(NewAiModelProvider {
                id,
                name: " OpenAI ".to_string(),
                base_url: "https://api.openai.com/v1/".to_string(),
                default_model: "gpt-4.1-mini".to_string(),
                timeout_seconds: 60,
                api_key_secret: "sk-secret-value".to_string(),
                enabled: true,
                created_at: now,
            })
            .await?;

        assert_eq!(provider.id, id);
        assert_eq!(provider.name, "OpenAI");
        assert_eq!(provider.base_url, "https://api.openai.com/v1");
        assert!(provider.has_api_key);
        assert_eq!(provider.api_key_hint.as_deref(), Some("...alue"));
        assert_ne!(provider.api_key_hint.as_deref(), Some("sk-secret-value"));
        Ok(())
    }

    #[tokio::test]
    async fn rejects_duplicate_ai_model_provider_names_case_insensitively() {
        let existing = ai_model_provider_model(Uuid::new_v4(), "OpenAI", "sk-existing", true);
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[existing]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let error = match store
            .ai_model_providers()
            .create(NewAiModelProvider {
                id: Uuid::new_v4(),
                name: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                default_model: "gpt-4.1-mini".to_string(),
                timeout_seconds: 60,
                api_key_secret: "sk-new".to_string(),
                enabled: true,
                created_at: Utc::now(),
            })
            .await
        {
            Ok(_) => panic!("duplicate provider should fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn updates_ai_model_provider_without_replacing_missing_secret() -> anyhow::Result<()> {
        let id = Uuid::new_v4();
        let existing = ai_model_provider_model(id, "OpenAI", "sk-existing-secret", true);
        let mut updated = existing.clone();
        updated.name = "OpenAI Compatible".to_string();
        updated.default_model = "gpt-4.1".to_string();
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[existing]])
            .append_query_results([Vec::<entities::ai_model_providers::Model>::new()])
            .append_query_results([[updated]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let provider = store
            .ai_model_providers()
            .update(
                id,
                AiModelProviderChanges {
                    name: Some("OpenAI Compatible".to_string()),
                    default_model: Some("gpt-4.1".to_string()),
                    ..AiModelProviderChanges::default()
                },
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("provider should update"))?;

        assert_eq!(provider.name, "OpenAI Compatible");
        assert_eq!(provider.default_model, "gpt-4.1");
        assert_eq!(provider.api_key_hint.as_deref(), Some("...cret"));
        Ok(())
    }

    #[tokio::test]
    async fn updates_ai_model_provider_secret_when_supplied() -> anyhow::Result<()> {
        let id = Uuid::new_v4();
        let existing = ai_model_provider_model(id, "OpenAI", "sk-existing-secret", true);
        let mut updated = existing.clone();
        updated.api_key_secret = "sk-replacement-secret".to_string();
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[existing]])
            .append_query_results([Vec::<entities::ai_model_providers::Model>::new()])
            .append_query_results([[updated]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let provider = store
            .ai_model_providers()
            .update(
                id,
                AiModelProviderChanges {
                    api_key_secret: Some("sk-replacement-secret".to_string()),
                    ..AiModelProviderChanges::default()
                },
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("provider should update"))?;

        assert_eq!(provider.api_key_hint.as_deref(), Some("...cret"));
        Ok(())
    }

    #[tokio::test]
    async fn updates_user_display_name() -> anyhow::Result<()> {
        let id = Uuid::new_v4();
        let existing = user_model(id, "admin", "Admin", "hash", "active");
        let updated = user_model(id, "admin", "Home Operator", "hash", "active");
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[existing]])
            .append_query_results([[updated]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let user = store
            .users()
            .update_display_name(id, "Home Operator".to_string(), Utc::now())
            .await?
            .ok_or_else(|| anyhow::anyhow!("user should update"))?;

        assert_eq!(user.display_name, "Home Operator");
        Ok(())
    }

    #[tokio::test]
    async fn updates_user_password_and_revokes_refresh_tokens() -> anyhow::Result<()> {
        let id = Uuid::new_v4();
        let existing = user_model(id, "admin", "Admin", "old-hash", "active");
        let updated = user_model(id, "admin", "Admin", "new-hash", "active");
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[existing]])
            .append_query_results([[updated]])
            .append_exec_results([mock_exec_result()])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let user = store
            .users()
            .update_password_hash_and_revoke_refresh_tokens(id, "new-hash".to_string(), Utc::now())
            .await?
            .ok_or_else(|| anyhow::anyhow!("user should update"))?;

        assert_eq!(user.password_hash, "new-hash");
        Ok(())
    }

    #[tokio::test]
    async fn deletes_ai_model_provider_and_reports_whether_row_existed() -> anyhow::Result<()> {
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let deleted = store.ai_model_providers().delete(Uuid::new_v4()).await?;

        assert!(deleted);
        Ok(())
    }

    #[tokio::test]
    async fn loads_ai_model_provider_secret_for_internal_dispatch() -> anyhow::Result<()> {
        let id = Uuid::new_v4();
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[ai_model_provider_model(id, "OpenAI", "sk-secret", true)]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let secret = store
            .ai_model_providers()
            .get_secret(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("provider should exist"))?;

        assert_eq!(secret.api_key_secret, "sk-secret");
        assert!(secret.enabled);
        Ok(())
    }

    #[tokio::test]
    async fn persists_ai_chat_conversation_messages_and_events() -> anyhow::Result<()> {
        let conversation_id = Uuid::new_v4();
        let user_message_id = Uuid::new_v4();
        let assistant_message_id = Uuid::new_v4();
        let event_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let provider_id = Uuid::new_v4();
        let created_at = Utc::now();
        let appended_at = created_at + chrono::Duration::seconds(1);
        let event_at = appended_at + chrono::Duration::seconds(1);
        let secret = "sk-never-persist";

        let conversation = ai_conversation_model(
            conversation_id,
            "Storage check",
            host_id,
            provider_id,
            "admin",
            created_at,
        );
        let conversation_after_user = ai_conversation_model(
            conversation_id,
            "Storage check",
            host_id,
            provider_id,
            "admin",
            created_at,
        );
        let conversation_after_assistant = ai_conversation_model(
            conversation_id,
            "Storage check",
            host_id,
            provider_id,
            "admin",
            created_at,
        );
        let conversation_after_append = ai_conversation_model(
            conversation_id,
            "Storage check",
            host_id,
            provider_id,
            "admin",
            appended_at,
        );
        let conversation_after_event = ai_conversation_model(
            conversation_id,
            "Storage check",
            host_id,
            provider_id,
            "admin",
            event_at,
        );
        let user_message = ai_chat_message_model(
            user_message_id,
            conversation_id,
            AiChatMessageRole::User,
            AiChatMessageStatus::Succeeded,
            "你好",
            None,
            Some(host_id),
            Some(provider_id),
            Some("gpt-4.1-mini"),
            created_at,
        );
        let assistant_message = ai_chat_message_model(
            assistant_message_id,
            conversation_id,
            AiChatMessageRole::Assistant,
            AiChatMessageStatus::Running,
            "",
            Some(task_id),
            Some(host_id),
            Some(provider_id),
            Some("gpt-4.1-mini"),
            created_at,
        );
        let mut appended_assistant = assistant_message.clone();
        appended_assistant.content = "收到".to_string();
        appended_assistant.updated_at = appended_at.into();
        let event = ai_chat_event_model(
            event_id,
            conversation_id,
            assistant_message_id,
            AiChatEventKind::ToolResult,
            Some("工具完成"),
            json!({"tool": "shell_execute", "status": "ok"}),
            event_at,
        );

        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[conversation.clone()]])
            .append_query_results([[user_message.clone()]])
            .append_query_results([[conversation.clone()]])
            .append_query_results([[conversation_after_user]])
            .append_query_results([[assistant_message.clone()]])
            .append_query_results([[conversation.clone()]])
            .append_query_results([[conversation_after_assistant]])
            .append_query_results([[assistant_message]])
            .append_query_results([[appended_assistant.clone()]])
            .append_query_results([[conversation.clone()]])
            .append_query_results([[conversation_after_append.clone()]])
            .append_query_results([[event.clone()]])
            .append_query_results([[conversation_after_append]])
            .append_query_results([[conversation_after_event]])
            .append_query_results([[user_message.clone(), appended_assistant.clone()]])
            .append_query_results([[event.clone()]])
            .append_query_results([[event]])
            .append_query_results([[appended_assistant.clone()]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let created = store
            .ai_chats()
            .create_conversation(NewAiConversation {
                id: conversation_id,
                title: " Storage check ".to_string(),
                host_id,
                ai_provider_id: provider_id,
                created_by: "admin".to_string(),
                created_at,
            })
            .await?;
        let user = store
            .ai_chats()
            .create_message(NewAiChatMessage {
                id: user_message_id,
                conversation_id,
                role: AiChatMessageRole::User,
                status: AiChatMessageStatus::Succeeded,
                content: "你好".to_string(),
                task_id: None,
                host_id: Some(host_id),
                ai_provider_id: Some(provider_id),
                model: Some("gpt-4.1-mini".to_string()),
                metadata: json!({}),
                created_at,
            })
            .await?;
        let assistant = store
            .ai_chats()
            .create_message(NewAiChatMessage {
                id: assistant_message_id,
                conversation_id,
                role: AiChatMessageRole::Assistant,
                status: AiChatMessageStatus::Running,
                content: String::new(),
                task_id: Some(task_id),
                host_id: Some(host_id),
                ai_provider_id: Some(provider_id),
                model: Some("gpt-4.1-mini".to_string()),
                metadata: json!({}),
                created_at,
            })
            .await?;
        let appended = store
            .ai_chats()
            .append_message_content(assistant_message_id, "收到", appended_at)
            .await?
            .ok_or_else(|| anyhow::anyhow!("assistant message should append"))?;
        let recorded = store
            .ai_chats()
            .record_event(NewAiChatEvent {
                id: event_id,
                conversation_id,
                message_id: assistant_message_id,
                kind: AiChatEventKind::ToolResult,
                content: Some("工具完成".to_string()),
                payload: json!({"tool": "shell_execute", "status": "ok"}),
                created_at: event_at,
            })
            .await?;
        let messages = store.ai_chats().list_messages(conversation_id).await?;
        let events = store.ai_chats().list_events(conversation_id).await?;
        let message_events = store
            .ai_chats()
            .list_message_events(assistant_message_id)
            .await?;
        let message_for_task = store
            .ai_chats()
            .message_for_task(task_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("assistant message should be linked to task"))?;

        assert_eq!(created.title, "Storage check");
        assert_eq!(user.role, AiChatMessageRole::User);
        assert_eq!(assistant.task_id, Some(task_id));
        assert_eq!(appended.content, "收到");
        assert_eq!(recorded.kind, AiChatEventKind::ToolResult);
        assert_eq!(messages.len(), 2);
        assert_eq!(events.len(), 1);
        assert_eq!(message_events[0].message_id, assistant_message_id);
        assert_eq!(message_for_task.id, assistant_message_id);
        assert!(!serde_json::to_string(&messages)?.contains(secret));
        assert!(!serde_json::to_string(&events)?.contains(secret));
        Ok(())
    }

    #[tokio::test]
    async fn creates_website_and_maps_protocol_fields() -> anyhow::Result<()> {
        let id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let now = Utc::now();
        let mut inserted = website_model(id, "example.com", WebsiteStatus::Stopped);
        inserted.host_id = Some(host_id);
        inserted.aliases = json!(["www.example.com"]);
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<entities::websites::Model>::new()])
            .append_query_results([[inserted]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let website = store
            .websites()
            .create(NewWebsite {
                id,
                host_id,
                name: "example.com".to_string(),
                primary_domain: "example.com".to_string(),
                aliases: vec!["www.example.com".to_string()],
                status: WebsiteStatus::Stopped,
                kind: WebsiteKind::ReverseProxy,
                protocol: WebsiteProtocol::Http,
                listen_port: 8080,
                upstream_url: "http://127.0.0.1:8787".to_string(),
                app_install_id: None,
                tls_certificate_id: None,
                config: json!({}),
                notes: Some("local app".to_string()),
                created_at: now,
            })
            .await?;

        assert_eq!(website.id, id);
        assert_eq!(website.host_id, Some(host_id));
        assert_eq!(website.status, WebsiteStatus::Stopped);
        assert_eq!(website.kind, WebsiteKind::ReverseProxy);
        assert_eq!(website.protocol, WebsiteProtocol::Http);
        assert_eq!(website.aliases, vec!["www.example.com"]);
        Ok(())
    }

    #[tokio::test]
    async fn rejects_duplicate_website_domain_and_listen_port_on_same_host() {
        let host_id = Uuid::new_v4();
        let mut existing = website_model(Uuid::new_v4(), "Example.com", WebsiteStatus::Stopped);
        existing.host_id = Some(host_id);
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[existing]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let error = match store
            .websites()
            .create(NewWebsite {
                id: Uuid::new_v4(),
                host_id,
                name: "example".to_string(),
                primary_domain: "example.com".to_string(),
                aliases: Vec::new(),
                status: WebsiteStatus::Stopped,
                kind: WebsiteKind::ReverseProxy,
                protocol: WebsiteProtocol::Http,
                listen_port: 8080,
                upstream_url: "http://127.0.0.1:8787".to_string(),
                app_install_id: None,
                tls_certificate_id: None,
                config: json!({}),
                notes: None,
                created_at: Utc::now(),
            })
            .await
        {
            Ok(_) => panic!("duplicate website should fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn allows_duplicate_website_domain_and_listen_port_on_different_hosts()
    -> anyhow::Result<()> {
        let id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let mut existing = website_model(Uuid::new_v4(), "Example.com", WebsiteStatus::Stopped);
        existing.host_id = Some(Uuid::new_v4());
        let mut inserted = website_model(id, "example.com", WebsiteStatus::Stopped);
        inserted.host_id = Some(host_id);
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[existing]])
            .append_query_results([[inserted]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let website = store
            .websites()
            .create(NewWebsite {
                id,
                host_id,
                name: "example".to_string(),
                primary_domain: "example.com".to_string(),
                aliases: Vec::new(),
                status: WebsiteStatus::Stopped,
                kind: WebsiteKind::ReverseProxy,
                protocol: WebsiteProtocol::Http,
                listen_port: 8080,
                upstream_url: "http://127.0.0.1:8787".to_string(),
                app_install_id: None,
                tls_certificate_id: None,
                config: json!({}),
                notes: None,
                created_at: Utc::now(),
            })
            .await?;

        assert_eq!(website.host_id, Some(host_id));
        Ok(())
    }

    #[tokio::test]
    async fn lists_running_websites_by_host() -> anyhow::Result<()> {
        let host_id = Uuid::new_v4();
        let mut website = website_model(Uuid::new_v4(), "example.com", WebsiteStatus::Running);
        website.host_id = Some(host_id);
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[website]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let websites = store.websites().running_by_host(host_id).await?;

        assert_eq!(websites.len(), 1);
        assert_eq!(websites[0].host_id, Some(host_id));
        Ok(())
    }

    #[tokio::test]
    async fn running_website_configuration_is_not_updated() {
        let id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let mut website = website_model(id, "example.com", WebsiteStatus::Running);
        website.host_id = Some(host_id);
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([[website]])
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        let error = match store
            .websites()
            .update_stopped(
                id,
                WebsiteChanges {
                    host_id,
                    name: "changed".to_string(),
                    primary_domain: "changed.example".to_string(),
                    aliases: Vec::new(),
                    listen_port: 8080,
                    upstream_url: "http://127.0.0.1:8788".to_string(),
                    notes: None,
                },
            )
            .await
        {
            Ok(_) => panic!("running website should not update"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("must be stopped"));
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

    fn user_model(
        id: Uuid,
        username: &str,
        display_name: &str,
        password_hash: &str,
        status: &str,
    ) -> entities::users::Model {
        entities::users::Model {
            id,
            username: username.to_string(),
            display_name: display_name.to_string(),
            password_hash: password_hash.to_string(),
            role: "admin".to_string(),
            status: status.to_string(),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            last_login_at: None,
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

    fn ai_model_provider_model(
        id: Uuid,
        name: &str,
        api_key_secret: &str,
        enabled: bool,
    ) -> entities::ai_model_providers::Model {
        entities::ai_model_providers::Model {
            id,
            name: name.to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            default_model: "gpt-4.1-mini".to_string(),
            timeout_seconds: 60,
            api_key_secret: api_key_secret.to_string(),
            enabled,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    fn ai_conversation_model(
        id: Uuid,
        title: &str,
        host_id: Uuid,
        ai_provider_id: Uuid,
        created_by: &str,
        updated_at: DateTime<Utc>,
    ) -> entities::ai_conversations::Model {
        entities::ai_conversations::Model {
            id,
            title: title.to_string(),
            host_id: Some(host_id),
            ai_provider_id: Some(ai_provider_id),
            created_by: created_by.to_string(),
            created_at: updated_at.into(),
            updated_at: updated_at.into(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn ai_chat_message_model(
        id: Uuid,
        conversation_id: Uuid,
        role: AiChatMessageRole,
        status: AiChatMessageStatus,
        content: &str,
        task_id: Option<Uuid>,
        host_id: Option<Uuid>,
        ai_provider_id: Option<Uuid>,
        model: Option<&str>,
        created_at: DateTime<Utc>,
    ) -> entities::ai_chat_messages::Model {
        entities::ai_chat_messages::Model {
            id,
            conversation_id,
            role: serialize_ai_chat_message_role(role),
            status: serialize_ai_chat_message_status(status),
            content: content.to_string(),
            task_id,
            host_id,
            ai_provider_id,
            model: model.map(ToString::to_string),
            metadata: json!({}),
            created_at: created_at.into(),
            updated_at: created_at.into(),
        }
    }

    fn ai_chat_event_model(
        id: Uuid,
        conversation_id: Uuid,
        message_id: Uuid,
        kind: AiChatEventKind,
        content: Option<&str>,
        payload: Value,
        created_at: DateTime<Utc>,
    ) -> entities::ai_chat_events::Model {
        entities::ai_chat_events::Model {
            id,
            conversation_id,
            message_id,
            kind: serialize_ai_chat_event_kind(kind),
            content: content.map(ToString::to_string),
            payload,
            created_at: created_at.into(),
        }
    }

    fn website_model(
        id: Uuid,
        primary_domain: &str,
        status: WebsiteStatus,
    ) -> entities::websites::Model {
        entities::websites::Model {
            id,
            host_id: None,
            name: primary_domain.to_string(),
            primary_domain: primary_domain.to_string(),
            aliases: json!([]),
            status: serialize_website_status(status),
            kind: serialize_website_kind(WebsiteKind::ReverseProxy),
            protocol: serialize_website_protocol(WebsiteProtocol::Http),
            listen_port: 8080,
            upstream_url: "http://127.0.0.1:8787".to_string(),
            app_install_id: None,
            tls_certificate_id: None,
            config: json!({}),
            notes: Some("local app".to_string()),
            last_runtime_error: None,
            last_checked_at: None,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }
}
