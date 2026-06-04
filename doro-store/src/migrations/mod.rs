#[derive(Debug, Clone, Copy)]
pub(crate) struct Migration {
    pub(crate) id: &'static str,
    pub(crate) sql: &'static str,
}

pub(crate) const SCHEMA_MIGRATIONS: Migration = Migration {
    id: "202605270001_schema_migrations",
    sql: include_str!("../../migrations/202605270001_schema_migrations.sql"),
};

pub(crate) fn all() -> &'static [Migration] {
    &[
        SCHEMA_MIGRATIONS,
        Migration {
            id: "202605270002_core_store_schema",
            sql: include_str!("../../migrations/202605270002_core_store_schema.sql"),
        },
        Migration {
            id: "202605270002_timescale_hypertables",
            sql: include_str!("../../migrations/202605270002_timescale_hypertables.sql"),
        },
        Migration {
            id: "202605270003_seed_apps_settings",
            sql: include_str!("../../migrations/202605270003_seed_apps_settings.sql"),
        },
        Migration {
            id: "202605270004_users_refresh_tokens",
            sql: include_str!("../../migrations/202605270004_users_refresh_tokens.sql"),
        },
        Migration {
            id: "202605280001_host_system_profile",
            sql: include_str!("../../migrations/202605280001_host_system_profile.sql"),
        },
        Migration {
            id: "202605290001_container_created_at",
            sql: include_str!("../../migrations/202605290001_container_created_at.sql"),
        },
        Migration {
            id: "202605300001_approval_expiration",
            sql: include_str!("../../migrations/202605300001_approval_expiration.sql"),
        },
        Migration {
            id: "202605300002_virtual_machines",
            sql: include_str!("../../migrations/202605300002_virtual_machines.sql"),
        },
        Migration {
            id: "202605310001_websites_pingora",
            sql: include_str!("../../migrations/202605310001_websites_pingora.sql"),
        },
        Migration {
            id: "202606020001_scheduled_tasks",
            sql: include_str!("../../migrations/202606020001_scheduled_tasks.sql"),
        },
        Migration {
            id: "202606030001_agent_side_websites",
            sql: include_str!("../../migrations/202606030001_agent_side_websites.sql"),
        },
        Migration {
            id: "202606030002_ai_model_providers",
            sql: include_str!("../../migrations/202606030002_ai_model_providers.sql"),
        },
        Migration {
            id: "202606030003_ai_chat",
            sql: include_str!("../../migrations/202606030003_ai_chat.sql"),
        },
        Migration {
            id: "202606040001_agent_event_external_ids",
            sql: include_str!("../../migrations/202606040001_agent_event_external_ids.sql"),
        },
        Migration {
            id: "202606040002_ai_conversation_bindings",
            sql: include_str!("../../migrations/202606040002_ai_conversation_bindings.sql"),
        },
    ]
}

pub(crate) fn split_sql_statements(sql: &str) -> Vec<String> {
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(|statement| format!("{statement};"))
        .collect()
}
