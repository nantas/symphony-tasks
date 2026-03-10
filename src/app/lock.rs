use anyhow::{Context, Result};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct DaemonLock {
    path: PathBuf,
    _file: File,
}

impl DaemonLock {
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create lock dir {}", parent.display()))?;
        }

        let file = File::options()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("failed to acquire daemon lock {}", path.display()))?;

        Ok(Self { path, _file: file })
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
