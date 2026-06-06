use crate::notifications::{create_alert_system_notification, send_alert_email};
use crate::prelude::*;

const ALERT_STATE_INACTIVE: &str = "inactive";
const ALERT_STATE_PENDING: &str = "pending";
const ALERT_STATE_FIRING: &str = "firing";

pub(crate) async fn evaluate_metric_alerts(
    store: &Store,
    snapshot: &NewMetricSnapshot,
) -> Result<(), sea_orm::DbErr> {
    let rules = store
        .alerts()
        .enabled_rules_for_host(snapshot.host_id)
        .await?;
    for rule in rules {
        if let Err(error) = evaluate_metric_alert(store, &rule, snapshot).await {
            tracing::warn!(
                %error,
                rule_id = %rule.id,
                host_id = %snapshot.host_id,
                "failed to evaluate alert rule"
            );
        }
    }
    Ok(())
}

async fn evaluate_metric_alert(
    store: &Store,
    rule: &AlertRule,
    snapshot: &NewMetricSnapshot,
) -> Result<(), sea_orm::DbErr> {
    let observed_at = snapshot.captured_at;
    let observed_value = metric_value(rule, snapshot);
    let matched = observed_value
        .map(|value| compare_value(value, rule.operator, rule.threshold))
        .unwrap_or(false);
    let state = store.alerts().get_state(rule.id, snapshot.host_id).await?;

    if matched {
        let value = observed_value.unwrap_or_default();
        let first_matched_at = state
            .as_ref()
            .and_then(|state| state.first_matched_at)
            .unwrap_or(observed_at);
        let last_fired_at = state.as_ref().and_then(|state| state.last_fired_at);
        let is_firing = state
            .as_ref()
            .is_some_and(|state| state.state == ALERT_STATE_FIRING);
        let in_cooldown = last_fired_at.is_some_and(|fired_at| {
            let cooldown = ChronoDuration::seconds(rule.cooldown_seconds as i64);
            fired_at + cooldown > observed_at
        });
        let matured =
            first_matched_at + ChronoDuration::seconds(rule.for_seconds as i64) <= observed_at;

        if !is_firing && matured && !in_cooldown {
            fire_alert(store, rule, snapshot, value, first_matched_at, observed_at).await?;
        } else {
            let state_name = if is_firing {
                ALERT_STATE_FIRING
            } else {
                ALERT_STATE_PENDING
            };
            store
                .alerts()
                .upsert_state(
                    rule.id,
                    snapshot.host_id,
                    AlertRuleStateChanges {
                        state: state_name.to_string(),
                        first_matched_at: Some(first_matched_at),
                        last_matched_at: Some(observed_at),
                        last_fired_at,
                        active_incident_id: state.and_then(|state| state.active_incident_id),
                        last_resolved_at: None,
                        updated_at: observed_at,
                    },
                )
                .await?;
        }
        return Ok(());
    }

    if let Some(current_state) = state.as_ref()
        && current_state.state == ALERT_STATE_FIRING
    {
        recover_alert(
            store,
            rule,
            snapshot,
            current_state,
            observed_value.unwrap_or_default(),
        )
        .await?;
        return Ok(());
    }

    store
        .alerts()
        .upsert_state(
            rule.id,
            snapshot.host_id,
            AlertRuleStateChanges {
                state: ALERT_STATE_INACTIVE.to_string(),
                first_matched_at: None,
                last_matched_at: None,
                last_fired_at: state.and_then(|state| state.last_fired_at),
                active_incident_id: None,
                last_resolved_at: None,
                updated_at: observed_at,
            },
        )
        .await?;
    Ok(())
}

async fn fire_alert(
    store: &Store,
    rule: &AlertRule,
    snapshot: &NewMetricSnapshot,
    observed_value: f32,
    first_matched_at: DateTime<Utc>,
    observed_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    let incident = store
        .alerts()
        .create_incident(NewAlertIncident {
            id: Uuid::new_v4(),
            alert_rule_id: rule.id,
            host_id: snapshot.host_id,
            rule_name: rule.name.clone(),
            severity: rule.severity,
            metric: rule.metric.clone(),
            operator: rule.operator,
            threshold: rule.threshold,
            observed_value,
            status: AlertIncidentStatus::Firing,
            triggered_at: observed_at,
            last_observed_at: observed_at,
        })
        .await?;
    store
        .alerts()
        .upsert_state(
            rule.id,
            snapshot.host_id,
            AlertRuleStateChanges {
                state: ALERT_STATE_FIRING.to_string(),
                first_matched_at: Some(first_matched_at),
                last_matched_at: Some(observed_at),
                last_fired_at: Some(observed_at),
                active_incident_id: Some(incident.id),
                last_resolved_at: None,
                updated_at: observed_at,
            },
        )
        .await?;
    if let Err(error) = send_alert_email(store, rule, &incident, false, observed_value).await {
        tracing::warn!(
            ?error,
            rule_id = %rule.id,
            incident_id = %incident.id,
            "failed to send alert notification"
        );
    }
    if let Err(error) =
        create_alert_system_notification(store, rule, &incident, false, observed_value).await
    {
        tracing::warn!(
            ?error,
            rule_id = %rule.id,
            incident_id = %incident.id,
            "failed to create alert system notification"
        );
    }
    Ok(())
}

async fn recover_alert(
    store: &Store,
    rule: &AlertRule,
    snapshot: &NewMetricSnapshot,
    state: &doro_store::StoredAlertRuleState,
    observed_value: f32,
) -> Result<(), sea_orm::DbErr> {
    let observed_at = snapshot.captured_at;
    if let Some(incident_id) = state.active_incident_id
        && let Some(incident) = store
            .alerts()
            .resolve_incident(incident_id, observed_value, observed_at)
            .await?
    {
        if let Err(error) = send_alert_email(store, rule, &incident, true, observed_value).await {
            tracing::warn!(
                ?error,
                rule_id = %rule.id,
                incident_id = %incident.id,
                "failed to send alert recovery notification"
            );
        }
        if let Err(error) =
            create_alert_system_notification(store, rule, &incident, true, observed_value).await
        {
            tracing::warn!(
                ?error,
                rule_id = %rule.id,
                incident_id = %incident.id,
                "failed to create alert recovery system notification"
            );
        }
    }
    store
        .alerts()
        .upsert_state(
            rule.id,
            snapshot.host_id,
            AlertRuleStateChanges {
                state: ALERT_STATE_INACTIVE.to_string(),
                first_matched_at: None,
                last_matched_at: None,
                last_fired_at: state.last_fired_at,
                active_incident_id: None,
                last_resolved_at: Some(observed_at),
                updated_at: observed_at,
            },
        )
        .await?;
    Ok(())
}

fn metric_value(rule: &AlertRule, snapshot: &NewMetricSnapshot) -> Option<f32> {
    match rule.metric.source {
        AlertMetricSource::Core => match rule.metric.key.as_str() {
            "cpu_percent" => Some(snapshot.cpu_percent),
            "memory_percent" => Some(snapshot.memory_percent),
            "disk_percent" => Some(snapshot.disk_percent),
            "load_average" => Some(snapshot.load_average),
            _ => None,
        },
        AlertMetricSource::Extra => snapshot
            .extra
            .pointer(&rule.metric.key)
            .and_then(json_number_as_f32),
    }
}

fn json_number_as_f32(value: &Value) -> Option<f32> {
    value.as_f64().map(|value| value as f32)
}

fn compare_value(value: f32, operator: AlertOperator, threshold: f32) -> bool {
    match operator {
        AlertOperator::GreaterThan => value > threshold,
        AlertOperator::GreaterThanOrEqual => value >= threshold,
        AlertOperator::LessThan => value < threshold,
        AlertOperator::LessThanOrEqual => value <= threshold,
        AlertOperator::Equal => (value - threshold).abs() <= f32::EPSILON,
        AlertOperator::NotEqual => (value - threshold).abs() > f32::EPSILON,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doro_protocol::AlertSeverity;

    #[test]
    fn extra_metric_value_uses_json_pointer() {
        let rule = AlertRule {
            id: Uuid::new_v4(),
            name: "gpu".to_string(),
            description: None,
            severity: AlertSeverity::Warning,
            metric: doro_protocol::AlertMetricSelector {
                source: AlertMetricSource::Extra,
                key: "/gpus/0/utilization_percent".to_string(),
            },
            operator: AlertOperator::GreaterThan,
            threshold: 90.0,
            host_id: None,
            enabled: true,
            for_seconds: 60,
            cooldown_seconds: 600,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let snapshot = NewMetricSnapshot {
            host_id: Uuid::new_v4(),
            captured_at: Utc::now(),
            cpu_percent: 0.0,
            memory_percent: 0.0,
            disk_percent: 0.0,
            load_average: 0.0,
            extra: serde_json::json!({
                "gpus": [
                    { "utilization_percent": 93.5 }
                ]
            }),
        };

        assert_eq!(metric_value(&rule, &snapshot), Some(93.5));
    }

    #[test]
    fn compare_value_supports_threshold_operators() {
        assert!(compare_value(91.0, AlertOperator::GreaterThan, 90.0));
        assert!(compare_value(90.0, AlertOperator::GreaterThanOrEqual, 90.0));
        assert!(compare_value(79.0, AlertOperator::LessThan, 80.0));
        assert!(compare_value(80.0, AlertOperator::LessThanOrEqual, 80.0));
        assert!(compare_value(80.0, AlertOperator::Equal, 80.0));
        assert!(compare_value(81.0, AlertOperator::NotEqual, 80.0));
    }
}
