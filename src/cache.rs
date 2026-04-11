use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::fs;
use std::hash::{
  Hash,
  Hasher,
};
use std::io::Read as _;
use std::path::{
  Path,
  PathBuf,
};

use anyhow::Context as _;
use glob::glob;
use hashbrown::HashMap;
use serde::{
  Deserialize,
  Serialize,
};

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
  task_debug: &str,
  env_vars: &[(String, String)],
  inputs: &[PathBuf],
  env_files: &[PathBuf],
  outputs: &[PathBuf],
) -> anyhow::Result<String> {
  let mut hasher = DefaultHasher::new();
  let mut visited = HashSet::new();

  task_name.hash(&mut hasher);
  task_debug.hash(&mut hasher);
  outputs.hash(&mut hasher);

  for (key, value) in env_vars {
    key.hash(&mut hasher);
    value.hash(&mut hasher);
  }

  for path in inputs {
    path.to_string_lossy().hash(&mut hasher);
    hash_path(path, &mut hasher, &mut visited)?;
  }

  for path in env_files {
    path.to_string_lossy().hash(&mut hasher);
    hash_path(path, &mut hasher, &mut visited)?;
  }

  Ok(format!("{:016x}", hasher.finish()))
}

fn hash_path(path: &Path, hasher: &mut DefaultHasher, visited: &mut HashSet<PathBuf>) -> anyhow::Result<()> {
  let metadata = match fs::symlink_metadata(path) {
    Ok(metadata) => metadata,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
      "missing".hash(hasher);
      return Ok(());
    },
    Err(err) => return Err(err.into()),
  };

  if metadata.file_type().is_symlink() {
    "symlink".hash(hasher);
    let target = fs::read_link(path)
      .map(|target| {
        if target.is_absolute() {
          target
        } else {
          path.parent().unwrap_or(Path::new("")).join(target)
        }
      })
      .unwrap_or_else(|_| PathBuf::from("<unreadable-symlink>"));
    target.to_string_lossy().hash(hasher);

    let resolved = fs::canonicalize(&target).unwrap_or(target);
    if visited.insert(resolved.clone()) {
      hash_path(&resolved, hasher, visited)?;
    }
    return Ok(());
  }

  if metadata.is_file() {
    "file".hash(hasher);
    metadata.len().hash(hasher);

    let mut file = fs::File::open(path)?;
    let mut buffer = [0u8; 8192];

    loop {
      let read = file.read(&mut buffer)?;
      if read == 0 {
        break;
      }
      hasher.write(&buffer[..read]);
    }
  } else if metadata.is_dir() {
    "dir".hash(hasher);

    let mut entries = fs::read_dir(path)?
      .map(|entry| entry.map(|entry| entry.path()))
      .collect::<Result<Vec<_>, _>>()?;

    entries.sort();

    for entry in entries {
      entry.to_string_lossy().hash(hasher);
      hash_path(&entry, hasher, visited)?;
    }
  } else {
    "other".hash(hasher);
    metadata.len().hash(hasher);
    let modified = metadata.modified().ok();
    format!("{modified:?}").hash(hasher);
  }

  Ok(())
}
