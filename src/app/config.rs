use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub config_path: PathBuf,
}

impl AppConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_path: path.as_ref().to_path_buf(),
        }
    }
}
