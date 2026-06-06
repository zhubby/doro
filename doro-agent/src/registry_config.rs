use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use doro_container::{
    ContainerRegistryCommand, RegistryCredentialSummary, RemoveRegistryCredentialRequest,
    UpsertRegistryCredentialRequest, decode_auth_username, docker_config_registry_key,
    normalize_registry,
};
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone)]
pub(crate) struct DockerRegistryConfigManager {
    config_dir: PathBuf,
}

impl DockerRegistryConfigManager {
    pub(crate) fn from_default_config_dir() -> anyhow::Result<Self> {
        Ok(Self::new(default_docker_config_dir()?))
    }

    pub(crate) fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    pub(crate) fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub(crate) fn execute(
        &self,
        command: ContainerRegistryCommand,
    ) -> anyhow::Result<serde_json::Value> {
        match command {
            ContainerRegistryCommand::List => Ok(json!(self.list()?)),
            ContainerRegistryCommand::Upsert(request) => Ok(json!(self.upsert(request)?)),
            ContainerRegistryCommand::Remove(request) => Ok(json!(self.remove(request)?)),
        }
    }

    fn list(&self) -> anyhow::Result<Vec<RegistryCredentialSummary>> {
        let config = self.read_config()?;
        Ok(self.summaries_from_config(&config))
    }

    fn upsert(
        &self,
        request: UpsertRegistryCredentialRequest,
    ) -> anyhow::Result<RegistryCredentialSummary> {
        let registry = required_registry(&request.registry)?;
        let username = required_text(&request.username, "registry username is required")?;
        let secret = required_text(&request.secret, "registry secret is required")?;
        let mut config = self.read_config()?;
        if self.has_external_credential(&config, &registry) {
            anyhow::bail!("registry credential is managed by Docker credential helper");
        }
        let object = config_object_mut(&mut config)?;
        let auths_value = object.entry("auths").or_insert_with(|| json!({}));
        let auths = auths_value
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Docker config auths must be an object"))?;
        auths.retain(|key, _| normalize_registry(key) != registry);
        let auth = STANDARD.encode(format!("{username}:{secret}"));
        auths.insert(
            docker_config_registry_key(&registry),
            json!({
                "auth": auth,
            }),
        );
        self.write_config(&config)?;
        Ok(RegistryCredentialSummary {
            registry,
            username: Some(username),
            source: "inline".to_string(),
            has_secret: true,
            config_path: self.config_path().display().to_string(),
        })
    }

    fn remove(
        &self,
        request: RemoveRegistryCredentialRequest,
    ) -> anyhow::Result<RegistryCredentialSummary> {
        let registry = required_registry(&request.registry)?;
        let mut config = self.read_config()?;
        if self.has_external_credential(&config, &registry) {
            anyhow::bail!("registry credential is managed by Docker credential helper");
        }
        if let Some(auths) = config.get_mut("auths").and_then(Value::as_object_mut) {
            auths.retain(|key, _| normalize_registry(key) != registry);
        }
        self.write_config(&config)?;
        Ok(RegistryCredentialSummary {
            registry,
            username: None,
            source: "removed".to_string(),
            has_secret: false,
            config_path: self.config_path().display().to_string(),
        })
    }

    fn summaries_from_config(&self, config: &Value) -> Vec<RegistryCredentialSummary> {
        let mut summaries = BTreeMap::<String, RegistryCredentialSummary>::new();
        let config_path = self.config_path().display().to_string();
        if let Some(auths) = config.get("auths").and_then(Value::as_object) {
            for (key, entry) in auths {
                let registry = normalize_registry(key);
                let summary = inline_summary(&registry, entry, &config_path).unwrap_or_else(|| {
                    RegistryCredentialSummary {
                        registry: registry.clone(),
                        username: None,
                        source: "external".to_string(),
                        has_secret: false,
                        config_path: config_path.clone(),
                    }
                });
                summaries.insert(registry, summary);
            }
        }
        if let Some(helpers) = config.get("credHelpers").and_then(Value::as_object) {
            for key in helpers.keys() {
                let registry = normalize_registry(key);
                summaries
                    .entry(registry.clone())
                    .or_insert_with(|| RegistryCredentialSummary {
                        registry,
                        username: None,
                        source: "external".to_string(),
                        has_secret: false,
                        config_path: config_path.clone(),
                    });
            }
        }
        summaries.into_values().collect()
    }

    fn has_external_credential(&self, config: &Value, registry: &str) -> bool {
        let helper = config
            .get("credHelpers")
            .and_then(Value::as_object)
            .is_some_and(|helpers| {
                helpers
                    .keys()
                    .any(|key| normalize_registry(key) == registry)
            });
        let global_store = config.get("credsStore").and_then(Value::as_str).is_some();
        let matching_empty_auth =
            config
                .get("auths")
                .and_then(Value::as_object)
                .is_some_and(|auths| {
                    auths.iter().any(|(key, entry)| {
                        normalize_registry(key) == registry
                            && inline_summary(registry, entry, "").is_none()
                    })
                });
        helper || (global_store && matching_empty_auth)
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join(CONFIG_FILE_NAME)
    }

    fn read_config(&self) -> anyhow::Result<Value> {
        let config_path = self.config_path();
        if !config_path.exists() {
            return Ok(json!({}));
        }
        let body = fs::read_to_string(&config_path)?;
        if body.trim().is_empty() {
            return Ok(json!({}));
        }
        let value = serde_json::from_str::<Value>(&body)
            .map_err(|error| anyhow::anyhow!("Docker config JSON is invalid: {error}"))?;
        if !value.is_object() {
            anyhow::bail!("Docker config root must be an object");
        }
        Ok(value)
    }

    fn write_config(&self, config: &Value) -> anyhow::Result<()> {
        fs::create_dir_all(&self.config_dir)?;
        let config_path = self.config_path();
        let temp_path = self
            .config_dir
            .join(format!(".config.json.doro-{}", std::process::id()));
        fs::write(&temp_path, serde_json::to_vec_pretty(config)?)?;
        #[cfg(unix)]
        {
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&temp_path, permissions)?;
        }
        match fs::rename(&temp_path, &config_path) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                Err(error.into())
            }
        }
    }
}

pub(crate) fn default_docker_config_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Agent user home directory is unavailable"))?;
    Ok(home.join(".docker"))
}

fn inline_summary(
    registry: &str,
    entry: &Value,
    config_path: &str,
) -> Option<RegistryCredentialSummary> {
    let object = entry.as_object()?;
    let auth = object.get("auth").and_then(Value::as_str);
    let username = object
        .get("username")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| auth.and_then(decode_auth_username));
    let has_secret = auth.is_some()
        || object.get("password").and_then(Value::as_str).is_some()
        || object
            .get("identitytoken")
            .and_then(Value::as_str)
            .is_some();
    if !has_secret {
        return None;
    }
    Some(RegistryCredentialSummary {
        registry: registry.to_string(),
        username,
        source: "inline".to_string(),
        has_secret: true,
        config_path: config_path.to_string(),
    })
}

fn config_object_mut(
    value: &mut Value,
) -> anyhow::Result<&mut serde_json::Map<String, serde_json::Value>> {
    if !value.is_object() {
        anyhow::bail!("Docker config root must be an object");
    }
    value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Docker config root must be an object"))
}

fn required_registry(registry: &str) -> anyhow::Result<String> {
    let registry = normalize_registry(registry);
    if registry.trim().is_empty() {
        anyhow::bail!("registry is required");
    }
    Ok(registry)
}

fn required_text(value: &str, message: &str) -> anyhow::Result<String> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!("{message}");
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn registry_config_upsert_lists_without_plain_secret() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let manager = DockerRegistryConfigManager::new(temp.path());

        let summary = manager.upsert(UpsertRegistryCredentialRequest {
            registry: "ghcr.io".to_string(),
            username: "doro".to_string(),
            secret: "token-secret".to_string(),
        })?;
        let listed = manager.list()?;
        let serialized_summary = serde_json::to_string(&summary)?;
        let serialized_listed = serde_json::to_string(&listed)?;

        assert_eq!(summary.registry, "ghcr.io");
        assert_eq!(listed.len(), 1);
        assert!(!serialized_summary.contains("token-secret"));
        assert!(!serialized_listed.contains("token-secret"));
        Ok(())
    }

    #[test]
    fn registry_config_remove_deletes_matching_docker_hub_alias() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let manager = DockerRegistryConfigManager::new(temp.path());
        manager.upsert(UpsertRegistryCredentialRequest {
            registry: "docker.io".to_string(),
            username: "doro".to_string(),
            secret: "token".to_string(),
        })?;

        manager.remove(RemoveRegistryCredentialRequest {
            registry: "https://index.docker.io/v1/".to_string(),
        })?;

        assert!(manager.list()?.is_empty());
        Ok(())
    }

    #[test]
    fn registry_config_refuses_credential_helper_entries() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        fs::create_dir_all(temp.path())?;
        fs::write(
            temp.path().join(CONFIG_FILE_NAME),
            serde_json::to_vec(&json!({
                "credHelpers": {
                    "ghcr.io": "pass"
                }
            }))?,
        )?;
        let manager = DockerRegistryConfigManager::new(temp.path());

        let result = manager.upsert(UpsertRegistryCredentialRequest {
            registry: "ghcr.io".to_string(),
            username: "doro".to_string(),
            secret: "token".to_string(),
        });

        assert!(result.is_err());
        Ok(())
    }
}
