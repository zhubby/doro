use doro_config::StoreBackend;
use doro_config::StoreConfig;
use sea_orm::ConnectOptions;
use sea_orm::ConnectionTrait;
use sea_orm::Database;
use sea_orm::DatabaseBackend;
use sea_orm::DatabaseConnection;
use sea_orm::DbErr;
use sea_orm::Statement;
use std::time::Duration;

pub mod entities;

#[derive(Debug, Clone)]
pub struct Store {
    connection: DatabaseConnection,
    backend: DatabaseBackend,
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
        for sql in migration_statements() {
            self.execute_sql(sql).await?;
        }

        Ok(())
    }

    pub fn connection(&self) -> &DatabaseConnection {
        &self.connection
    }

    async fn execute_sql(&self, sql: &str) -> Result<(), DbErr> {
        let statement = Statement::from_string(self.backend, sql.to_string());
        self.connection.execute_raw(statement).await?;
        Ok(())
    }
}

fn database_backend(backend: StoreBackend) -> DatabaseBackend {
    match backend {
        StoreBackend::Postgres => DatabaseBackend::Postgres,
    }
}

fn migration_statements() -> [&'static str; 4] {
    [
        r#"
            CREATE TABLE IF NOT EXISTS hosts (
                id TEXT PRIMARY KEY,
                hostname TEXT NOT NULL,
                status TEXT NOT NULL,
                last_seen_at TIMESTAMPTZ
            );
            "#,
        r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                host_id TEXT,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL
            );
            "#,
        r#"
            CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                step_id TEXT NOT NULL,
                reason TEXT NOT NULL,
                status TEXT NOT NULL,
                requested_at TIMESTAMPTZ NOT NULL
            );
            "#,
        r#"
            CREATE TABLE IF NOT EXISTS agent_events (
                id BIGSERIAL PRIMARY KEY,
                event_json JSONB NOT NULL,
                recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::MockDatabase;
    use sea_orm::MockExecResult;

    #[tokio::test]
    async fn migrate_executes_postgres_schema_statements() -> anyhow::Result<()> {
        let connection = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results((0..4).map(|_| mock_exec_result()))
            .into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);

        store.migrate().await?;

        Ok(())
    }

    #[test]
    fn migration_sql_is_postgres_native() {
        let sql = migration_statements().join("\n");

        assert!(sql.contains("BIGSERIAL PRIMARY KEY"));
        assert!(sql.contains("JSONB NOT NULL"));
        assert!(sql.contains("TIMESTAMPTZ"));
        assert!(!sql.contains("AUTOINCREMENT"));
        assert!(!sql.contains("sqlite_master"));
    }

    fn mock_exec_result() -> MockExecResult {
        MockExecResult {
            last_insert_id: 0,
            rows_affected: 0,
        }
    }
}
