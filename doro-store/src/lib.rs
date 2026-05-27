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
use sea_orm::Set;
use sea_orm::Statement;
use sea_orm::TransactionTrait;
use serde_json::Value;
use serde_json::json;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

pub mod entities;

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
pub struct AgentRegistration {
    pub agent_id: Uuid,
    pub host_id: Uuid,
    pub hostname: String,
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
        for migration in migrations() {
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

    pub fn settings(&self) -> SettingsRepository<'_> {
        SettingsRepository { store: self }
    }

    pub fn apps(&self) -> AppRepository<'_> {
        AppRepository { store: self }
    }

    async fn execute_sql(&self, sql: &str) -> Result<(), DbErr> {
        let statement = Statement::from_string(self.backend, sql.to_string());
        self.connection.execute_raw(statement).await?;
        Ok(())
    }

    async fn execute_sql_batch(&self, sql: &str) -> Result<(), DbErr> {
        for statement in split_sql_statements(sql) {
            self.execute_sql(&statement).await?;
        }
        Ok(())
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
            last_seen_at: Set(Some(observed_at.into())),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };
        entities::hosts::Entity::insert(model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(entities::hosts::Column::Id)
                    .update_columns([
                        entities::hosts::Column::Hostname,
                        entities::hosts::Column::DisplayName,
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

        Ok(Host {
            id: host.id,
            hostname: host.hostname,
            labels: json_array_strings(host.labels),
            status: parse_host_status(&host.status).unwrap_or(HostStatus::Pending),
            last_seen_at: host.last_seen_at.map(Into::into),
            capabilities,
        })
    }
}

pub struct AgentRepository<'a> {
    store: &'a Store,
}

impl AgentRepository<'_> {
    pub async fn register(&self, registration: AgentRegistration) -> Result<(), DbErr> {
        let transaction = self.store.connection().begin().await?;
        upsert_host(
            &transaction,
            registration.host_id,
            registration.hostname,
            registration.observed_at,
        )
        .await?;
        upsert_agent(
            &transaction,
            registration.agent_id,
            registration.host_id,
            registration.observed_at,
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
        transaction.commit().await?;
        Ok(())
    }

    pub async fn heartbeat(&self, heartbeat: AgentHeartbeat) -> Result<(), DbErr> {
        let transaction = self.store.connection().begin().await?;
        upsert_agent(
            &transaction,
            heartbeat.agent_id,
            heartbeat.host_id,
            heartbeat.observed_at,
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
        let approvals = entities::approvals::Entity::find()
            .order_by(entities::approvals::Column::RequestedAt, Order::Desc)
            .all(self.store.connection())
            .await?;
        Ok(approvals
            .into_iter()
            .map(|approval| ApprovalRequest {
                id: approval.id,
                task_id: approval.task_id,
                step_id: approval.step_id,
                reason: approval.reason,
                status: parse_approval_status(&approval.status).unwrap_or(ApprovalStatus::Pending),
                requested_at: approval.requested_at.into(),
            })
            .collect())
    }
}

pub struct EventRepository<'a> {
    store: &'a Store,
}

impl EventRepository<'_> {
    pub async fn record(&self, event: NewAgentEvent) -> Result<(), DbErr> {
        entities::agent_events::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            host_id: Set(event.host_id),
            agent_id: Set(event.agent_id),
            event_type: Set(event.event_type),
            event_json: Set(event.event_json),
            recorded_at: Set(event.recorded_at.into()),
        }
        .insert(self.store.connection())
        .await?;
        Ok(())
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

    pub fn from_protocol(snapshot: MetricSnapshot) -> NewMetricSnapshot {
        NewMetricSnapshot {
            host_id: snapshot.host_id,
            captured_at: snapshot.captured_at,
            cpu_percent: snapshot.cpu_percent,
            memory_percent: snapshot.memory_percent,
            disk_percent: snapshot.disk_percent,
            load_average: snapshot.load_average,
            extra: json!({}),
        }
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
}

pub struct AppRepository<'a> {
    store: &'a Store,
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
        last_seen_at: Set(Some(observed_at.into())),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    })
    .on_conflict(
        sea_orm::sea_query::OnConflict::column(entities::hosts::Column::Id)
            .update_columns([
                entities::hosts::Column::Hostname,
                entities::hosts::Column::DisplayName,
                entities::hosts::Column::Status,
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
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    entities::agents::Entity::insert(entities::agents::ActiveModel {
        id: Set(agent_id),
        host_id: Set(host_id),
        status: Set("online".to_string()),
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

#[derive(Debug, Clone, Copy)]
struct Migration {
    id: &'static str,
    sql: &'static str,
}

fn migrations() -> &'static [Migration] {
    &[
        Migration {
            id: "202605270001_schema_migrations",
            sql: r#"
                CREATE TABLE IF NOT EXISTS doro_schema_migrations (
                    id TEXT PRIMARY KEY,
                    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );
            "#,
        },
        Migration {
            id: "202605270002_core_store_schema",
            sql: CORE_SCHEMA_SQL,
        },
        Migration {
            id: "202605270003_seed_apps_settings",
            sql: SEED_SQL,
        },
    ]
}

#[cfg(test)]
fn migration_statements() -> Vec<&'static str> {
    migrations().iter().map(|migration| migration.sql).collect()
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(|statement| format!("{statement};"))
        .collect()
}

const CORE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS hosts (
    id UUID PRIMARY KEY,
    hostname TEXT NOT NULL,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL,
    labels JSONB NOT NULL DEFAULT '[]'::jsonb,
    last_seen_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS agents (
    id UUID PRIMARY KEY,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    version TEXT,
    protocol_version TEXT,
    last_seen_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agents_host_id ON agents(host_id);

CREATE TABLE IF NOT EXISTS enrollment_tokens (
    id UUID PRIMARY KEY,
    label TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    expires_at TIMESTAMPTZ,
    used_at TIMESTAMPTZ,
    used_by_agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS agent_capabilities (
    id UUID PRIMARY KEY,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    risk TEXT NOT NULL,
    description TEXT NOT NULL,
    declared_at TIMESTAMPTZ NOT NULL,
    UNIQUE(agent_id, name)
);
CREATE INDEX IF NOT EXISTS idx_agent_capabilities_host_id ON agent_capabilities(host_id);

CREATE TABLE IF NOT EXISTS metric_snapshots (
    id BIGSERIAL PRIMARY KEY,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    captured_at TIMESTAMPTZ NOT NULL,
    cpu_percent REAL NOT NULL,
    memory_percent REAL NOT NULL,
    disk_percent REAL NOT NULL,
    load_average REAL NOT NULL,
    extra JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS idx_metric_snapshots_host_captured_at ON metric_snapshots(host_id, captured_at DESC);

CREATE TABLE IF NOT EXISTS agent_events (
    id BIGSERIAL PRIMARY KEY,
    host_id UUID REFERENCES hosts(id) ON DELETE SET NULL,
    agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    event_json JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_agent_events_recorded_at ON agent_events(recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_events_host_id ON agent_events(host_id);

CREATE TABLE IF NOT EXISTS tasks (
    id UUID PRIMARY KEY,
    host_id UUID REFERENCES hosts(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    prompt TEXT,
    status TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    queued_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error_message TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_host_id ON tasks(host_id);

CREATE TABLE IF NOT EXISTS task_steps (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    capability TEXT NOT NULL,
    risk TEXT NOT NULL,
    summary TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE(task_id, position)
);
CREATE INDEX IF NOT EXISTS idx_task_steps_task_id ON task_steps(task_id);

CREATE TABLE IF NOT EXISTS task_runs (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    step_id UUID REFERENCES task_steps(id) ON DELETE SET NULL,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    command_id TEXT,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    result_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT
);
CREATE INDEX IF NOT EXISTS idx_task_runs_task_id ON task_runs(task_id);
CREATE INDEX IF NOT EXISTS idx_task_runs_agent_id ON task_runs(agent_id);

CREATE TABLE IF NOT EXISTS approvals (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    step_id UUID NOT NULL REFERENCES task_steps(id) ON DELETE CASCADE,
    reason TEXT NOT NULL,
    status TEXT NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL,
    resolved_at TIMESTAMPTZ,
    resolved_by TEXT,
    decision_note TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_approvals_step_pending ON approvals(step_id) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_approvals_requested_at ON approvals(requested_at DESC);

CREATE TABLE IF NOT EXISTS operation_logs (
    id BIGSERIAL PRIMARY KEY,
    source TEXT NOT NULL,
    actor TEXT,
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    latency_ms INTEGER NOT NULL,
    message TEXT,
    detail JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    description TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS resource_groups (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(kind, name)
);

CREATE TABLE IF NOT EXISTS apps (
    id UUID PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    status TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS app_installs (
    id UUID PRIMARY KEY,
    app_id UUID NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    status TEXT NOT NULL,
    ports JSONB NOT NULL DEFAULT '[]'::jsonb,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS websites (
    id UUID PRIMARY KEY,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    primary_domain TEXT NOT NULL,
    status TEXT NOT NULL,
    protocol TEXT NOT NULL,
    app_install_id UUID REFERENCES app_installs(id) ON DELETE SET NULL,
    tls_certificate_id UUID,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS databases (
    id UUID PRIMARY KEY,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    app_install_id UUID REFERENCES app_installs(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    engine TEXT NOT NULL,
    version TEXT NOT NULL,
    status TEXT NOT NULL,
    endpoint JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS containers (
    id UUID PRIMARY KEY,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    runtime TEXT NOT NULL,
    container_ref TEXT NOT NULL,
    name TEXT NOT NULL,
    image TEXT NOT NULL,
    status TEXT NOT NULL,
    ports JSONB NOT NULL DEFAULT '[]'::jsonb,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    observed_at TIMESTAMPTZ NOT NULL,
    UNIQUE(host_id, runtime, container_ref)
);

CREATE TABLE IF NOT EXISTS backup_accounts (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    is_public BOOLEAN NOT NULL DEFAULT FALSE,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS backup_records (
    id UUID PRIMARY KEY,
    account_id UUID REFERENCES backup_accounts(id) ON DELETE SET NULL,
    task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    resource_kind TEXT NOT NULL,
    resource_name TEXT NOT NULL,
    status TEXT NOT NULL,
    file_path TEXT,
    message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS cron_jobs (
    id UUID PRIMARY KEY,
    host_id UUID REFERENCES hosts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    schedule TEXT NOT NULL,
    status TEXT NOT NULL,
    task_template JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS cron_job_runs (
    id UUID PRIMARY KEY,
    cron_job_id UUID NOT NULL REFERENCES cron_jobs(id) ON DELETE CASCADE,
    task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    status TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    message TEXT
);
"#;

const SEED_SQL: &str = r#"
INSERT INTO apps (id, key, name, category, status, description, metadata)
VALUES
    ('00000000-0000-0000-0000-000000000101', 'mysql', 'MySQL', 'database', 'planned', 'Relational database service', '{}'::jsonb),
    ('00000000-0000-0000-0000-000000000102', 'openresty', 'OpenResty', 'website', 'planned', 'Web server and reverse proxy', '{}'::jsonb),
    ('00000000-0000-0000-0000-000000000103', 'redis', 'Redis', 'database', 'planned', 'In-memory data store', '{}'::jsonb)
ON CONFLICT (key) DO NOTHING;

INSERT INTO settings (key, value, description)
VALUES
    ('approval_policy', '"policy_and_human_approval"'::jsonb, 'Control-plane approval mode'),
    ('agent_transport', '"grpc_protobuf"'::jsonb, 'Agent transport protocol'),
    ('database', '"postgres"'::jsonb, 'Configured store backend')
ON CONFLICT (key) DO NOTHING;
"#;

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
        let exec_count = migrations()
            .iter()
            .map(|migration| split_sql_statements(migration.sql).len() + 1)
            .sum::<usize>();
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results((0..exec_count).map(|_| mock_exec_result()))
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        store.migrate().await?;

        Ok(())
    }

    #[test]
    fn migration_sql_uses_postgres_native_types() {
        let sql = migration_statements().join("\n");

        assert!(sql.contains("UUID PRIMARY KEY"));
        assert!(sql.contains("JSONB NOT NULL"));
        assert!(sql.contains("TIMESTAMPTZ"));
        assert!(sql.contains("BIGSERIAL PRIMARY KEY"));
        assert!(sql.contains("idx_metric_snapshots_host_captured_at"));
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
        let statements = split_sql_statements("CREATE TABLE a (id int);\nCREATE TABLE b (id int);");

        assert_eq!(statements.len(), 2);
        assert!(statements[0].ends_with(';'));
    }

    fn mock_exec_result() -> MockExecResult {
        MockExecResult {
            last_insert_id: 0,
            rows_affected: 0,
        }
    }
}
