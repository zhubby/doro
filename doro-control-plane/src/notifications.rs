use crate::error::{AppError, normalize_optional_text};
use crate::prelude::*;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

const EMAIL_SETTINGS_KEY: &str = "notification_email";
const SYSTEM_SETTINGS_KEY: &str = "notification_system";
const DEFAULT_SUBJECT_PREFIX: &str = "[Doro]";

#[derive(Debug, Clone)]
pub(crate) struct EmailSettingsSecret {
    pub enabled: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub security: EmailSecurityMode,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_address: String,
    pub recipients: Vec<String>,
    pub subject_prefix: String,
}

impl EmailSettingsSecret {
    fn public(&self) -> EmailNotificationSettings {
        EmailNotificationSettings {
            enabled: self.enabled,
            smtp_host: self.smtp_host.clone(),
            smtp_port: self.smtp_port,
            security: self.security,
            username: self.username.clone(),
            from_address: self.from_address.clone(),
            recipients: self.recipients.clone(),
            subject_prefix: self.subject_prefix.clone(),
            has_password: self
                .password
                .as_deref()
                .is_some_and(|password| !password.trim().is_empty()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SystemNotificationSettingsSecret {
    pub enabled: bool,
}

impl SystemNotificationSettingsSecret {
    fn public(self) -> SystemNotificationSettings {
        SystemNotificationSettings {
            enabled: self.enabled,
        }
    }
}

pub(crate) async fn public_email_settings(
    store: &Store,
) -> Result<EmailNotificationSettings, AppError> {
    Ok(load_email_settings(store).await?.public())
}

pub(crate) async fn public_system_notification_settings(
    store: &Store,
) -> Result<SystemNotificationSettings, AppError> {
    Ok(load_system_notification_settings(store).await?.public())
}

pub(crate) async fn save_system_notification_settings(
    store: &Store,
    request: UpdateSystemNotificationSettingsRequest,
) -> Result<SystemNotificationSettings, AppError> {
    let settings = SystemNotificationSettingsSecret {
        enabled: request.enabled,
    };
    store
        .settings()
        .upsert_json(
            SYSTEM_SETTINGS_KEY,
            system_notification_settings_json(settings),
            Some("System in-app notification channel".to_string()),
        )
        .await?;
    Ok(settings.public())
}

pub(crate) async fn save_email_settings(
    store: &Store,
    request: UpdateEmailNotificationSettingsRequest,
) -> Result<EmailNotificationSettings, AppError> {
    let current = load_email_settings(store).await?;
    let smtp_host = request.smtp_host.trim().to_string();
    let username = normalize_optional_text(request.username);
    let from_address = request.from_address.trim().to_string();
    let recipients = normalize_recipients(request.recipients);
    let subject_prefix = if request.subject_prefix.trim().is_empty() {
        DEFAULT_SUBJECT_PREFIX.to_string()
    } else {
        request.subject_prefix.trim().to_string()
    };
    let password = if request.clear_password {
        None
    } else {
        normalize_optional_text(request.password).or(current.password)
    };
    let settings = EmailSettingsSecret {
        enabled: request.enabled,
        smtp_host,
        smtp_port: request.smtp_port,
        security: request.security,
        username,
        password,
        from_address,
        recipients,
        subject_prefix,
    };
    validate_email_settings(&settings)?;
    store
        .settings()
        .upsert_json(
            EMAIL_SETTINGS_KEY,
            email_settings_json(&settings),
            Some("Email notification channel".to_string()),
        )
        .await?;
    Ok(settings.public())
}

pub(crate) async fn list_system_notifications(
    store: &Store,
    status: Option<SystemNotificationStatus>,
    limit: u64,
) -> Result<Vec<SystemNotification>, AppError> {
    Ok(store.system_notifications().list(status, limit).await?)
}

pub(crate) async fn mark_system_notification_read(
    store: &Store,
    notification_id: Uuid,
) -> Result<SystemNotification, AppError> {
    store
        .system_notifications()
        .mark_read(notification_id, Utc::now())
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "system notification not found"))
}

pub(crate) async fn send_test_email(
    store: &Store,
    recipient: Option<String>,
) -> Result<(), AppError> {
    let settings = load_email_settings(store).await?;
    ensure_sendable(&settings)?;
    let recipient = recipient
        .and_then(|recipient| {
            let recipient = recipient.trim().to_string();
            (!recipient.is_empty()).then_some(recipient)
        })
        .unwrap_or_else(|| settings.recipients[0].clone());
    send_email(
        &settings,
        &recipient,
        "Doro 邮件通知测试",
        "这是一封来自 Doro 控制面的测试邮件。",
    )
    .await
    .map_err(|error| AppError::status(StatusCode::BAD_GATEWAY, error.to_string()))
}

pub(crate) async fn send_alert_email(
    store: &Store,
    rule: &AlertRule,
    incident: &AlertIncident,
    recovered: bool,
    observed_value: f32,
) -> Result<(), AppError> {
    let settings = load_email_settings(store).await?;
    if !settings.enabled {
        return Ok(());
    }
    ensure_sendable(&settings)?;

    let state_label = if recovered { "恢复" } else { "触发" };
    let subject = format!(
        "{} 告警{}：{}",
        settings.subject_prefix, state_label, rule.name
    );
    let body = format_alert_body(rule, incident, recovered, observed_value);
    for recipient in &settings.recipients {
        let now = Utc::now();
        let result = send_email(&settings, recipient, &subject, &body).await;
        let (status, error_message, sent_at) = match result {
            Ok(()) => (AlertNotificationStatus::Sent, None, Some(now)),
            Err(error) => (
                AlertNotificationStatus::Failed,
                Some(error.to_string()),
                None,
            ),
        };
        store
            .alerts()
            .record_notification(NewAlertNotification {
                id: Uuid::new_v4(),
                alert_incident_id: Some(incident.id),
                alert_rule_id: Some(rule.id),
                channel: "email".to_string(),
                status,
                recipient: recipient.clone(),
                subject: subject.clone(),
                error_message,
                sent_at,
                created_at: now,
            })
            .await?;
    }
    Ok(())
}

pub(crate) async fn create_alert_system_notification(
    store: &Store,
    rule: &AlertRule,
    incident: &AlertIncident,
    recovered: bool,
    observed_value: f32,
) -> Result<(), AppError> {
    let settings = load_system_notification_settings(store).await?;
    if !settings.enabled {
        return Ok(());
    }

    let state_label = if recovered { "恢复" } else { "触发" };
    let title = format!("告警{}：{}", state_label, rule.name);
    let body = format!(
        "{} {} {}，当前值 {}。",
        metric_label(&rule.metric),
        operator_label(rule.operator),
        rule.threshold,
        observed_value,
    );
    store
        .system_notifications()
        .create(NewSystemNotification {
            id: Uuid::new_v4(),
            source: SystemNotificationSource::Alert,
            severity: rule.severity,
            title,
            body,
            link_url: Some("/alerts".to_string()),
            alert_incident_id: Some(incident.id),
            alert_rule_id: Some(rule.id),
            host_id: Some(incident.host_id),
            created_at: Utc::now(),
        })
        .await?;
    Ok(())
}

pub(crate) async fn load_email_settings(store: &Store) -> Result<EmailSettingsSecret, AppError> {
    let value = store.settings().get_json(EMAIL_SETTINGS_KEY).await?;
    Ok(email_settings_from_json(
        value.unwrap_or_else(|| serde_json::json!({})),
    ))
}

pub(crate) async fn load_system_notification_settings(
    store: &Store,
) -> Result<SystemNotificationSettingsSecret, AppError> {
    let value = store.settings().get_json(SYSTEM_SETTINGS_KEY).await?;
    Ok(system_notification_settings_from_json(
        value.unwrap_or_else(|| serde_json::json!({})),
    ))
}

fn system_notification_settings_from_json(value: Value) -> SystemNotificationSettingsSecret {
    SystemNotificationSettingsSecret {
        enabled: value
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    }
}

fn system_notification_settings_json(settings: SystemNotificationSettingsSecret) -> Value {
    serde_json::json!({
        "enabled": settings.enabled,
    })
}

fn email_settings_from_json(value: Value) -> EmailSettingsSecret {
    EmailSettingsSecret {
        enabled: value
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        smtp_host: value
            .get("smtp_host")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        smtp_port: value
            .get("smtp_port")
            .and_then(Value::as_u64)
            .unwrap_or(587)
            .min(u16::MAX as u64) as u16,
        security: parse_email_security(
            value
                .get("security")
                .and_then(Value::as_str)
                .unwrap_or("start_tls"),
        ),
        username: value
            .get("username")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty()),
        password: value
            .get("password_secret")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty()),
        from_address: value
            .get("from_address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        recipients: value
            .get("recipients")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        subject_prefix: value
            .get("subject_prefix")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_SUBJECT_PREFIX)
            .to_string(),
    }
}

fn email_settings_json(settings: &EmailSettingsSecret) -> Value {
    serde_json::json!({
        "enabled": settings.enabled,
        "smtp_host": settings.smtp_host,
        "smtp_port": settings.smtp_port,
        "security": serialize_email_security(settings.security),
        "username": settings.username,
        "password_secret": settings.password,
        "from_address": settings.from_address,
        "recipients": settings.recipients,
        "subject_prefix": settings.subject_prefix,
    })
}

fn validate_email_settings(settings: &EmailSettingsSecret) -> Result<(), AppError> {
    if settings.smtp_port == 0 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "smtp_port must be greater than zero",
        ));
    }
    if !settings.enabled {
        return Ok(());
    }
    ensure_sendable(settings)
}

fn ensure_sendable(settings: &EmailSettingsSecret) -> Result<(), AppError> {
    if !settings.enabled {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "email notification is disabled",
        ));
    }
    if settings.smtp_host.trim().is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "smtp_host is required",
        ));
    }
    if settings.from_address.trim().is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "from_address is required",
        ));
    }
    if settings.recipients.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "at least one recipient is required",
        ));
    }
    Ok(())
}

async fn send_email(
    settings: &EmailSettingsSecret,
    recipient: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    let from: Mailbox = settings.from_address.parse()?;
    let to: Mailbox = recipient.parse()?;
    let message = Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .body(body.to_string())?;
    let mailer = smtp_transport(settings)?;
    mailer.send(message).await?;
    Ok(())
}

fn smtp_transport(
    settings: &EmailSettingsSecret,
) -> anyhow::Result<AsyncSmtpTransport<Tokio1Executor>> {
    let mut builder = match settings.security {
        EmailSecurityMode::None => {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&settings.smtp_host)
        }
        EmailSecurityMode::StartTls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.smtp_host)?
        }
        EmailSecurityMode::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.smtp_host)?,
    };
    builder = builder.port(settings.smtp_port);
    if let (Some(username), Some(password)) = (&settings.username, &settings.password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }
    Ok(builder.build())
}

fn format_alert_body(
    rule: &AlertRule,
    incident: &AlertIncident,
    recovered: bool,
    observed_value: f32,
) -> String {
    let state = if recovered { "已恢复" } else { "已触发" };
    format!(
        "Doro 告警{state}\n\n规则：{}\n级别：{:?}\n主机：{}\n指标：{} {}\n阈值：{}\n当前值：{}\n触发时间：{}\n事件 ID：{}\n",
        rule.name,
        rule.severity,
        incident.host_id,
        metric_label(&rule.metric),
        operator_label(rule.operator),
        rule.threshold,
        observed_value,
        incident.triggered_at,
        incident.id,
    )
}

fn metric_label(metric: &doro_protocol::AlertMetricSelector) -> String {
    match metric.source {
        AlertMetricSource::Core => metric.key.clone(),
        AlertMetricSource::Extra => format!("extra{}", metric.key),
    }
}

fn operator_label(operator: AlertOperator) -> &'static str {
    match operator {
        AlertOperator::GreaterThan => ">",
        AlertOperator::GreaterThanOrEqual => ">=",
        AlertOperator::LessThan => "<",
        AlertOperator::LessThanOrEqual => "<=",
        AlertOperator::Equal => "=",
        AlertOperator::NotEqual => "!=",
    }
}

fn normalize_recipients(recipients: Vec<String>) -> Vec<String> {
    recipients
        .into_iter()
        .map(|recipient| recipient.trim().to_string())
        .filter(|recipient| !recipient.is_empty())
        .fold(Vec::new(), |mut current, recipient| {
            if !current
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(&recipient))
            {
                current.push(recipient);
            }
            current
        })
}

fn parse_email_security(value: &str) -> EmailSecurityMode {
    match value {
        "tls" => EmailSecurityMode::Tls,
        "none" => EmailSecurityMode::None,
        _ => EmailSecurityMode::StartTls,
    }
}

fn serialize_email_security(value: EmailSecurityMode) -> &'static str {
    match value {
        EmailSecurityMode::StartTls => "start_tls",
        EmailSecurityMode::Tls => "tls",
        EmailSecurityMode::None => "none",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_notification_settings_default_to_enabled() {
        let settings = system_notification_settings_from_json(serde_json::json!({}));

        assert!(settings.enabled);
    }

    #[test]
    fn system_notification_settings_parse_disabled_state() {
        let settings = system_notification_settings_from_json(serde_json::json!({
            "enabled": false
        }));

        assert!(!settings.enabled);
    }
}
