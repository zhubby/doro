use crate::VmId;
use crate::VmProviderError;
use crate::VmRuntimeState;
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

    pub fn delete(&self, id: &VmId) -> Result<(), VmProviderError> {
        let dir = self.vm_dir(id)?;
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}

fn validate_id(id: &VmId) -> Result<(), VmProviderError> {
    if id.0.is_empty()
        || id.0.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
    {
        return Err(VmProviderError::InvalidRequest(
            "vm id may only contain ascii letters, numbers, dashes, and underscores".to_string(),
        ));
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
