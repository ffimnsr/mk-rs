use std::path::{
  Component,
  Path,
  PathBuf,
};
use std::{
  fmt,
  fs,
};

use anyhow::Context as _;
use hashbrown::HashMap;
use serde::de::{
  self,
  MapAccess,
  Visitor,
};
use serde::{
  Deserialize,
  Deserializer,
};
use serde_json::Value as JsonValue;

use crate::file::ToUtf8 as _;

#[allow(dead_code)]
#[derive(Debug)]
enum AnyValue {
  String(String),
  Number(serde_json::Number),
  Bool(bool),
}

impl fmt::Display for AnyValue {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      AnyValue::String(s) => write!(f, "{}", s),
      AnyValue::Number(n) => write!(f, "{}", n),
      AnyValue::Bool(b) => write!(f, "{}", b),
    }
  }
}

impl<'de> Deserialize<'de> for AnyValue {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value: JsonValue = Deserialize::deserialize(deserializer)?;
    match value {
      JsonValue::String(s) => Ok(AnyValue::String(s)),
      JsonValue::Number(n) => Ok(AnyValue::Number(n)),
      JsonValue::Bool(b) => Ok(AnyValue::Bool(b)),
      _ => Err(de::Error::custom("expected a string, number, or boolean")),
    }
  }
}

pub(crate) fn deserialize_environment<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
  D: Deserializer<'de>,
{
  struct EnvironmentVisitor;

  impl<'de> Visitor<'de> for EnvironmentVisitor {
    type Value = HashMap<String, String>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      formatter.write_str("a map of strings to any value (string, int, or bool)")
    }

    fn visit_map<M>(self, mut access: M) -> Result<HashMap<String, String>, M::Error>
    where
      M: MapAccess<'de>,
    {
      let mut map = HashMap::new();
      while let Some((key, value)) = access.next_entry::<String, AnyValue>()? {
        map.insert(key, value.to_string());
      }
      Ok(map)
    }
  }

  deserializer.deserialize_map(EnvironmentVisitor)
}

pub(crate) fn resolve_path(base_dir: &Path, value: &str) -> PathBuf {
  let path = Path::new(value);
  let joined = if path.is_absolute() {
    path.to_path_buf()
  } else {
    base_dir.join(path)
  };

  normalize_path(&joined)
}

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
  let mut normalized = PathBuf::new();

  for component in path.components() {
    match component {
      Component::CurDir => {},
      Component::ParentDir => {
        normalized.pop();
      },
      other => normalized.push(other.as_os_str()),
    }
  }

  normalized
}

pub(crate) fn load_env_files_in_dir(
  env_files: &[String],
  base_dir: &Path,
) -> anyhow::Result<HashMap<String, String>> {
  let mut local_env: HashMap<String, String> = HashMap::new();
  for env_file in env_files {
    let path = resolve_path(base_dir, env_file);
    let contents = fs::read_to_string(&path).with_context(|| {
      format!(
        "Failed to read env file - {}",
        path.to_utf8().unwrap_or("<non-utf8-path>")
      )
    })?;

    local_env.extend(parse_env_contents(&contents));
  }

  Ok(local_env)
}

pub(crate) fn parse_env_contents(contents: &str) -> HashMap<String, String> {
  let mut env_vars = HashMap::new();

  for line in contents.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }

    if let Some((key, value)) = line.split_once('=') {
      env_vars.insert(key.trim().to_string(), value.trim().to_string());
    }
  }

  env_vars
}
