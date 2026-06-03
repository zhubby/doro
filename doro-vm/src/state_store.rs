use crate::VmId;
use crate::VmProviderError;
use crate::VmRuntimeState;
use crate::VmSnapshot;
use std::cmp::Reverse;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileStateStore {
    root: PathBuf,
}

impl FileStateStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn vm_dir(&self, id: &VmId) -> Result<PathBuf, VmProviderError> {
        validate_id(id)?;
        Ok(self.root.join(&id.0))
    }

    pub fn state_path(&self, id: &VmId) -> Result<PathBuf, VmProviderError> {
        Ok(self.vm_dir(id)?.join("state.json"))
    }

    pub fn disk_path(&self, id: &VmId) -> Result<PathBuf, VmProviderError> {
        Ok(self.vm_dir(id)?.join("disk.qcow2"))
    }

    pub fn snapshots_dir(&self, id: &VmId) -> Result<PathBuf, VmProviderError> {
        Ok(self.vm_dir(id)?.join("snapshots"))
    }

    pub fn snapshot_path(&self, id: &VmId, snapshot_ref: &str) -> Result<PathBuf, VmProviderError> {
        validate_ref(snapshot_ref, "snapshot ref")?;
        Ok(self.snapshots_dir(id)?.join(format!("{snapshot_ref}.json")))
    }

    pub fn list(&self) -> Result<Vec<VmRuntimeState>, VmProviderError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut states: Vec<VmRuntimeState> = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path().join("state.json");
            if !path.exists() {
                continue;
            }
            let raw = fs::read_to_string(path)?;
            states.push(serde_json::from_str(&raw)?);
        }
        states.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(states)
    }

    pub fn load(&self, id: &VmId) -> Result<VmRuntimeState, VmProviderError> {
        let path = self.state_path(id)?;
        if !path.exists() {
            return Err(VmProviderError::NotFound(id.clone()));
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, state: &VmRuntimeState) -> Result<(), VmProviderError> {
        let dir = self.vm_dir(&state.id)?;
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join("state.json"),
            serde_json::to_string_pretty(state)?.as_bytes(),
        )?;
        Ok(())
    }

    pub fn snapshots(&self, id: &VmId) -> Result<Vec<VmSnapshot>, VmProviderError> {
        let dir = self.snapshots_dir(id)?;
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots: Vec<VmSnapshot> = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(path)?;
            snapshots.push(serde_json::from_str(&raw)?);
        }
        snapshots.sort_by_key(|snapshot| Reverse(snapshot.created_at));
        Ok(snapshots)
    }

    pub fn save_snapshot(&self, snapshot: &VmSnapshot) -> Result<(), VmProviderError> {
        let dir = self.snapshots_dir(&snapshot.vm_id)?;
        fs::create_dir_all(&dir)?;
        fs::write(
            self.snapshot_path(&snapshot.vm_id, &snapshot.snapshot_ref)?,
            serde_json::to_string_pretty(snapshot)?.as_bytes(),
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &VmId) -> Result<(), VmProviderError> {
        let dir = self.vm_dir(id)?;
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}

fn validate_id(id: &VmId) -> Result<(), VmProviderError> {
    validate_ref(&id.0, "vm id")
}

fn validate_ref(value: &str, label: &str) -> Result<(), VmProviderError> {
    if value.is_empty()
        || value.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
    {
        return Err(VmProviderError::InvalidRequest(format!(
            "{label} may only contain ascii letters, numbers, dashes, and underscores"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_ids() {
        let store = FileStateStore::new("/tmp/doro-vm-test");
        let result = store.vm_dir(&VmId::new("../outside"));

        assert!(matches!(result, Err(VmProviderError::InvalidRequest(_))));
    }
}
