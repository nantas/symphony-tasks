use anyhow::{Context, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;

pub fn write_json_file<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let tmp_path = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(value).context("failed to encode json")?;
    fs::write(&tmp_path, body)
        .with_context(|| format!("failed to write temp file {}", tmp_path.display()))?;
    fs::rename(&tmp_path, path)
        .with_context(|| format!("failed to move temp file into place {}", path.display()))?;

    Ok(())
}

pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let body = fs::read_to_string(path)
        .with_context(|| format!("failed to read json file {}", path.display()))?;
    serde_json::from_str(&body).context("failed to decode json")
}
