use chrono::DateTime;
use chrono::Utc;
use doro_protocol::FileDirectoryResponse;
use doro_protocol::FileEntry;
use doro_protocol::FileEntryKind;
use doro_protocol::FileOperationResponse;
use doro_protocol::FileSearchResponse;
use doro_protocol::grpc;
use serde_json::json;
use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

const DEFAULT_SEARCH_LIMIT: usize = 500;
const HOME_SCOPE_ERROR: &str = "path is outside the Agent user home directory";

#[derive(Debug)]
pub struct FileCommandOutput {
    pub message: String,
    pub result_json: String,
    pub content: Vec<u8>,
}

pub fn list_directory(path: &str) -> anyhow::Result<FileCommandOutput> {
    HomeFileScope::from_agent_home()?.list_directory(path)
}

pub fn read_file(path: &str, max_bytes: usize) -> anyhow::Result<FileCommandOutput> {
    HomeFileScope::from_agent_home()?.read_file(path, max_bytes)
}

pub fn search_files(path: &str, query: &str, limit: u32) -> anyhow::Result<FileCommandOutput> {
    HomeFileScope::from_agent_home()?.search_files(path, query, limit)
}

pub fn run_operation(
    command: grpc::RunFileOperationCommand,
    max_bytes: usize,
) -> anyhow::Result<FileCommandOutput> {
    HomeFileScope::from_agent_home()?.run_operation(command, max_bytes)
}

#[derive(Debug, Clone)]
struct HomeFileScope {
    home: PathBuf,
}

impl HomeFileScope {
    fn from_agent_home() -> anyhow::Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Agent user home directory is unavailable"))?;
        Self::new(home)
    }

    fn new(home: impl AsRef<Path>) -> anyhow::Result<Self> {
        let home = canonical_existing_path(home)?;
        let metadata = fs::metadata(&home)?;
        if !metadata.is_dir() {
            anyhow::bail!("Agent user home directory is not a directory");
        }
        Ok(Self { home })
    }

    fn list_directory(&self, path: &str) -> anyhow::Result<FileCommandOutput> {
        let directory = self.existing_path(path)?;
        let metadata = fs::metadata(&directory)?;
        if !metadata.is_dir() {
            anyhow::bail!("path is not a directory");
        }

        let mut items = Vec::new();
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            items.push(file_entry(&entry.path())?);
        }
        items.sort_by(|a, b| {
            let a_dir = matches!(a.kind, FileEntryKind::Directory);
            let b_dir = matches!(b.kind, FileEntryKind::Directory);
            b_dir
                .cmp(&a_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let response = FileDirectoryResponse {
            path: display_path(&directory),
            parent_path: self.parent_path(&directory),
            items,
        };
        json_output("directory listed", &response)
    }

    fn read_file(&self, path: &str, max_bytes: usize) -> anyhow::Result<FileCommandOutput> {
        let path = self.existing_path(path)?;
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            anyhow::bail!("path is not a file");
        }
        if metadata.len() as usize > max_bytes {
            anyhow::bail!("file is larger than the transfer limit");
        }
        let content = fs::read(&path)?;
        Ok(FileCommandOutput {
            message: "file read".to_string(),
            result_json: json!({
                "path": display_path(&path),
                "name": file_name(&path),
                "size_bytes": content.len() as u64,
            })
            .to_string(),
            content,
        })
    }

    fn search_files(
        &self,
        path: &str,
        query: &str,
        limit: u32,
    ) -> anyhow::Result<FileCommandOutput> {
        let root = self.existing_path(path)?;
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            anyhow::bail!("search query is required");
        }

        let mut items = Vec::new();
        let mut stack = vec![root];
        let limit = if limit == 0 {
            DEFAULT_SEARCH_LIMIT
        } else {
            limit as usize
        };

        while let Some(directory) = stack.pop() {
            if items.len() >= limit {
                break;
            }
            let Ok(entries) = fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries {
                if items.len() >= limit {
                    break;
                }
                let Ok(entry) = entry else {
                    continue;
                };
                let path = entry.path();
                let name = file_name(&path);
                let Ok(entry_summary) = file_entry(&path) else {
                    continue;
                };
                if name.to_lowercase().contains(&query) {
                    items.push(entry_summary.clone());
                }
                if matches!(entry_summary.kind, FileEntryKind::Directory) {
                    stack.push(path);
                }
            }
        }

        let response = FileSearchResponse { items };
        json_output("search completed", &response)
    }

    fn run_operation(
        &self,
        command: grpc::RunFileOperationCommand,
        max_bytes: usize,
    ) -> anyhow::Result<FileCommandOutput> {
        match command.operation.as_str() {
            "create_directory" => self.create_directory(&command.path),
            "upload" => self.upload_file(
                &command.path,
                &command.content,
                command.overwrite,
                max_bytes,
            ),
            "rename" => self.rename_path(&command.path, &command.name, command.overwrite),
            "move" => self.move_path(&command.path, &command.target_path, command.overwrite),
            "copy" => self.copy_path(&command.path, &command.target_path, command.overwrite),
            "delete" => self.delete_path(&command.path),
            other => anyhow::bail!("unsupported file operation: {other}"),
        }
    }

    fn create_directory(&self, path: &str) -> anyhow::Result<FileCommandOutput> {
        let path = self.target_path(path)?;
        self.ensure_not_home_root(&path, "create directory at")?;
        fs::create_dir_all(&path)?;
        let entry = file_entry(&self.existing_path(&display_path(path))?)?;
        operation_output(Some(entry), "directory created")
    }

    fn upload_file(
        &self,
        path: &str,
        content: &[u8],
        overwrite: bool,
        max_bytes: usize,
    ) -> anyhow::Result<FileCommandOutput> {
        if content.len() > max_bytes {
            anyhow::bail!("file is larger than the transfer limit");
        }
        let path = self.target_path(path)?;
        self.ensure_not_home_root(&path, "upload to")?;
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if !overwrite {
                anyhow::bail!("target already exists");
            }
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                anyhow::bail!("target is a directory");
            }
            remove_existing_target(&path)?;
        }
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        let entry = file_entry(&self.existing_path(&display_path(path))?)?;
        operation_output(Some(entry), "file uploaded")
    }

    fn rename_path(
        &self,
        path: &str,
        name: &str,
        overwrite: bool,
    ) -> anyhow::Result<FileCommandOutput> {
        let source = self.existing_node_path(path)?;
        self.ensure_not_home_root(&source, "rename")?;
        let name = validate_name(name)?;
        let target = source
            .parent()
            .ok_or_else(|| anyhow::anyhow!("path has no parent"))?
            .join(name);
        self.ensure_path_inside_home(&target)?;
        rename_or_move(&source, &target, overwrite)?;
        let entry = file_entry(&self.existing_node_path(&display_path(target))?)?;
        operation_output(Some(entry), "path renamed")
    }

    fn move_path(
        &self,
        path: &str,
        target_path: &str,
        overwrite: bool,
    ) -> anyhow::Result<FileCommandOutput> {
        let source = self.existing_node_path(path)?;
        self.ensure_not_home_root(&source, "move")?;
        let target = self.target_path(required_text(target_path, "target path is required")?)?;
        self.ensure_not_home_root(&target, "move to")?;
        rename_or_move(&source, &target, overwrite)?;
        let entry = file_entry(&self.existing_node_path(&display_path(target))?)?;
        operation_output(Some(entry), "path moved")
    }

    fn copy_path(
        &self,
        path: &str,
        target_path: &str,
        overwrite: bool,
    ) -> anyhow::Result<FileCommandOutput> {
        let source = self.existing_node_path(path)?;
        self.ensure_not_home_root(&source, "copy")?;
        reject_symlink(&source)?;
        let target = self.target_path(required_text(target_path, "target path is required")?)?;
        self.ensure_not_home_root(&target, "copy to")?;
        if fs::symlink_metadata(&target).is_ok() {
            if !overwrite {
                anyhow::bail!("target already exists");
            }
            remove_existing_target(&target)?;
        }
        if let Some(parent) = target.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let metadata = fs::symlink_metadata(&source)?;
        if metadata.is_dir() {
            copy_directory(&source, &target, overwrite)?;
        } else {
            fs::copy(&source, &target)?;
        }
        let entry = file_entry(&self.existing_node_path(&display_path(target))?)?;
        operation_output(Some(entry), "path copied")
    }

    fn delete_path(&self, path: &str) -> anyhow::Result<FileCommandOutput> {
        let path = self.existing_node_path(path)?;
        self.ensure_not_home_root(&path, "delete")?;
        remove_existing_target(&path)?;
        operation_output(None, "path deleted")
    }

    fn path_candidate(&self, path: &str) -> anyhow::Result<PathBuf> {
        let value = path.trim();
        if value.is_empty() || value == "~" {
            return Ok(self.home.clone());
        }
        reject_parent_traversal(Path::new(value))?;
        if let Some(relative) = value
            .strip_prefix("~/")
            .or_else(|| value.strip_prefix("~\\"))
        {
            return Ok(self.home.join(relative));
        }

        let path = Path::new(value);
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(self.home.join(path))
        }
    }

    fn existing_path(&self, path: &str) -> anyhow::Result<PathBuf> {
        let candidate = self.path_candidate(path)?;
        let path = canonical_existing_path(candidate)?;
        self.ensure_path_inside_home(&path)?;
        Ok(path)
    }

    fn existing_node_path(&self, path: &str) -> anyhow::Result<PathBuf> {
        let candidate = self.path_candidate(path)?;
        if candidate == self.home {
            return Ok(self.home.clone());
        }
        let metadata = fs::symlink_metadata(&candidate)
            .map_err(|source| filesystem_error(&candidate, source))?;
        let parent = candidate
            .parent()
            .ok_or_else(|| anyhow::anyhow!("path has no parent"))?;
        let parent = canonical_existing_path(parent)?;
        self.ensure_path_inside_home(&parent)?;
        let name = candidate
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("path has no file name"))?;
        let path = parent.join(name);
        if metadata.file_type().is_symlink() {
            if let Ok(target) = fs::canonicalize(&path) {
                self.ensure_path_inside_home(&target)?;
            }
            return Ok(path);
        }

        let canonical = canonical_existing_path(&path)?;
        self.ensure_path_inside_home(&canonical)?;
        Ok(canonical)
    }

    fn target_path(&self, path: &str) -> anyhow::Result<PathBuf> {
        let candidate = self.path_candidate(path)?;
        if fs::symlink_metadata(&candidate).is_ok() {
            return self.existing_node_path(path);
        }
        let parent = nearest_existing_parent(&candidate)?;
        let canonical_parent = canonical_existing_path(&parent)?;
        self.ensure_path_inside_home(&canonical_parent)?;
        let suffix = candidate
            .strip_prefix(&parent)
            .map_err(|_| anyhow::anyhow!(HOME_SCOPE_ERROR))?;
        let target = canonical_parent.join(suffix);
        self.ensure_path_inside_home(&target)?;
        Ok(target)
    }

    fn parent_path(&self, path: &Path) -> Option<String> {
        if path == self.home {
            return None;
        }
        path.parent()
            .filter(|parent| parent == &self.home || parent.starts_with(&self.home))
            .map(display_path)
    }

    fn ensure_path_inside_home(&self, path: &Path) -> anyhow::Result<()> {
        if path == self.home || path.starts_with(&self.home) {
            return Ok(());
        }
        anyhow::bail!(HOME_SCOPE_ERROR)
    }

    fn ensure_not_home_root(&self, path: &Path, operation: &str) -> anyhow::Result<()> {
        if path == self.home {
            anyhow::bail!("{operation} the Agent user home directory is not allowed");
        }
        Ok(())
    }
}

fn rename_or_move(source: &Path, target: &Path, overwrite: bool) -> anyhow::Result<()> {
    if fs::symlink_metadata(target).is_ok() {
        if !overwrite {
            anyhow::bail!("target already exists");
        }
        remove_existing_target(target)?;
    }
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::rename(source, target)?;
    Ok(())
}

fn copy_directory(source: &Path, target: &Path, overwrite: bool) -> anyhow::Result<()> {
    if fs::symlink_metadata(target).is_ok() && overwrite {
        remove_existing_target(target)?;
    }
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!("copying symlinks is not supported");
        }
        if metadata.is_dir() {
            copy_directory(&source_path, &target_path, overwrite)?;
        } else {
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn file_entry(path: &Path) -> anyhow::Result<FileEntry> {
    let symlink_metadata = fs::symlink_metadata(path)?;
    let file_type = symlink_metadata.file_type();
    let metadata = if file_type.is_symlink() {
        symlink_metadata.clone()
    } else {
        fs::metadata(path)?
    };
    let kind = if file_type.is_symlink() {
        FileEntryKind::Symlink
    } else if metadata.is_dir() {
        FileEntryKind::Directory
    } else if metadata.is_file() {
        FileEntryKind::File
    } else {
        FileEntryKind::Other
    };
    let size_bytes = if metadata.is_file() {
        Some(metadata.len())
    } else {
        None
    };
    let modified_at = metadata.modified().ok().map(system_time_to_utc);
    let symlink_target = if file_type.is_symlink() {
        fs::read_link(path)
            .ok()
            .map(|target| target.display().to_string())
    } else {
        None
    };

    Ok(FileEntry {
        path: display_path(path),
        name: file_name(path),
        kind,
        size_bytes,
        modified_at,
        readonly: metadata.permissions().readonly(),
        symlink_target,
    })
}

fn canonical_existing_path(path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
        anyhow::bail!("path is required");
    }
    fs::canonicalize(path).map_err(|source| filesystem_error(path, source))
}

fn nearest_existing_parent(path: &Path) -> anyhow::Result<PathBuf> {
    let mut parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent"))?;
    loop {
        if fs::symlink_metadata(parent).is_ok() {
            return Ok(parent.to_path_buf());
        }
        parent = parent
            .parent()
            .ok_or_else(|| anyhow::anyhow!(HOME_SCOPE_ERROR))?;
    }
}

fn reject_parent_traversal(path: &Path) -> anyhow::Result<()> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        anyhow::bail!(
            "path must not contain '..' because file access is limited to the Agent user home directory"
        );
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!("copying symlinks is not supported");
    }
    Ok(())
}

fn remove_existing_target(path: &Path) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn filesystem_error(path: &Path, source: io::Error) -> anyhow::Error {
    anyhow::anyhow!("{}: {}", path.display(), source)
}

fn validate_name(name: &str) -> anyhow::Result<&str> {
    let name = required_text(name, "name is required")?;
    if name.contains('/') || name.contains('\\') {
        anyhow::bail!("name must not contain path separators");
    }
    Ok(name)
}

fn required_text<'a>(value: &'a str, message: &str) -> anyhow::Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!(message.to_string());
    }
    Ok(value)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn display_path(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string()
}

fn system_time_to_utc(value: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(value)
}

fn json_output<T: serde::Serialize>(message: &str, value: &T) -> anyhow::Result<FileCommandOutput> {
    Ok(FileCommandOutput {
        message: message.to_string(),
        result_json: serde_json::to_string(value)?,
        content: Vec::new(),
    })
}

fn operation_output(
    item: Option<FileEntry>,
    message: impl Into<String>,
) -> anyhow::Result<FileCommandOutput> {
    let response = FileOperationResponse {
        item,
        message: message.into(),
    };
    json_output(&response.message.clone(), &response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn list_directory_returns_files_and_directories() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;
        fs::write(dir.path().join("file.txt"), b"hello")?;
        fs::create_dir(dir.path().join("child"))?;

        let output = scope.list_directory("")?;
        let response: FileDirectoryResponse = serde_json::from_str(&output.result_json)?;

        assert_eq!(
            response.path,
            display_path(canonical_existing_path(dir.path())?)
        );
        assert_eq!(response.parent_path, None);
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[0].kind, FileEntryKind::Directory);
        Ok(())
    }

    #[test]
    fn search_files_matches_file_names() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;
        fs::write(dir.path().join("needle.txt"), b"hello")?;

        let output = scope.search_files("~", "need", 500)?;
        let response: FileSearchResponse = serde_json::from_str(&output.result_json)?;

        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].name, "needle.txt");
        Ok(())
    }

    #[test]
    fn file_operations_cover_create_upload_copy_move_rename_delete() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;

        scope.run_operation(
            grpc::RunFileOperationCommand {
                command_id: "1".to_string(),
                operation: "create_directory".to_string(),
                path: "folder".to_string(),
                target_path: String::new(),
                name: String::new(),
                content: Vec::new(),
                overwrite: false,
            },
            64,
        )?;
        scope.run_operation(
            grpc::RunFileOperationCommand {
                command_id: "2".to_string(),
                operation: "upload".to_string(),
                path: "folder/file.txt".to_string(),
                target_path: String::new(),
                name: String::new(),
                content: b"hello".to_vec(),
                overwrite: false,
            },
            64,
        )?;
        scope.run_operation(
            grpc::RunFileOperationCommand {
                command_id: "3".to_string(),
                operation: "copy".to_string(),
                path: "folder/file.txt".to_string(),
                target_path: "copy.txt".to_string(),
                name: String::new(),
                content: Vec::new(),
                overwrite: false,
            },
            64,
        )?;
        scope.run_operation(
            grpc::RunFileOperationCommand {
                command_id: "4".to_string(),
                operation: "move".to_string(),
                path: "copy.txt".to_string(),
                target_path: "moved.txt".to_string(),
                name: String::new(),
                content: Vec::new(),
                overwrite: false,
            },
            64,
        )?;
        scope.run_operation(
            grpc::RunFileOperationCommand {
                command_id: "5".to_string(),
                operation: "rename".to_string(),
                path: "moved.txt".to_string(),
                target_path: String::new(),
                name: "renamed.txt".to_string(),
                content: Vec::new(),
                overwrite: false,
            },
            64,
        )?;
        scope.run_operation(
            grpc::RunFileOperationCommand {
                command_id: "6".to_string(),
                operation: "delete".to_string(),
                path: "renamed.txt".to_string(),
                target_path: String::new(),
                name: String::new(),
                content: Vec::new(),
                overwrite: false,
            },
            64,
        )?;

        assert!(!dir.path().join("renamed.txt").exists());
        Ok(())
    }

    #[test]
    fn read_file_rejects_files_over_limit() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;
        let file = dir.path().join("big.txt");
        fs::write(&file, b"hello")?;

        let result = scope.read_file("big.txt", 4);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn relative_paths_resolve_under_home() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;
        fs::create_dir(dir.path().join("documents"))?;

        let output = scope.list_directory("documents")?;
        let response: FileDirectoryResponse = serde_json::from_str(&output.result_json)?;

        assert_eq!(
            response.path,
            display_path(canonical_existing_path(dir.path().join("documents"))?)
        );
        assert_eq!(response.parent_path, Some(display_path(scope.home)));
        Ok(())
    }

    #[test]
    fn paths_outside_home_are_rejected() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let outside = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;
        let outside_path = outside.path().display().to_string();

        let result = scope.list_directory(&outside_path);

        assert_error_contains(result, HOME_SCOPE_ERROR);
        Ok(())
    }

    #[test]
    fn parent_directory_traversal_is_rejected() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;

        let result = scope.list_directory("../outside");

        assert_error_contains(result, "Agent user home directory");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected() -> anyhow::Result<()> {
        use std::os::unix::fs::symlink;

        let dir = tempdir()?;
        let outside = tempdir()?;
        let outside_file = outside.path().join("outside.txt");
        fs::write(&outside_file, b"outside")?;
        symlink(&outside_file, dir.path().join("link"))?;
        let scope = HomeFileScope::new(dir.path())?;

        let result = scope.read_file("link", 64);

        assert_error_contains(result, HOME_SCOPE_ERROR);
        Ok(())
    }

    #[test]
    fn write_operations_reject_targets_outside_home() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let outside = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;
        fs::write(dir.path().join("source.txt"), b"hello")?;
        fs::write(outside.path().join("delete.txt"), b"delete")?;

        assert_error_contains(
            scope.run_operation(
                file_command("create_directory", outside.path().join("new")),
                64,
            ),
            HOME_SCOPE_ERROR,
        );
        assert_error_contains(
            scope.run_operation(
                grpc::RunFileOperationCommand {
                    operation: "upload".to_string(),
                    path: outside.path().join("upload.txt").display().to_string(),
                    content: b"hello".to_vec(),
                    ..file_command("", "")
                },
                64,
            ),
            HOME_SCOPE_ERROR,
        );
        assert_error_contains(
            scope.run_operation(
                grpc::RunFileOperationCommand {
                    operation: "move".to_string(),
                    path: "source.txt".to_string(),
                    target_path: outside.path().join("moved.txt").display().to_string(),
                    ..file_command("", "")
                },
                64,
            ),
            HOME_SCOPE_ERROR,
        );
        assert_error_contains(
            scope.run_operation(
                grpc::RunFileOperationCommand {
                    operation: "copy".to_string(),
                    path: "source.txt".to_string(),
                    target_path: outside.path().join("copied.txt").display().to_string(),
                    ..file_command("", "")
                },
                64,
            ),
            HOME_SCOPE_ERROR,
        );
        assert_error_contains(
            scope.run_operation(
                file_command("delete", outside.path().join("delete.txt")),
                64,
            ),
            HOME_SCOPE_ERROR,
        );
        Ok(())
    }

    #[test]
    fn write_operations_reject_home_root() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;
        fs::write(dir.path().join("source.txt"), b"hello")?;

        assert_error_contains(
            scope.run_operation(file_command("create_directory", ""), 64),
            "home directory is not allowed",
        );
        assert_error_contains(
            scope.run_operation(
                grpc::RunFileOperationCommand {
                    operation: "upload".to_string(),
                    path: String::new(),
                    content: b"hello".to_vec(),
                    overwrite: true,
                    ..file_command("", "")
                },
                64,
            ),
            "home directory is not allowed",
        );
        assert_error_contains(
            scope.run_operation(
                grpc::RunFileOperationCommand {
                    operation: "move".to_string(),
                    path: "source.txt".to_string(),
                    target_path: "~".to_string(),
                    ..file_command("", "")
                },
                64,
            ),
            "home directory is not allowed",
        );
        assert_error_contains(
            scope.run_operation(file_command("delete", ""), 64),
            "home directory is not allowed",
        );
        assert!(dir.path().exists());
        Ok(())
    }

    #[test]
    fn upload_rejects_existing_directory() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let scope = HomeFileScope::new(dir.path())?;
        let folder = dir.path().join("folder");
        fs::create_dir(&folder)?;

        let result = scope.run_operation(
            grpc::RunFileOperationCommand {
                operation: "upload".to_string(),
                path: "folder".to_string(),
                content: b"hello".to_vec(),
                overwrite: true,
                ..file_command("", "")
            },
            64,
        );

        assert_error_contains(result, "target is a directory");
        assert!(folder.is_dir());
        Ok(())
    }

    fn file_command(operation: &str, path: impl AsRef<Path>) -> grpc::RunFileOperationCommand {
        grpc::RunFileOperationCommand {
            command_id: "test".to_string(),
            operation: operation.to_string(),
            path: path.as_ref().display().to_string(),
            target_path: String::new(),
            name: String::new(),
            content: Vec::new(),
            overwrite: false,
        }
    }

    fn assert_error_contains(result: anyhow::Result<FileCommandOutput>, needle: &str) {
        match result {
            Ok(_) => panic!("operation should have failed"),
            Err(error) => assert!(error.to_string().contains(needle), "{error}"),
        }
    }
}
