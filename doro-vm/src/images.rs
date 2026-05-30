use crate::VmImageRef;
use crate::VmProviderError;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LocalImageStore {
    root: PathBuf,
}

impl LocalImageStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn images(&self) -> Result<Vec<VmImageRef>, VmProviderError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut images = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };
            if extension != "qcow2" && extension != "img" {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("image")
                .to_string();
            images.push(VmImageRef {
                id: name.clone(),
                name,
                path,
                os_family: None,
                architecture: std::env::consts::ARCH.to_string(),
            });
        }
        images.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(images)
    }
}
