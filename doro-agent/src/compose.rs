use doro_container::ContainerComposeCommand;
use serde::Serialize;
use serde_json::json;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

const COMPOSE_FILE_NAME: &str = "compose.yaml";
const ENV_FILE_NAME: &str = ".env";

#[derive(Debug, Clone)]
pub(crate) struct ComposeManager {
    root: PathBuf,
    docker_config_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ComposeProject {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) path: String,
    pub(crate) services: Vec<String>,
    pub(crate) compose_yaml: Option<String>,
    pub(crate) env_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ComposeCommandOutput {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) status_code: Option<i32>,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(crate) struct ComposeCommandError {
    pub(crate) message: String,
    pub(crate) output: ComposeCommandOutput,
}

impl ComposeManager {
    pub(crate) fn from_config(root: Option<&str>) -> anyhow::Result<Self> {
        let root = match root.and_then(non_empty_text) {
            Some(root) => PathBuf::from(root),
            None => default_compose_root()?,
        };
        Self::new(root)
    }

    pub(crate) fn new(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        fs::create_dir_all(root.as_ref())?;
        let root =
            fs::canonicalize(root.as_ref()).map_err(|source| path_error(root.as_ref(), source))?;
        if !root.is_dir() {
            anyhow::bail!("compose root is not a directory");
        }
        Ok(Self {
            root,
            docker_config_dir: None,
        })
    }

    pub(crate) fn with_docker_config_dir(mut self, docker_config_dir: Option<PathBuf>) -> Self {
        self.docker_config_dir = docker_config_dir;
        self
    }

    pub(crate) fn probe_cli() -> anyhow::Result<String> {
        let output = Command::new("docker")
            .args(["compose", "version"])
            .output()
            .map_err(|source| anyhow::anyhow!("failed to run docker compose version: {source}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let message = stderr.trim();
            if message.is_empty() {
                anyhow::bail!(
                    "docker compose version exited with status {}",
                    output.status
                );
            }
            anyhow::bail!("docker compose version failed: {message}");
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .next()
            .filter(|line| !line.trim().is_empty())
            .unwrap_or("docker compose is available")
            .to_string())
    }

    pub(crate) fn execute(
        &self,
        command_id: Uuid,
        command: ContainerComposeCommand,
    ) -> anyhow::Result<doro_container::ContainerCommandResult> {
        let details = match command {
            ContainerComposeCommand::List => json!(self.list_projects(false)?),
            ContainerComposeCommand::Read { project } => json!(self.read_project(&project)?),
            ContainerComposeCommand::CreateOrUpdate {
                project,
                compose_yaml,
                env_file,
            } => json!(self.create_or_update(&project, &compose_yaml, env_file.as_deref())?),
            ContainerComposeCommand::Up { project } => {
                json!(self.run_compose(&project, &["up", "-d"])?)
            }
            ContainerComposeCommand::Down { project } => {
                json!(self.run_compose(&project, &["down"])?)
            }
            ContainerComposeCommand::Restart { project } => {
                json!(self.run_compose(&project, &["restart"])?)
            }
            ContainerComposeCommand::Pull { project } => {
                json!(self.run_compose(&project, &["pull"])?)
            }
            ContainerComposeCommand::Delete { project } => json!(self.delete_project(&project)?),
        };
        Ok(doro_container::ContainerCommandResult {
            command_id,
            status: doro_container::ContainerCommandStatus::Succeeded,
            message: "docker compose command succeeded".to_string(),
            details,
        })
    }

    pub(crate) fn list_projects(&self, include_files: bool) -> anyhow::Result<Vec<ComposeProject>> {
        let mut projects = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if validate_project_name(&name).is_err() {
                continue;
            }
            projects.push(self.project_summary(&name, include_files)?);
        }
        projects.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(projects)
    }

    pub(crate) fn read_project(&self, project: &str) -> anyhow::Result<ComposeProject> {
        self.project_summary(project, true)
    }

    pub(crate) fn create_or_update(
        &self,
        project: &str,
        compose_yaml: &str,
        env_file: Option<&str>,
    ) -> anyhow::Result<ComposeProject> {
        let compose_yaml = required_text(compose_yaml, "compose YAML is required")?;
        let project_dir = self.project_dir(project)?;
        fs::create_dir_all(&project_dir)?;
        let project_dir =
            fs::canonicalize(&project_dir).map_err(|source| path_error(&project_dir, source))?;
        reject_symlink_escape(&self.root, &project_dir)?;
        fs::write(project_dir.join(COMPOSE_FILE_NAME), compose_yaml)?;
        match env_file.and_then(non_empty_text) {
            Some(body) => fs::write(project_dir.join(ENV_FILE_NAME), body)?,
            None => {
                let env_path = project_dir.join(ENV_FILE_NAME);
                if env_path.exists() {
                    fs::remove_file(env_path)?;
                }
            }
        }
        self.project_summary(project, true)
    }

    pub(crate) fn delete_project(&self, project: &str) -> anyhow::Result<ComposeProject> {
        let summary = self.project_summary(project, true)?;
        let project_dir = self.project_dir(project)?;
        reject_symlink_escape(&self.root, &project_dir)?;
        fs::remove_dir_all(project_dir)?;
        Ok(ComposeProject {
            status: "deleted".to_string(),
            ..summary
        })
    }

    pub(crate) fn compose_argv(
        &self,
        project: &str,
        args: &[&str],
    ) -> anyhow::Result<Vec<OsString>> {
        let project_dir = self.existing_project_dir(project)?;
        let compose_file = project_dir.join(COMPOSE_FILE_NAME);
        if !compose_file.is_file() {
            anyhow::bail!("compose project is missing compose.yaml");
        }
        let mut argv = vec![
            OsString::from("compose"),
            OsString::from("-f"),
            compose_file.into_os_string(),
            OsString::from("--project-name"),
            OsString::from(project),
        ];
        argv.extend(args.iter().map(OsString::from));
        Ok(argv)
    }

    fn run_compose(&self, project: &str, args: &[&str]) -> anyhow::Result<ComposeCommandOutput> {
        let project_dir = self.existing_project_dir(project)?;
        let argv = self.compose_argv(project, args)?;
        let mut command = Command::new("docker");
        command.args(&argv).current_dir(project_dir);
        if let Some(config_dir) = &self.docker_config_dir {
            command.env("DOCKER_CONFIG", config_dir);
        }
        let output = command
            .output()
            .map_err(|source| anyhow::anyhow!("failed to run docker compose: {source}"))?;
        let result = ComposeCommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            status_code: output.status.code(),
        };
        if !output.status.success() {
            let status = result
                .status_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string());
            let stderr = result.stderr.trim();
            let message = if stderr.is_empty() {
                format!("docker compose failed with status {status}")
            } else {
                format!("docker compose failed with status {status}: {stderr}")
            };
            return Err(ComposeCommandError {
                message,
                output: result,
            }
            .into());
        }
        Ok(result)
    }

    fn project_summary(
        &self,
        project: &str,
        include_files: bool,
    ) -> anyhow::Result<ComposeProject> {
        let project_dir = self.existing_project_dir(project)?;
        let compose_path = project_dir.join(COMPOSE_FILE_NAME);
        let env_path = project_dir.join(ENV_FILE_NAME);
        let compose_yaml = if compose_path.is_file() {
            Some(fs::read_to_string(&compose_path)?)
        } else {
            None
        };
        let env_file = if env_path.is_file() {
            Some(fs::read_to_string(&env_path)?)
        } else {
            None
        };
        Ok(ComposeProject {
            name: project.to_string(),
            status: if compose_yaml.is_some() {
                "configured".to_string()
            } else {
                "missing_compose_file".to_string()
            },
            path: project_dir.display().to_string(),
            services: services_from_compose(compose_yaml.as_deref().unwrap_or_default()),
            compose_yaml: include_files.then_some(compose_yaml).flatten(),
            env_file: include_files.then_some(env_file).flatten(),
        })
    }

    fn project_dir(&self, project: &str) -> anyhow::Result<PathBuf> {
        validate_project_name(project)?;
        let path = self.root.join(project);
        if !path.starts_with(&self.root) {
            anyhow::bail!("compose project path is outside the compose root");
        }
        Ok(path)
    }

    fn existing_project_dir(&self, project: &str) -> anyhow::Result<PathBuf> {
        let path = self.project_dir(project)?;
        let canonical = fs::canonicalize(&path).map_err(|source| path_error(&path, source))?;
        reject_symlink_escape(&self.root, &canonical)?;
        if !canonical.is_dir() {
            anyhow::bail!("compose project is not a directory");
        }
        Ok(canonical)
    }
}

fn default_compose_root() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Agent user home directory is unavailable"))?;
    Ok(home.join(".doro").join("compose"))
}

fn validate_project_name(project: &str) -> anyhow::Result<()> {
    let project = required_text(project, "compose project name is required")?;
    let valid = project.len() <= 64
        && project
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && project
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && project
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric);
    if !valid {
        anyhow::bail!("compose project name must use lowercase letters, digits, and hyphens");
    }
    Ok(())
}

fn reject_symlink_escape(root: &Path, path: &Path) -> anyhow::Result<()> {
    if path == root || path.starts_with(root) {
        return Ok(());
    }
    anyhow::bail!("compose project path is outside the compose root")
}

fn required_text<'a>(value: &'a str, field: &str) -> anyhow::Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!("{field}");
    }
    Ok(value)
}

fn non_empty_text(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn path_error(path: &Path, source: io::Error) -> anyhow::Error {
    anyhow::anyhow!("{}: {}", path.display(), source)
}

fn services_from_compose(compose_yaml: &str) -> Vec<String> {
    let mut services = Vec::new();
    let mut in_services = false;
    for line in compose_yaml.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() || trimmed.trim_start().starts_with('#') {
            continue;
        }
        if !trimmed.starts_with(' ') && !trimmed.starts_with('\t') {
            in_services = trimmed == "services:";
            continue;
        }
        if !in_services {
            continue;
        }
        if trimmed.starts_with("  ") && !trimmed.starts_with("    ") {
            let Some((name, _)) = trimmed.trim().split_once(':') else {
                continue;
            };
            let name = name.trim().trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                services.push(name.to_string());
            }
        }
    }
    services
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_project_name_rejects_path_traversal() {
        assert!(validate_project_name("../bad").is_err());
        assert!(validate_project_name("bad/name").is_err());
        assert!(validate_project_name("bad_name").is_err());
        assert!(validate_project_name("Good").is_err());
        assert!(validate_project_name("good-name-1").is_ok());
    }

    #[test]
    fn compose_manager_stays_inside_root() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let manager = ComposeManager::new(dir.path())?;

        let project = manager.create_or_update(
            "media",
            "services:\n  nginx:\n    image: nginx:1.27\n",
            Some("TAG=1\n"),
        )?;

        assert_eq!(project.name, "media");
        assert_eq!(project.services, vec!["nginx"]);
        assert!(dir.path().join("media").join("compose.yaml").exists());
        assert!(dir.path().join("media").join(".env").exists());
        Ok(())
    }

    #[test]
    fn compose_argv_is_stable() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let manager = ComposeManager::new(dir.path())?;
        manager.create_or_update("media", "services:\n  app:\n    image: nginx\n", None)?;

        let argv = manager.compose_argv("media", &["up", "-d"])?;
        let rendered = argv
            .iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(rendered[0], "compose");
        assert_eq!(rendered[1], "-f");
        assert!(rendered[2].ends_with("media/compose.yaml"));
        assert_eq!(&rendered[3..], ["--project-name", "media", "up", "-d"]);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn compose_manager_rejects_symlink_escape() -> anyhow::Result<()> {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        symlink(outside.path(), root.path().join("escape"))?;
        let manager = ComposeManager::new(root.path())?;

        let result = manager.read_project("escape");

        assert!(result.is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn compose_manager_rejects_symlink_escape_on_write() -> anyhow::Result<()> {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        symlink(outside.path(), root.path().join("escape"))?;
        let manager = ComposeManager::new(root.path())?;

        let result = manager.create_or_update("escape", "services: {}\n", None);

        assert!(result.is_err());
        assert!(!outside.path().join("compose.yaml").exists());
        Ok(())
    }
}
