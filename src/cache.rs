use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use glob::glob;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::file::ToUtf8 as _;
use crate::utils::resolve_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
  pub fingerprint: String,
  pub outputs: Vec<String>,
  pub updated_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CacheStore {
  pub tasks: HashMap<String, CacheEntry>,
}

impl CacheStore {
  pub fn load() -> anyhow::Result<Self> {
    Self::load_in_dir(Path::new("."))
  }

  pub fn load_in_dir(base_dir: &Path) -> anyhow::Result<Self> {
    let path = cache_path_in_dir(base_dir);
    if !path.exists() {
      return Ok(Self::default());
    }

    let contents = fs::read_to_string(&path).with_context(|| {
      format!(
        "Failed to read cache file - {}",
        path.to_utf8().unwrap_or("<non-utf8-path>")
      )
    })?;
    Ok(serde_json::from_str(&contents)?)
  }

  pub fn save(&self) -> anyhow::Result<()> {
    self.save_in_dir(Path::new("."))
  }

  pub fn save_in_dir(&self, base_dir: &Path) -> anyhow::Result<()> {
    let path = cache_path_in_dir(base_dir);
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }

    fs::write(&path, serde_json::to_string_pretty(self)?).with_context(|| {
      format!(
        "Failed to write cache file - {}",
        path.to_utf8().unwrap_or("<non-utf8-path>")
      )
    })?;
    Ok(())
  }

  pub fn remove() -> anyhow::Result<()> {
    Self::remove_in_dir(Path::new("."))
  }

  pub fn remove_in_dir(base_dir: &Path) -> anyhow::Result<()> {
    let path = cache_path_in_dir(base_dir);
    if path.exists() {
      fs::remove_file(&path).with_context(|| {
        format!(
          "Failed to remove cache file - {}",
          path.to_utf8().unwrap_or("<non-utf8-path>")
        )
      })?;
    }
    Ok(())
  }
}

pub fn cache_path() -> PathBuf {
  cache_path_in_dir(Path::new("."))
}

pub fn cache_path_in_dir(base_dir: &Path) -> PathBuf {
  base_dir.join(".mk").join("cache.json")
}

pub fn expand_patterns(patterns: &[String]) -> anyhow::Result<Vec<PathBuf>> {
  expand_patterns_in_dir(Path::new("."), patterns)
}

pub fn expand_patterns_in_dir(base_dir: &Path, patterns: &[String]) -> anyhow::Result<Vec<PathBuf>> {
  let mut paths = Vec::new();

  for pattern in patterns {
    let mut matched = false;
    let resolved_pattern = resolve_path(base_dir, pattern);
    let resolved_pattern = resolved_pattern.to_string_lossy().into_owned();
    for entry in glob(&resolved_pattern)? {
      matched = true;
      let path = entry?;
      paths.push(path);
    }

    if !matched {
      paths.push(resolve_path(base_dir, pattern));
    }
  }

  paths.sort();
  paths.dedup();
  Ok(paths)
}

pub fn compute_fingerprint(
  task_name: &str,
  task_fingerprint: &str,
  env_vars: &[(String, String)],
  inputs: &[PathBuf],
  env_files: &[PathBuf],
  outputs: &[PathBuf],
) -> anyhow::Result<String> {
  let mut hasher = DefaultHasher::new();

  task_name.hash(&mut hasher);
  task_fingerprint.hash(&mut hasher);

  for (key, value) in env_vars {
    key.hash(&mut hasher);
    value.hash(&mut hasher);
  }

  for path in inputs {
    path.to_string_lossy().hash(&mut hasher);
    hash_path(path, &mut hasher)?;
  }

  for path in env_files {
    path.to_string_lossy().hash(&mut hasher);
    hash_path(path, &mut hasher)?;
  }

  for path in outputs {
    path.to_string_lossy().hash(&mut hasher);
    hash_path(path, &mut hasher)?;
  }

  Ok(format!("{:016x}", hasher.finish()))
}

fn hash_path(path: &Path, hasher: &mut DefaultHasher) -> anyhow::Result<()> {
  if !path.exists() {
    "missing".hash(hasher);
    return Ok(());
  }

  hash_path_contents(path, path, hasher)?;

  Ok(())
}

fn hash_path_contents(root: &Path, path: &Path, hasher: &mut DefaultHasher) -> anyhow::Result<()> {
  let metadata = fs::symlink_metadata(path)?;

  if metadata.file_type().is_symlink() {
    "symlink".hash(hasher);
    fs::read_link(path)?.to_string_lossy().hash(hasher);
    return Ok(());
  }

  if metadata.is_file() {
    "file".hash(hasher);
    metadata.len().hash(hasher);
    let bytes = fs::read(path)?;
    bytes.hash(hasher);
    return Ok(());
  }

  if metadata.is_dir() {
    "dir".hash(hasher);

    let mut entries = fs::read_dir(path)?
      .collect::<Result<Vec<_>, _>>()?
      .into_iter()
      .map(|entry| entry.path())
      .collect::<Vec<_>>();
    entries.sort();

    for entry in entries {
      let relative = entry.strip_prefix(root).unwrap_or(&entry);
      relative.to_string_lossy().hash(hasher);
      hash_path_contents(root, &entry, hasher)?;
    }

    return Ok(());
  }

  "other".hash(hasher);
  metadata.len().hash(hasher);
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use assert_fs::TempDir;

  #[test]
  fn test_compute_fingerprint_changes_when_output_content_changes() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let output = temp_dir.path().join("output.txt");
    fs::write(&output, "one")?;

    let first = compute_fingerprint("build", "task", &[], &[], &[], std::slice::from_ref(&output))?;

    fs::write(&output, "two")?;

    let second = compute_fingerprint("build", "task", &[], &[], &[], &[output])?;
    assert_ne!(first, second);
    Ok(())
  }

  #[test]
  fn test_compute_fingerprint_changes_when_directory_child_changes() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let input_dir = temp_dir.path().join("input");
    fs::create_dir_all(input_dir.join("nested"))?;
    fs::write(input_dir.join("nested/file.txt"), "one")?;

    let first = compute_fingerprint("build", "task", &[], std::slice::from_ref(&input_dir), &[], &[])?;

    fs::write(input_dir.join("nested/file.txt"), "two")?;

    let second = compute_fingerprint("build", "task", &[], &[input_dir], &[], &[])?;
    assert_ne!(first, second);
    Ok(())
  }
}
