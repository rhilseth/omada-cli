#![allow(dead_code)]

use anyhow::{Context, Result};
use rkyv::Deserialize;
use std::path::PathBuf;

use crate::model::ApiSpec;

fn cache_path(omadac_id: &str) -> Option<PathBuf> {
    let mut path = dirs::home_dir()?;
    path.push(".omadacli");
    path.push(omadac_id);
    path.push("spec.rkyv");
    Some(path)
}

pub fn load(omadac_id: &str) -> Option<ApiSpec> {
    let path = cache_path(omadac_id)?;
    let bytes = std::fs::read(&path).ok()?;
    let archived = rkyv::check_archived_root::<ApiSpec>(&bytes).ok()?;
    archived
        .deserialize(&mut rkyv::Infallible)
        .ok()
}

pub fn save(omadac_id: &str, spec: &ApiSpec) -> Result<()> {
    let path = cache_path(omadac_id).context("Could not resolve home directory")?;
    std::fs::create_dir_all(path.parent().unwrap())
        .context("Failed to create cache directory")?;

    let bytes = rkyv::to_bytes::<_, 1024>(spec).context("Failed to serialize spec")?;

    // Atomic write: write to temp file then rename
    let tmp = path.with_extension("rkyv.tmp");
    std::fs::write(&tmp, &bytes).context("Failed to write cache temp file")?;
    std::fs::rename(&tmp, &path).context("Failed to rename cache file")?;

    Ok(())
}

pub fn delete(omadac_id: &str) -> Result<()> {
    if let Some(path) = cache_path(omadac_id)
        && path.exists()
    {
        std::fs::remove_file(&path).context("Failed to delete cache file")?;
    }
    Ok(())
}
