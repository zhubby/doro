use crate::prelude::*;

pub(crate) fn grpc_capability_to_protocol(
    capability: grpc::AgentCapability,
) -> Option<AgentCapability> {
    doro_store::parse_agent_capability(&capability.name, &capability.risk, capability.description)
}

pub(crate) fn timestamp_to_utc(timestamp: &Timestamp) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(timestamp.seconds, timestamp.nanos as u32)
        .single()
}

pub(crate) fn parse_optional_uuid(value: &str) -> Option<Uuid> {
    if value.trim().is_empty() {
        return None;
    }
    doro_store::parse_uuid(value).ok()
}

pub(crate) fn parse_event_payload(payload_json: &str) -> Value {
    if payload_json.trim().is_empty() {
        return serde_json::json!({});
    }
    serde_json::from_str(payload_json).unwrap_or_else(|_| {
        serde_json::json!({
            "raw": payload_json
        })
    })
}

pub(crate) fn typed_agent_event_payload(event: &grpc::AgentEvent) -> Option<(String, Value)> {
    match event.event.as_ref()? {
        grpc::agent_event::Event::Connected(connected) => Some((
            "connected".to_string(),
            serde_json::json!({
                "protocol_version": connected.protocol_version,
                "hostname": connected.hostname
            }),
        )),
        grpc::agent_event::Event::Heartbeat(heartbeat) => Some((
            "heartbeat".to_string(),
            serde_json::json!({
                "protocol_version": heartbeat.protocol_version
            }),
        )),
        grpc::agent_event::Event::MetricsSnapshot(snapshot) => Some((
            "metrics.snapshot".to_string(),
            serde_json::json!({
                "host_id": snapshot.host_id,
                "captured_at": snapshot.captured_at.as_ref().and_then(timestamp_to_utc),
                "cpu_percent": snapshot.cpu_percent,
                "memory_percent": snapshot.memory_percent,
                "disk_percent": snapshot.disk_percent,
                "load_average": snapshot.load_average,
                "extra": parse_event_payload(&snapshot.extra_json),
            }),
        )),
        grpc::agent_event::Event::ContainerSnapshot(snapshot) => Some((
            "container.snapshot".to_string(),
            container_snapshot_payload(snapshot),
        )),
        grpc::agent_event::Event::CollectorError(error) => Some((
            "metrics.collector_error".to_string(),
            serde_json::json!({
                "command_id": error.command_id,
                "collector": error.collector,
                "message": error.message,
            }),
        )),
        grpc::agent_event::Event::CommandResult(result) => Some((
            "command.result".to_string(),
            serde_json::json!({
                "command_id": result.command_id,
                "status": result.status,
                "message": result.message,
            }),
        )),
        grpc::agent_event::Event::TerminalCommandResult(result) => Some((
            "terminal.command_result".to_string(),
            serde_json::json!({
                "command_id": result.command_id,
                "status": result.status,
                "output": result.output,
                "exit_code": result.exit_code,
                "started_at": result.started_at.as_ref().and_then(timestamp_to_utc),
                "finished_at": result.finished_at.as_ref().and_then(timestamp_to_utc),
            }),
        )),
        grpc::agent_event::Event::TerminalOutput(output) => Some((
            "terminal.output".to_string(),
            serde_json::json!({
                "session_id": output.session_id,
                "bytes": output.data.len(),
            }),
        )),
        grpc::agent_event::Event::TerminalSessionClosed(closed) => Some((
            "terminal.session_closed".to_string(),
            serde_json::json!({
                "session_id": closed.session_id,
                "reason": closed.reason,
            }),
        )),
        grpc::agent_event::Event::LogLine(line) => Some((
            "log.line".to_string(),
            serde_json::json!({
                "log_id": line.log_id,
                "level": line.level,
                "target": line.target,
                "message": line.message,
                "fields": parse_event_payload(&line.fields_json),
            }),
        )),
        grpc::agent_event::Event::VirtualMachineSnapshot(snapshot) => Some((
            "virtual_machine.snapshot".to_string(),
            virtual_machine_snapshot_payload(snapshot),
        )),
        grpc::agent_event::Event::VirtualMachineCommandResult(result) => Some((
            "virtual_machine.command_result".to_string(),
            serde_json::json!({
                "command_id": result.command_id,
                "status": result.status,
                "message": result.message,
                "details": parse_event_payload(&result.details_json),
            }),
        )),
        grpc::agent_event::Event::FileCommandResult(result) => Some((
            "file.command_result".to_string(),
            serde_json::json!({
                "command_id": result.command_id,
                "status": result.status,
                "message": result.message,
                "result": parse_event_payload(&result.result_json),
                "content_bytes": result.content.len(),
            }),
        )),
        grpc::agent_event::Event::AgentTaskProgress(progress) => Some((
            "agent_task.progress".to_string(),
            serde_json::json!({
                "command_id": progress.command_id,
                "task_id": progress.task_id,
                "step_id": progress.step_id,
                "status": progress.status,
                "message": progress.message,
                "details": parse_event_payload(&progress.details_json),
            }),
        )),
        grpc::agent_event::Event::AgentTaskResult(result) => Some((
            "agent_task.result".to_string(),
            serde_json::json!({
                "command_id": result.command_id,
                "task_id": result.task_id,
                "status": result.status,
                "summary": result.summary,
                "result": parse_event_payload(&result.result_json),
            }),
        )),
        grpc::agent_event::Event::AgentToolApprovalRequest(request) => Some((
            "agent_tool.approval_requested".to_string(),
            serde_json::json!({
                "request_id": request.request_id,
                "command_id": request.command_id,
                "task_id": request.task_id,
                "tool_call_id": request.tool_call_id,
                "tool_name": request.tool_name,
                "risk": request.risk,
                "summary": request.summary,
                "arguments": parse_event_payload(&request.arguments_json),
            }),
        )),
    }
}

pub(crate) fn runtime_log_from_agent_line(
    line: grpc::LogLineEvent,
    agent_id: Uuid,
    host_id: Uuid,
    recorded_at: DateTime<Utc>,
) -> RuntimeLogEntry {
    RuntimeLogEntry {
        id: line.log_id.parse().unwrap_or_else(|_| Uuid::new_v4()),
        source: "agent".to_string(),
        host_id: Some(host_id),
        agent_id: Some(agent_id),
        level: line.level,
        target: line.target,
        message: line.message,
        fields: parse_event_payload(&line.fields_json),
        recorded_at,
    }
}

pub(crate) fn container_snapshot_payload(snapshot: &grpc::ContainerSnapshotEvent) -> Value {
    serde_json::json!({
        "command_id": snapshot.command_id,
        "runtime": snapshot.runtime,
        "containers": snapshot.containers.iter().map(container_observation_payload).collect::<Vec<_>>(),
        "extra": parse_event_payload(&snapshot.extra_json),
    })
}

pub(crate) fn container_observation_payload(container: &grpc::ContainerObservation) -> Value {
    serde_json::json!({
        "id": container.id,
        "names": container.names,
        "image": container.image,
        "image_id": container.image_id,
        "command": container.command,
        "created": container.created,
        "ports": parse_event_payload(&container.ports_json),
        "labels": parse_event_payload(&container.labels_json),
        "state": container.state,
        "status": container.status,
    })
}

pub(crate) fn virtual_machine_snapshot_payload(
    snapshot: &grpc::VirtualMachineSnapshotEvent,
) -> Value {
    serde_json::json!({
        "command_id": snapshot.command_id,
        "provider": snapshot.provider,
        "virtual_machines": snapshot.virtual_machines.iter().map(virtual_machine_observation_payload).collect::<Vec<_>>(),
        "extra": parse_event_payload(&snapshot.extra_json),
    })
}

pub(crate) fn virtual_machine_observation_payload(vm: &grpc::VirtualMachineObservation) -> Value {
    serde_json::json!({
        "vm_ref": vm.vm_ref,
        "name": vm.name,
        "status": vm.status,
        "cpu_cores": vm.cpu_cores,
        "memory_mib": vm.memory_mib,
        "disk_gb": vm.disk_gb,
        "image": vm.image,
        "networks": parse_event_payload(&vm.networks_json),
        "console": parse_event_payload(&vm.console_json),
        "metadata": parse_event_payload(&vm.metadata_json),
        "created_at": vm.created_at.as_ref().and_then(timestamp_to_utc),
        "observed_at": vm.observed_at.as_ref().and_then(timestamp_to_utc),
    })
}

pub(crate) async fn ingest_agent_event(
    store: &Store,
    host_id: Option<Uuid>,
    event_type: &str,
    payload: &Value,
    recorded_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    match event_type {
        "metrics.snapshot" => {
            if let Some(snapshot) = metric_snapshot_from_payload(host_id, payload, recorded_at) {
                store.metrics().record(snapshot).await?;
            }
        }
        "container.snapshot" => {
            if let Some(host_id) = host_id {
                let containers = container_observations_from_payload(host_id, payload, recorded_at);
                store.containers().upsert_many(containers).await?;
            }
        }
        "virtual_machine.snapshot" => {
            if let Some(host_id) = host_id {
                let vms = virtual_machine_observations_from_payload(host_id, payload, recorded_at);
                store.virtual_machines().upsert_many(vms).await?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn virtual_machine_observations_from_payload(
    host_id: Uuid,
    payload: &Value,
    recorded_at: DateTime<Utc>,
) -> Vec<NewVirtualMachineObservation> {
    let provider = payload
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("qemu");
    payload
        .get("virtual_machines")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|vm| virtual_machine_observation(host_id, provider, vm, recorded_at))
        .collect()
}

pub(crate) fn virtual_machine_observation(
    host_id: Uuid,
    provider: &str,
    vm: &Value,
    recorded_at: DateTime<Utc>,
) -> Option<NewVirtualMachineObservation> {
    let vm_ref = vm.get("vm_ref").and_then(Value::as_str)?.to_string();
    let status = match vm
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
    {
        "stopped" => VirtualMachineStatus::Stopped,
        "starting" => VirtualMachineStatus::Starting,
        "running" => VirtualMachineStatus::Running,
        "paused" => VirtualMachineStatus::Paused,
        "stopping" => VirtualMachineStatus::Stopping,
        "failed" => VirtualMachineStatus::Failed,
        _ => VirtualMachineStatus::Unknown,
    };
    Some(NewVirtualMachineObservation {
        host_id,
        provider: provider.to_string(),
        vm_ref,
        name: vm
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unnamed-vm")
            .to_string(),
        status,
        image: vm
            .get("image")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        cpu_cores: vm
            .get("cpu_cores")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(u16::MAX as u64) as u16,
        memory_mib: vm
            .get("memory_mib")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(u32::MAX as u64) as u32,
        disk_gb: vm
            .get("disk_gb")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(u32::MAX as u64) as u32,
        networks: vm
            .get("networks")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default(),
        console: vm.get("console").cloned().filter(|value| !value.is_null()),
        metadata: vm
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        created_at: vm
            .get("created_at")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc)),
        observed_at: vm
            .get("observed_at")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or(recorded_at),
    })
}

pub(crate) fn metric_snapshot_from_payload(
    fallback_host_id: Option<Uuid>,
    payload: &Value,
    recorded_at: DateTime<Utc>,
) -> Option<NewMetricSnapshot> {
    let host_id = payload
        .get("host_id")
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .or(fallback_host_id)?;
    Some(NewMetricSnapshot {
        host_id,
        captured_at: payload
            .get("captured_at")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or(recorded_at),
        cpu_percent: json_f32(payload, "cpu_percent")?,
        memory_percent: json_f32(payload, "memory_percent")?,
        disk_percent: json_f32(payload, "disk_percent")?,
        load_average: json_f32(payload, "load_average")?,
        extra: payload
            .get("extra")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    })
}

pub(crate) fn container_observations_from_payload(
    host_id: Uuid,
    payload: &Value,
    recorded_at: DateTime<Utc>,
) -> Vec<NewContainerObservation> {
    let runtime = payload
        .get("runtime")
        .and_then(Value::as_str)
        .unwrap_or("docker");
    payload
        .get("containers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|container| container_observation(host_id, runtime, container, recorded_at))
        .collect()
}

pub(crate) fn container_observation(
    host_id: Uuid,
    runtime: &str,
    container: &Value,
    recorded_at: DateTime<Utc>,
) -> Option<NewContainerObservation> {
    let container_ref = container.get("id").and_then(Value::as_str)?.to_string();
    let name = container
        .get("names")
        .and_then(Value::as_array)
        .and_then(|names| names.first())
        .and_then(Value::as_str)
        .map(|name| name.trim_start_matches('/').to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| container_ref.chars().take(12).collect());
    let image = container
        .get("image")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let status = container
        .get("state")
        .and_then(Value::as_str)
        .or_else(|| container.get("status").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    Some(NewContainerObservation {
        host_id,
        runtime: runtime.to_string(),
        container_ref,
        name,
        image,
        status,
        ports: container
            .get("ports")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
        labels: container
            .get("labels")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        created_at: container
            .get("created")
            .and_then(Value::as_i64)
            .and_then(|seconds| Utc.timestamp_opt(seconds, 0).single()),
        observed_at: recorded_at,
    })
}

pub(crate) fn json_f32(payload: &Value, key: &str) -> Option<f32> {
    payload.get(key)?.as_f64().map(|value| value as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_event_payload_json() {
        assert_eq!(
            parse_event_payload(r#"{"ok":true}"#),
            serde_json::json!({"ok": true})
        );
        assert_eq!(
            parse_event_payload("not json"),
            serde_json::json!({"raw": "not json"})
        );
    }

    #[test]
    fn metrics_snapshot_payload_maps_to_store_model() {
        let host_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "host_id": host_id,
            "captured_at": "2026-05-27T00:00:00Z",
            "cpu_percent": 12.5,
            "memory_percent": 34.5,
            "disk_percent": 56.5,
            "load_average": 1.5,
            "extra": {"networks": []}
        });

        let snapshot = match metric_snapshot_from_payload(None, &payload, Utc::now()) {
            Some(snapshot) => snapshot,
            None => panic!("valid metric payload should parse"),
        };

        assert_eq!(snapshot.host_id, host_id);
        assert_eq!(snapshot.cpu_percent, 12.5);
        assert_eq!(snapshot.extra, serde_json::json!({"networks": []}));
    }

    #[test]
    fn malformed_metrics_snapshot_payload_is_ignored() {
        let payload = serde_json::json!({
            "cpu_percent": "not-a-number",
            "memory_percent": 34.5,
            "disk_percent": 56.5,
            "load_average": 1.5
        });

        assert!(metric_snapshot_from_payload(Some(Uuid::new_v4()), &payload, Utc::now()).is_none());
    }

    #[test]
    fn container_snapshot_payload_maps_to_observations() {
        let host_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "runtime": "docker",
            "containers": [{
                "id": "abc123",
                "names": ["/postgres"],
                "image": "postgres:16",
                "state": "running",
                "ports": [],
                "labels": {"app": "db"}
            }]
        });

        let observations = container_observations_from_payload(host_id, &payload, Utc::now());

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].container_ref, "abc123");
        assert_eq!(observations[0].name, "postgres");
        assert_eq!(observations[0].runtime, "docker");
    }
}
