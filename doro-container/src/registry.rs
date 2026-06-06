use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bollard::auth::DockerCredentials;
use serde_json::Value;
use std::path::Path;

const DOCKER_HUB_REGISTRY: &str = "docker.io";
const DOCKER_HUB_CONFIG_KEY: &str = "https://index.docker.io/v1/";

pub fn registry_for_image_reference(reference: &str) -> String {
    let reference = reference.trim();
    let Some((first, _)) = reference.split_once('/') else {
        return DOCKER_HUB_REGISTRY.to_string();
    };
    if first.contains('.') || first.contains(':') || first == "localhost" {
        normalize_registry(first)
    } else {
        DOCKER_HUB_REGISTRY.to_string()
    }
}

pub fn normalize_registry(registry: &str) -> String {
    let registry = registry
        .trim()
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    let registry = registry
        .strip_prefix("index.")
        .unwrap_or(registry.as_str())
        .to_string();
    match registry.as_str() {
        "docker.io"
        | "docker.io/v1"
        | "registry-1.docker.io"
        | "index.docker.io"
        | "index.docker.io/v1" => DOCKER_HUB_REGISTRY.to_string(),
        _ => registry,
    }
}

pub fn docker_config_registry_key(registry: &str) -> String {
    let normalized = normalize_registry(registry);
    if normalized == DOCKER_HUB_REGISTRY {
        DOCKER_HUB_CONFIG_KEY.to_string()
    } else {
        normalized
    }
}

pub(crate) fn docker_credentials_for_reference(
    config_dir: Option<&Path>,
    reference: &str,
) -> Option<DockerCredentials> {
    let config_dir = config_dir?;
    let config_path = config_dir.join("config.json");
    let value = std::fs::read_to_string(config_path)
        .ok()
        .and_then(|body| serde_json::from_str::<Value>(&body).ok())?;
    let auths = value.get("auths")?.as_object()?;
    let registry = registry_for_image_reference(reference);
    auths
        .iter()
        .find(|(key, _)| normalize_registry(key) == registry)
        .and_then(|(key, value)| credentials_from_auth_entry(key, value))
}

pub(crate) fn credentials_from_auth_entry(key: &str, value: &Value) -> Option<DockerCredentials> {
    let object = value.as_object()?;
    let username = object
        .get("username")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let password = object
        .get("password")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let auth = object
        .get("auth")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let identitytoken = object
        .get("identitytoken")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let (decoded_username, decoded_password) = auth.as_deref().and_then(decode_auth_pair).unzip();
    let username = username.or(decoded_username);
    let password = password.or(decoded_password);
    if username.is_none() && password.is_none() && auth.is_none() && identitytoken.is_none() {
        return None;
    }
    Some(DockerCredentials {
        username,
        password,
        auth,
        email: None,
        serveraddress: Some(docker_config_registry_key(key)),
        identitytoken,
        registrytoken: None,
    })
}

pub fn decode_auth_username(auth: &str) -> Option<String> {
    decode_auth_pair(auth).map(|(username, _)| username)
}

fn decode_auth_pair(auth: &str) -> Option<(String, String)> {
    let decoded = STANDARD.decode(auth.as_bytes()).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (username, password) = decoded.split_once(':')?;
    Some((username.to_string(), password.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn image_reference_registry_defaults_to_docker_hub() {
        assert_eq!(registry_for_image_reference("postgres:16"), "docker.io");
        assert_eq!(
            registry_for_image_reference("ghcr.io/example/app:latest"),
            "ghcr.io"
        );
        assert_eq!(
            registry_for_image_reference("localhost:5000/app:latest"),
            "localhost:5000"
        );
    }

    #[test]
    fn docker_hub_registry_keys_are_normalized() {
        assert_eq!(
            normalize_registry("https://index.docker.io/v1/"),
            "docker.io"
        );
        assert_eq!(
            docker_config_registry_key("docker.io"),
            "https://index.docker.io/v1/"
        );
    }

    #[test]
    fn credentials_decode_inline_auth_entry() {
        let auth = STANDARD.encode("user:secret");
        let credentials = match credentials_from_auth_entry(
            "ghcr.io",
            &json!({
                "auth": auth,
            }),
        ) {
            Some(credentials) => credentials,
            None => panic!("inline auth should produce credentials"),
        };

        assert_eq!(credentials.username.as_deref(), Some("user"));
        assert_eq!(credentials.password.as_deref(), Some("secret"));
        assert_eq!(credentials.serveraddress.as_deref(), Some("ghcr.io"));
    }
}
