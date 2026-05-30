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
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

const DEFAULT_SEARCH_LIMIT: usize = 500;

#[derive(Debug)]
pub struct FileCommandOutput {
    pub message: String,
    pub result_json: String,
    pub content: Vec<u8>,
}

pub fn list_directory(path: &str) -> anyhow::Result<FileCommandOutput> {
    let directory = canonical_existing_path(path)?;
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
        parent_path: directory.parent().map(display_path),
        items,
    };
    json_output("directory listed", &response)
}

pub fn read_file(path: &str, max_bytes: usize) -> anyhow::Result<FileCommandOutput> {
    let path = canonical_existing_path(path)?;
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

pub fn search_files(path: &str, query: &str, limit: u32) -> anyhow::Result<FileCommandOutput> {
    let root = canonical_existing_path(path)?;
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

pub fn run_operation(
    command: grpc::RunFileOperationCommand,
    max_bytes: usize,
) -> anyhow::Result<FileCommandOutput> {
    match command.operation.as_str() {
        "create_directory" => create_directory(&command.path),
        "upload" => upload_file(
            &command.path,
            &command.content,
            command.overwrite,
            max_bytes,
        ),
        "rename" => rename_path(&command.path, &command.name, command.overwrite),
        "move" => move_path(&command.path, &command.target_path, command.overwrite),
        "copy" => copy_path(&command.path, &command.target_path, command.overwrite),
        "delete" => delete_path(&command.path),
        other => anyhow::bail!("unsupported file operation: {other}"),
    }
}

fn create_directory(path: &str) -> anyhow::Result<FileCommandOutput> {
    if path.trim().is_empty() {
        anyhow::bail!("directory path is required");
    }
    let path = PathBuf::from(path);
    fs::create_dir_all(&path)?;
    let entry = file_entry(&canonical_existing_path(path)?)?;
    operation_output(Some(entry), "directory created")
}

fn upload_file(
    path: &str,
    content: &[u8],
    overwrite: bool,
    max_bytes: usize,
) -> anyhow::Result<FileCommandOutput> {
    if content.len() > max_bytes {
        anyhow::bail!("file is larger than the transfer limit");
    }
    let path = PathBuf::from(path);
    if path.exists() && !overwrite {
        anyhow::bail!("target already exists");
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, content)?;
    let entry = file_entry(&canonical_existing_path(path)?)?;
    operation_output(Some(entry), "file uploaded")
}

fn rename_path(path: &str, name: &str, overwrite: bool) -> anyhow::Result<FileCommandOutput> {
    let source = canonical_existing_path(path)?;
    let name = validate_name(name)?;
    let target = source
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent"))?
        .join(name);
    rename_or_move(&source, &target, overwrite)?;
    let entry = file_entry(&canonical_existing_path(target)?)?;
    operation_output(Some(entry), "path renamed")
}

fn move_path(path: &str, target_path: &str, overwrite: bool) -> anyhow::Result<FileCommandOutput> {
    let source = canonical_existing_path(path)?;
    let target = PathBuf::from(required_text(target_path, "target path is required")?);
    rename_or_move(&source, &target, overwrite)?;
    let entry = file_entry(&canonical_existing_path(target)?)?;
    operation_output(Some(entry), "path moved")
}

fn copy_path(path: &str, target_path: &str, overwrite: bool) -> anyhow::Result<FileCommandOutput> {
    let source = canonical_existing_path(path)?;
    let target = PathBuf::from(required_text(target_path, "target path is required")?);
    if target.exists() && !overwrite {
        anyhow::bail!("target already exists");
    }
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let metadata = fs::metadata(&source)?;
    if metadata.is_dir() {
        copy_directory(&source, &target, overwrite)?;
    } else {
        fs::copy(&source, &target)?;
    }
    let entry = file_entry(&canonical_existing_path(target)?)?;
    operation_output(Some(entry), "path copied")
}

fn delete_path(path: &str) -> anyhow::Result<FileCommandOutput> {
    let path = canonical_existing_path(path)?;
    let metadata = fs::metadata(&path)?;
    if metadata.is_dir() {
        fs::remove_dir_all(&path)?;
    } else {
        fs::remove_file(&path)?;
    }
    operation_output(None, "path deleted")
}

fn rename_or_move(source: &Path, target: &Path, overwrite: bool) -> anyhow::Result<()> {
    if target.exists() {
        if !overwrite {
            anyhow::bail!("target already exists");
        }
        let metadata = fs::metadata(target)?;
        if metadata.is_dir() {
            fs::remove_dir_all(target)?;
        } else {
            fs::remove_file(target)?;
        }
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
    if target.exists() && overwrite {
        fs::remove_dir_all(target)?;
    }
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::metadata(&source_path)?;
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
    let metadata = fs::metadata(path).unwrap_or_else(|_| symlink_metadata.clone());
    let file_type = symlink_metadata.file_type();
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
        fs::write(dir.path().join("file.txt"), b"hello")?;
        fs::create_dir(dir.path().join("child"))?;

        let output = list_directory(&dir.path().display().to_string())?;
        let response: FileDirectoryResponse = serde_json::from_str(&output.result_json)?;

        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[0].kind, FileEntryKind::Directory);
        Ok(())
    }

    #[test]
    fn search_files_matches_file_names() -> anyhow::Result<()> {
        let dir = tempdir()?;
        fs::write(dir.path().join("needle.txt"), b"hello")?;

        let output = search_files(&dir.path().display().to_string(), "need", 500)?;
        let response: FileSearchResponse = serde_json::from_str(&output.result_json)?;

        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].name, "needle.txt");
        Ok(())
    }

    #[test]
    fn file_operations_cover_create_upload_copy_move_rename_delete() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let folder = dir.path().join("folder");
        let file = folder.join("file.txt");
        let copied = dir.path().join("copy.txt");
        let moved = dir.path().join("moved.txt");

        run_operation(
            grpc::RunFileOperationCommand {
                command_id: "1".to_string(),
                operation: "create_directory".to_string(),
                path: folder.display().to_string(),
                target_path: String::new(),
                name: String::new(),
                content: Vec::new(),
                overwrite: false,
            },
            64,
        )?;
        run_operation(
            grpc::RunFileOperationCommand {
                command_id: "2".to_string(),
                operation: "upload".to_string(),
                path: file.display().to_string(),
                target_path: String::new(),
                name: String::new(),
                content: b"hello".to_vec(),
                overwrite: false,
            },
            64,
        )?;
        run_operation(
            grpc::RunFileOperationCommand {
                command_id: "3".to_string(),
                operation: "copy".to_string(),
                path: file.display().to_string(),
                target_path: copied.display().to_string(),
                name: String::new(),
                content: Vec::new(),
                overwrite: false,
            },
            64,
        )?;
        run_operation(
            grpc::RunFileOperationCommand {
                command_id: "4".to_string(),
                operation: "move".to_string(),
                path: copied.display().to_string(),
                target_path: moved.display().to_string(),
                name: String::new(),
                content: Vec::new(),
                overwrite: false,
            },
            64,
        )?;
        run_operation(
            grpc::RunFileOperationCommand {
                command_id: "5".to_string(),
                operation: "rename".to_string(),
                path: moved.display().to_string(),
                target_path: String::new(),
                name: "renamed.txt".to_string(),
                content: Vec::new(),
                overwrite: false,
            },
            64,
        )?;
        run_operation(
            grpc::RunFileOperationCommand {
                command_id: "6".to_string(),
                operation: "delete".to_string(),
                path: dir.path().join("renamed.txt").display().to_string(),
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
        let file = dir.path().join("big.txt");
        fs::write(&file, b"hello")?;

        let result = read_file(&file.display().to_string(), 4);

        assert!(result.is_err());
        Ok(())
    }
}
