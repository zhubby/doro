use super::ContainerDetail;
use super::ContainerDevice;
use super::ContainerHealthcheck;
use super::ContainerListFilter;
use super::ContainerOperationResult;
use super::ContainerPortBinding;
use super::ContainerProviderError;
use super::ContainerRestartPolicyName;
use super::ContainerSummary;
use super::CreateContainerRequest;
use super::DockerProvider;
use super::RemoveContainerRequest;
use super::RestartContainerRequest;
use super::StopContainerRequest;
use bollard::container::Config;
use bollard::container::CreateContainerOptions;
use bollard::container::ListContainersOptions;
use bollard::container::NetworkingConfig;
use bollard::container::RemoveContainerOptions;
use bollard::container::RestartContainerOptions;
use bollard::container::StopContainerOptions;
use bollard::models::DeviceMapping;
use bollard::models::EndpointSettings;
use bollard::models::HealthConfig;
use bollard::models::HostConfig;
use bollard::models::HostConfigLogConfig;
use bollard::models::PortBinding;
use bollard::models::RestartPolicy;
use bollard::models::RestartPolicyNameEnum;
use serde_json::json;
use std::collections::HashMap;

impl DockerProvider {
    pub async fn containers(
        &self,
        filter: ContainerListFilter,
    ) -> Result<Vec<ContainerSummary>, ContainerProviderError> {
        let containers = self
            .docker()
            .list_containers::<String>(Some(ListContainersOptions {
                all: filter.all,
                ..Default::default()
            }))
            .await?;
        Ok(containers
            .into_iter()
            .map(|container| ContainerSummary {
                id: container.id,
                names: container.names.unwrap_or_default(),
                image: container.image,
                image_id: container.image_id,
                command: container.command,
                created: container.created,
                ports: json!(container.ports.unwrap_or_default()),
                labels: json!(container.labels.unwrap_or_default()),
                state: container.state,
                status: container.status,
            })
            .collect())
    }

    pub async fn inspect_container(
        &self,
        id_or_name: &str,
    ) -> Result<ContainerDetail, ContainerProviderError> {
        require_identifier(id_or_name, "container id or name")?;
        let container = self.docker().inspect_container(id_or_name, None).await?;
        Ok(ContainerDetail {
            id: container.id,
            name: container.name,
            image: container.image,
            state: json!(container.state),
            config: json!(container.config),
            host_config: json!(container.host_config),
            network_settings: json!(container.network_settings),
        })
    }

    pub async fn create_container(
        &self,
        request: CreateContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        require_identifier(&request.name, "container name")?;
        require_identifier(&request.image, "container image")?;
        let (options, config) = build_create_container_config(&request)?;
        let response = self
            .docker()
            .create_container(Some(options), config)
            .await?;
        Ok(ContainerOperationResult {
            id: Some(response.id),
            name: Some(request.name),
            action: "create".to_string(),
            details: json!({ "warnings": response.warnings }),
        })
    }

    pub async fn start_container(
        &self,
        id_or_name: &str,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        require_identifier(id_or_name, "container id or name")?;
        self.docker()
            .start_container::<String>(id_or_name, None)
            .await?;
        Ok(simple_result(id_or_name, "start"))
    }

    pub async fn stop_container(
        &self,
        request: StopContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        require_identifier(&request.id_or_name, "container id or name")?;
        self.docker()
            .stop_container(
                &request.id_or_name,
                Some(StopContainerOptions {
                    t: request.timeout_seconds,
                }),
            )
            .await?;
        Ok(simple_result(&request.id_or_name, "stop"))
    }

    pub async fn restart_container(
        &self,
        request: RestartContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        require_identifier(&request.id_or_name, "container id or name")?;
        self.docker()
            .restart_container(
                &request.id_or_name,
                Some(RestartContainerOptions {
                    t: request.timeout_seconds as isize,
                }),
            )
            .await?;
        Ok(simple_result(&request.id_or_name, "restart"))
    }

    pub async fn remove_container(
        &self,
        request: RemoveContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        require_identifier(&request.id_or_name, "container id or name")?;
        self.docker()
            .remove_container(
                &request.id_or_name,
                Some(RemoveContainerOptions {
                    v: request.remove_volumes,
                    force: request.force,
                    link: false,
                }),
            )
            .await?;
        Ok(simple_result(&request.id_or_name, "remove"))
    }
}

fn optional_vec(values: Vec<String>) -> Option<Vec<String>> {
    let values = clean_vec(values);
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn optional_map(
    values: std::collections::HashMap<String, String>,
) -> Option<std::collections::HashMap<String, String>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn build_create_container_config(
    request: &CreateContainerRequest,
) -> Result<(CreateContainerOptions<String>, Config<String>), ContainerProviderError> {
    let name = required_text(&request.name, "container name")?;
    let image = required_text(&request.image, "container image")?;
    let ports = build_ports(&request.ports)?;
    let volumes = build_volumes(&request.volumes)?;
    let host_config = build_host_config(request, ports.port_bindings)?;
    let networking_config = build_networking_config(request)?;

    Ok((
        CreateContainerOptions {
            name,
            platform: optional_text(request.platform.clone()),
        },
        Config {
            hostname: optional_text(request.hostname.clone()),
            domainname: optional_text(request.domainname.clone()),
            user: optional_text(request.user.clone()),
            exposed_ports: ports.exposed_ports,
            tty: true_option(request.tty),
            open_stdin: true_option(request.open_stdin),
            env: optional_vec(request.env.clone()),
            cmd: optional_vec(request.command.clone()),
            healthcheck: build_healthcheck(request.healthcheck.clone())?,
            image: Some(image),
            volumes,
            working_dir: optional_text(request.working_dir.clone()),
            entrypoint: optional_vec(request.entrypoint.clone()),
            mac_address: optional_text(request.mac_address.clone()),
            labels: optional_map(request.labels.clone()),
            host_config: Some(host_config),
            networking_config,
            ..Default::default()
        },
    ))
}

#[derive(Debug)]
struct BuiltPorts {
    exposed_ports: Option<HashMap<String, HashMap<(), ()>>>,
    port_bindings: Option<HashMap<String, Option<Vec<PortBinding>>>>,
}

fn build_ports(ports: &[ContainerPortBinding]) -> Result<BuiltPorts, ContainerProviderError> {
    let mut exposed_ports = HashMap::new();
    let mut port_bindings: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();

    for port in ports {
        let (container_port, inline_protocol) = split_port_protocol(&port.container_port)?;
        let protocol = optional_text(port.protocol.clone())
            .or(inline_protocol)
            .unwrap_or_else(|| "tcp".to_string())
            .to_ascii_lowercase();
        if !matches!(protocol.as_str(), "tcp" | "udp" | "sctp") {
            return Err(invalid(format!(
                "unsupported port protocol `{protocol}`; expected tcp, udp, or sctp"
            )));
        }
        let key = format!("{container_port}/{protocol}");
        exposed_ports.insert(key.clone(), HashMap::new());
        let host_port = optional_text(port.host_port.clone());
        if let Some(value) = host_port.as_deref() {
            validate_port(value, "host port")?;
        }
        let binding = PortBinding {
            host_ip: optional_text(port.host_ip.clone()),
            host_port,
        };
        port_bindings
            .entry(key)
            .or_insert_with(|| Some(Vec::new()))
            .get_or_insert_with(Vec::new)
            .push(binding);
    }

    Ok(BuiltPorts {
        exposed_ports: (!exposed_ports.is_empty()).then_some(exposed_ports),
        port_bindings: (!port_bindings.is_empty()).then_some(port_bindings),
    })
}

fn build_host_config(
    request: &CreateContainerRequest,
    port_bindings: Option<HashMap<String, Option<Vec<PortBinding>>>>,
) -> Result<HostConfig, ContainerProviderError> {
    let memory = parse_optional_size(request.memory.as_deref(), "memory")?;
    let memory_swap = parse_optional_size(request.memory_swap.as_deref(), "memory swap")?;
    let shm_size = parse_optional_size(request.shm_size.as_deref(), "shm size")?;
    let nano_cpus = parse_optional_cpus(request.cpus.as_deref())?;
    let devices = build_devices(&request.devices)?;
    let tmpfs = build_tmpfs(&request.tmpfs)?;
    let restart_policy = build_restart_policy(request.restart_policy, request.restart_max_retries)?;
    let log_config = build_log_config(request.log_driver.clone(), request.log_options.clone());
    let network_mode = optional_text(request.network_mode.clone())
        .or_else(|| optional_text(request.network_name.clone()));

    Ok(HostConfig {
        memory,
        memory_swap,
        nano_cpus,
        cpu_shares: positive_option(request.cpu_shares, "cpu shares")?,
        cpuset_cpus: optional_text(request.cpuset_cpus.clone()),
        devices,
        pids_limit: request.pids_limit,
        init: true_option(request.init),
        binds: build_binds(&request.binds)?,
        log_config,
        network_mode,
        port_bindings,
        restart_policy,
        auto_remove: true_option(request.auto_remove),
        cap_add: optional_vec(request.cap_add.clone()),
        cap_drop: optional_vec(request.cap_drop.clone()),
        dns: optional_vec(request.dns.clone()),
        dns_search: optional_vec(request.dns_search.clone()),
        extra_hosts: build_extra_hosts(&request.extra_hosts)?,
        privileged: true_option(request.privileged),
        readonly_rootfs: true_option(request.read_only_rootfs),
        tmpfs,
        shm_size,
        ..Default::default()
    })
}

fn build_networking_config(
    request: &CreateContainerRequest,
) -> Result<Option<NetworkingConfig<String>>, ContainerProviderError> {
    let Some(network_name) = optional_text(request.network_name.clone()) else {
        if !request.aliases.is_empty() || request.ipv4_address.is_some() {
            return Err(invalid(
                "network name is required when aliases or IPv4 address are configured",
            ));
        }
        return Ok(None);
    };

    let mut endpoints_config = HashMap::new();
    endpoints_config.insert(
        network_name,
        EndpointSettings {
            aliases: optional_vec(request.aliases.clone()),
            ip_address: optional_text(request.ipv4_address.clone()),
            mac_address: optional_text(request.mac_address.clone()),
            ..Default::default()
        },
    );
    Ok(Some(NetworkingConfig { endpoints_config }))
}

fn build_volumes(
    values: &[String],
) -> Result<Option<HashMap<String, HashMap<(), ()>>>, ContainerProviderError> {
    let mut volumes = HashMap::new();
    for value in clean_vec(values.to_vec()) {
        if !value.starts_with('/') {
            return Err(invalid(format!(
                "volume path `{value}` must be an absolute container path"
            )));
        }
        volumes.insert(value, HashMap::new());
    }
    Ok((!volumes.is_empty()).then_some(volumes))
}

fn build_binds(values: &[String]) -> Result<Option<Vec<String>>, ContainerProviderError> {
    let values = clean_vec(values.to_vec());
    for value in &values {
        let mut parts = value.split(':');
        let source = parts.next().unwrap_or_default();
        let target = parts.next().unwrap_or_default();
        if source.trim().is_empty() || !target.starts_with('/') {
            return Err(invalid(format!(
                "bind `{value}` must use source:/absolute/container/path[:options]"
            )));
        }
    }
    Ok((!values.is_empty()).then_some(values))
}

fn build_tmpfs(
    values: &[String],
) -> Result<Option<HashMap<String, String>>, ContainerProviderError> {
    let mut tmpfs = HashMap::new();
    for value in clean_vec(values.to_vec()) {
        let (path, options) = value
            .split_once(':')
            .map(|(path, options)| (path.trim(), options.trim()))
            .unwrap_or((value.trim(), ""));
        if !path.starts_with('/') {
            return Err(invalid(format!(
                "tmpfs path `{path}` must be an absolute container path"
            )));
        }
        tmpfs.insert(path.to_string(), options.to_string());
    }
    Ok((!tmpfs.is_empty()).then_some(tmpfs))
}

fn build_extra_hosts(values: &[String]) -> Result<Option<Vec<String>>, ContainerProviderError> {
    let values = clean_vec(values.to_vec());
    for value in &values {
        let Some((name, address)) = value.split_once(':') else {
            return Err(invalid(format!(
                "extra host `{value}` must use hostname:address"
            )));
        };
        if name.trim().is_empty() || address.trim().is_empty() {
            return Err(invalid(format!(
                "extra host `{value}` must use hostname:address"
            )));
        }
    }
    Ok((!values.is_empty()).then_some(values))
}

fn build_devices(
    values: &[ContainerDevice],
) -> Result<Option<Vec<DeviceMapping>>, ContainerProviderError> {
    let mut devices = Vec::new();
    for value in values {
        let host_path = required_text(&value.host_path, "device host path")?;
        if !host_path.starts_with('/') {
            return Err(invalid(format!(
                "device host path `{host_path}` must be absolute"
            )));
        }
        let container_path =
            optional_text(value.container_path.clone()).unwrap_or_else(|| host_path.clone());
        if !container_path.starts_with('/') {
            return Err(invalid(format!(
                "device container path `{container_path}` must be absolute"
            )));
        }
        devices.push(DeviceMapping {
            path_on_host: Some(host_path),
            path_in_container: Some(container_path),
            cgroup_permissions: optional_text(value.permissions.clone())
                .or_else(|| Some("rwm".to_string())),
        });
    }
    Ok((!devices.is_empty()).then_some(devices))
}

fn build_restart_policy(
    policy: Option<ContainerRestartPolicyName>,
    max_retries: Option<i64>,
) -> Result<Option<RestartPolicy>, ContainerProviderError> {
    let Some(policy) = policy else {
        return Ok(None);
    };
    if let Some(value) = max_retries
        && value < 0
    {
        return Err(invalid("restart max retries cannot be negative"));
    }
    if max_retries.is_some() && policy != ContainerRestartPolicyName::OnFailure {
        return Err(invalid(
            "restart max retries can only be used with on-failure policy",
        ));
    }
    let name = match policy {
        ContainerRestartPolicyName::No => RestartPolicyNameEnum::NO,
        ContainerRestartPolicyName::Always => RestartPolicyNameEnum::ALWAYS,
        ContainerRestartPolicyName::UnlessStopped => RestartPolicyNameEnum::UNLESS_STOPPED,
        ContainerRestartPolicyName::OnFailure => RestartPolicyNameEnum::ON_FAILURE,
    };
    Ok(Some(RestartPolicy {
        name: Some(name),
        maximum_retry_count: max_retries,
    }))
}

fn build_log_config(
    driver: Option<String>,
    options: HashMap<String, String>,
) -> Option<HostConfigLogConfig> {
    let driver = optional_text(driver);
    if driver.is_none() && options.is_empty() {
        return None;
    }
    Some(HostConfigLogConfig {
        typ: driver,
        config: (!options.is_empty()).then_some(options),
    })
}

fn build_healthcheck(
    healthcheck: Option<ContainerHealthcheck>,
) -> Result<Option<HealthConfig>, ContainerProviderError> {
    let Some(healthcheck) = healthcheck else {
        return Ok(None);
    };
    if let Some(retries) = healthcheck.retries
        && retries < 0
    {
        return Err(invalid("healthcheck retries cannot be negative"));
    }
    let test = if healthcheck.disabled {
        Some(vec!["NONE".to_string()])
    } else {
        optional_text(healthcheck.command).map(|command| vec!["CMD-SHELL".to_string(), command])
    };
    Ok(Some(HealthConfig {
        test,
        interval: duration_seconds_to_nanos(healthcheck.interval_seconds, "healthcheck interval")?,
        timeout: duration_seconds_to_nanos(healthcheck.timeout_seconds, "healthcheck timeout")?,
        retries: healthcheck.retries,
        start_period: duration_seconds_to_nanos(
            healthcheck.start_period_seconds,
            "healthcheck start period",
        )?,
        start_interval: duration_seconds_to_nanos(
            healthcheck.start_interval_seconds,
            "healthcheck start interval",
        )?,
    }))
}

fn split_port_protocol(value: &str) -> Result<(String, Option<String>), ContainerProviderError> {
    let value = required_text(value, "container port")?;
    let (port, protocol) = value
        .split_once('/')
        .map(|(port, protocol)| (port.trim(), Some(protocol.trim().to_string())))
        .unwrap_or((value.trim(), None));
    validate_port(port, "container port")?;
    Ok((port.to_string(), protocol.filter(|value| !value.is_empty())))
}

fn validate_port(value: &str, field: &str) -> Result<(), ContainerProviderError> {
    let port = value
        .parse::<u16>()
        .map_err(|_| invalid(format!("{field} `{value}` must be between 1 and 65535")))?;
    if port == 0 {
        return Err(invalid(format!(
            "{field} `{value}` must be between 1 and 65535"
        )));
    }
    Ok(())
}

fn parse_optional_size(
    value: Option<&str>,
    field: &str,
) -> Result<Option<i64>, ContainerProviderError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    parse_size(value, field).map(Some)
}

fn parse_size(value: &str, field: &str) -> Result<i64, ContainerProviderError> {
    if value == "-1" {
        return Ok(-1);
    }
    let split_at = value
        .find(|character: char| !(character.is_ascii_digit() || character == '.'))
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split_at);
    let amount = number
        .parse::<f64>()
        .map_err(|_| invalid(format!("{field} `{value}` is not a valid size")))?;
    if !amount.is_finite() || amount < 0.0 {
        return Err(invalid(format!("{field} `{value}` is not a valid size")));
    }
    let multiplier = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1.0,
        "k" | "kb" | "kib" => 1024.0,
        "m" | "mb" | "mib" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        "t" | "tb" | "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => {
            return Err(invalid(format!(
                "{field} `{value}` uses an unsupported size unit"
            )));
        }
    };
    let bytes = amount * multiplier;
    if bytes > i64::MAX as f64 {
        return Err(invalid(format!("{field} `{value}` is too large")));
    }
    Ok(bytes.round() as i64)
}

fn parse_optional_cpus(value: Option<&str>) -> Result<Option<i64>, ContainerProviderError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let cpus = value
        .parse::<f64>()
        .map_err(|_| invalid(format!("cpus `{value}` is not a valid number")))?;
    if !cpus.is_finite() || cpus <= 0.0 {
        return Err(invalid(format!("cpus `{value}` must be greater than 0")));
    }
    let nano_cpus = cpus * 1_000_000_000.0;
    if nano_cpus > i64::MAX as f64 {
        return Err(invalid(format!("cpus `{value}` is too large")));
    }
    Ok(Some(nano_cpus.round() as i64))
}

fn duration_seconds_to_nanos(
    value: Option<i64>,
    field: &str,
) -> Result<Option<i64>, ContainerProviderError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value < 0 {
        return Err(invalid(format!("{field} cannot be negative")));
    }
    value
        .checked_mul(1_000_000_000)
        .map(Some)
        .ok_or_else(|| invalid(format!("{field} is too large")))
}

fn positive_option(value: Option<i64>, field: &str) -> Result<Option<i64>, ContainerProviderError> {
    if let Some(value) = value
        && value < 0
    {
        return Err(invalid(format!("{field} cannot be negative")));
    }
    Ok(value)
}

fn required_text(value: &str, field: &'static str) -> Result<String, ContainerProviderError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ContainerProviderError::InvalidRequest(format!(
            "{field} is required"
        )));
    }
    Ok(value.to_string())
}

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn clean_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn true_option(value: bool) -> Option<bool> {
    value.then_some(true)
}

fn invalid(message: impl Into<String>) -> ContainerProviderError {
    ContainerProviderError::InvalidRequest(message.into())
}

fn simple_result(id_or_name: &str, action: &str) -> ContainerOperationResult {
    ContainerOperationResult {
        id: Some(id_or_name.to_string()),
        name: None,
        action: action.to_string(),
        details: json!({}),
    }
}

fn require_identifier(value: &str, field: &'static str) -> Result<(), ContainerProviderError> {
    if value.trim().is_empty() {
        return Err(ContainerProviderError::InvalidRequest(format!(
            "{field} is required"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_config_maps_ports_resources_restart_and_healthcheck() {
        let mut request = minimal_request();
        request.platform = Some("linux/amd64".to_string());
        request.ports = vec![ContainerPortBinding {
            container_port: "80".to_string(),
            protocol: Some("tcp".to_string()),
            host_ip: Some("127.0.0.1".to_string()),
            host_port: Some("8080".to_string()),
        }];
        request.memory = Some("512m".to_string());
        request.cpus = Some("0.5".to_string());
        request.restart_policy = Some(ContainerRestartPolicyName::OnFailure);
        request.restart_max_retries = Some(3);
        request.healthcheck = Some(ContainerHealthcheck {
            disabled: false,
            command: Some("curl -f http://localhost/ || exit 1".to_string()),
            interval_seconds: Some(30),
            timeout_seconds: Some(5),
            retries: Some(2),
            start_period_seconds: Some(10),
            start_interval_seconds: None,
        });

        let (options, config) = match build_create_container_config(&request) {
            Ok(config) => config,
            Err(error) => panic!("create config should build: {error}"),
        };

        assert_eq!(options.platform.as_deref(), Some("linux/amd64"));
        assert_eq!(config.exposed_ports.as_ref().map(HashMap::len), Some(1));
        let host_config = match config.host_config.as_ref() {
            Some(host_config) => host_config,
            None => panic!("host config should be present"),
        };
        assert_eq!(host_config.memory, Some(536_870_912));
        assert_eq!(host_config.nano_cpus, Some(500_000_000));
        assert!(
            host_config
                .port_bindings
                .as_ref()
                .is_some_and(|bindings| { bindings.contains_key("80/tcp") })
        );
        assert!(host_config.restart_policy.as_ref().is_some_and(|policy| {
            policy.name == Some(RestartPolicyNameEnum::ON_FAILURE)
                && policy.maximum_retry_count == Some(3)
        }));
        let healthcheck = match config.healthcheck.as_ref() {
            Some(healthcheck) => healthcheck,
            None => panic!("healthcheck should be present"),
        };
        assert_eq!(
            healthcheck.test.as_ref(),
            Some(&vec![
                "CMD-SHELL".to_string(),
                "curl -f http://localhost/ || exit 1".to_string(),
            ]),
        );
        assert_eq!(healthcheck.interval, Some(30_000_000_000));
    }

    #[test]
    fn create_config_maps_network_storage_and_features() {
        let mut request = minimal_request();
        request.network_name = Some("frontend".to_string());
        request.aliases = vec!["web".to_string()];
        request.ipv4_address = Some("172.20.0.10".to_string());
        request.binds = vec!["site-data:/usr/share/nginx/html:ro".to_string()];
        request.volumes = vec!["/cache".to_string()];
        request.tmpfs = vec!["/run:rw,size=64m".to_string()];
        request.devices = vec![ContainerDevice {
            host_path: "/dev/fuse".to_string(),
            container_path: None,
            permissions: Some("rwm".to_string()),
        }];
        request.init = true;
        request.tty = true;
        request.cap_add = vec!["NET_ADMIN".to_string()];

        let (_options, config) = match build_create_container_config(&request) {
            Ok(config) => config,
            Err(error) => panic!("create config should build: {error}"),
        };

        assert!(
            config
                .volumes
                .as_ref()
                .is_some_and(|volumes| { volumes.contains_key("/cache") })
        );
        assert!(
            config
                .networking_config
                .as_ref()
                .is_some_and(|networking| { networking.endpoints_config.contains_key("frontend") })
        );
        let host_config = match config.host_config.as_ref() {
            Some(host_config) => host_config,
            None => panic!("host config should be present"),
        };
        assert_eq!(host_config.network_mode.as_deref(), Some("frontend"));
        assert_eq!(
            host_config.binds.as_ref(),
            Some(&vec!["site-data:/usr/share/nginx/html:ro".to_string()]),
        );
        assert!(host_config.tmpfs.as_ref().is_some_and(|tmpfs| {
            tmpfs
                .get("/run")
                .is_some_and(|options| options == "rw,size=64m")
        }));
        assert_eq!(host_config.init, Some(true));
        assert_eq!(config.tty, Some(true));
        assert_eq!(
            host_config.cap_add.as_ref(),
            Some(&vec!["NET_ADMIN".to_string()]),
        );
        assert!(host_config.devices.as_ref().is_some_and(|devices| {
            devices.len() == 1 && devices[0].path_on_host.as_deref() == Some("/dev/fuse")
        }));
    }

    #[test]
    fn create_config_rejects_invalid_ports_and_sizes() {
        let mut request = minimal_request();
        request.ports = vec![ContainerPortBinding {
            container_port: "70000".to_string(),
            protocol: None,
            host_ip: None,
            host_port: None,
        }];
        assert!(matches!(
            build_create_container_config(&request),
            Err(ContainerProviderError::InvalidRequest(_))
        ));

        let mut request = minimal_request();
        request.memory = Some("12zz".to_string());
        assert!(matches!(
            build_create_container_config(&request),
            Err(ContainerProviderError::InvalidRequest(_))
        ));

        let mut request = minimal_request();
        request.cpus = Some("0".to_string());
        assert!(matches!(
            build_create_container_config(&request),
            Err(ContainerProviderError::InvalidRequest(_))
        ));
    }

    fn minimal_request() -> CreateContainerRequest {
        CreateContainerRequest {
            name: "web".to_string(),
            image: "nginx:1.27".to_string(),
            platform: None,
            hostname: None,
            domainname: None,
            user: None,
            working_dir: None,
            entrypoint: Vec::new(),
            command: Vec::new(),
            env: Vec::new(),
            labels: HashMap::new(),
            network_mode: None,
            network_name: None,
            aliases: Vec::new(),
            ipv4_address: None,
            mac_address: None,
            ports: Vec::new(),
            dns: Vec::new(),
            dns_search: Vec::new(),
            extra_hosts: Vec::new(),
            binds: Vec::new(),
            volumes: Vec::new(),
            tmpfs: Vec::new(),
            shm_size: None,
            restart_policy: None,
            restart_max_retries: None,
            auto_remove: false,
            privileged: false,
            init: false,
            tty: false,
            open_stdin: false,
            read_only_rootfs: false,
            cap_add: Vec::new(),
            cap_drop: Vec::new(),
            devices: Vec::new(),
            memory: None,
            memory_swap: None,
            cpus: None,
            cpu_shares: None,
            cpuset_cpus: None,
            pids_limit: None,
            healthcheck: None,
            log_driver: None,
            log_options: HashMap::new(),
        }
    }
}
