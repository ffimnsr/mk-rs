use std::env;
use std::fs::{
  self,
  File,
};
use std::path::{
  Path,
  PathBuf,
};

use anyhow::Context as _;
use hashbrown::HashMap;
use pgp::composed::{
  Deserializable as _,
  Message,
  SignedSecretKey,
};

use crate::file::ToUtf8 as _;
use crate::utils::{
  parse_env_contents,
  resolve_path,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretConfig {
  pub vault_location: PathBuf,
  pub keys_location: PathBuf,
  pub key_name: String,
}

impl SecretConfig {
  pub fn resolve(
    base_dir: &Path,
    vault_location: Option<&str>,
    keys_location: Option<&str>,
    key_name: Option<&str>,
  ) -> Self {
    let vault_location = vault_location
      .map(|path| resolve_path(base_dir, path))
      .unwrap_or_else(|| default_vault_location(base_dir));
    let keys_location = keys_location
      .map(|path| resolve_path(base_dir, path))
      .unwrap_or_else(default_keys_location);
    let key_name = key_name.unwrap_or("default").to_string();

    Self {
      vault_location,
      keys_location,
      key_name,
    }
  }
}

pub fn load_secret_values(
  path: &str,
  base_dir: &Path,
  vault_location: Option<&str>,
  keys_location: Option<&str>,
  key_name: Option<&str>,
) -> anyhow::Result<Vec<String>> {
  let config = SecretConfig::resolve(base_dir, vault_location, keys_location, key_name);
  verify_vault(&config.vault_location)?;
  let signed_secret_key = load_secret_key(&config)?;

  let secret_path = config.vault_location.join(path);
  if !secret_path.exists() || !secret_path.is_dir() {
    anyhow::bail!(
      "Secret path does not exist: {}",
      secret_path.to_utf8().unwrap_or("<non-utf8-path>")
    );
  }

  let mut data_paths = fs::read_dir(&secret_path)?
    .filter_map(Result::ok)
    .map(|entry| {
      if entry.path().is_dir() {
        entry.path().join("data.asc")
      } else {
        entry.path()
      }
    })
    .filter(|path| path.exists() && path.is_file())
    .collect::<Vec<_>>();
  data_paths.sort();

  let mut values = Vec::with_capacity(data_paths.len());
  for data_path in data_paths {
    let mut data_file = std::io::BufReader::new(File::open(data_path)?);
    let (message, _) = Message::from_armor(&mut data_file)?;
    let mut decrypted_message = message.decrypt(&pgp::types::Password::empty(), &signed_secret_key)?;
    let value = decrypted_message
      .as_data_string()
      .context("Failed to read secret value")?;
    values.push(value);
  }

  if values.is_empty() {
    anyhow::bail!("No secrets found for path: {path}");
  }

  Ok(values)
}

pub fn load_secret_value(
  path: &str,
  base_dir: &Path,
  vault_location: Option<&str>,
  keys_location: Option<&str>,
  key_name: Option<&str>,
) -> anyhow::Result<String> {
  let values = load_secret_values(path, base_dir, vault_location, keys_location, key_name)?;
  match values.as_slice() {
    [value] => Ok(value.clone()),
    [] => anyhow::bail!("No secrets found for path: {path}"),
    _ => anyhow::bail!("Secret path resolved to multiple values: {path}"),
  }
}

pub fn list_secret_paths(
  path_prefix: Option<&str>,
  base_dir: &Path,
  vault_location: Option<&str>,
) -> anyhow::Result<Vec<String>> {
  let config = SecretConfig::resolve(base_dir, vault_location, None, None);
  verify_vault(&config.vault_location)?;

  let root = match path_prefix {
    Some(path_prefix) if !path_prefix.is_empty() => config.vault_location.join(path_prefix),
    _ => config.vault_location.clone(),
  };

  if !root.exists() || !root.is_dir() {
    anyhow::bail!(
      "Secret path does not exist: {}",
      root.to_utf8().unwrap_or("<non-utf8-path>")
    );
  }

  let mut secret_paths = Vec::new();
  collect_secret_paths(&config.vault_location, &root, &mut secret_paths)?;
  secret_paths.sort();
  secret_paths.dedup();
  Ok(secret_paths)
}

pub fn load_secret_env(
  paths: &[String],
  base_dir: &Path,
  vault_location: Option<&str>,
  keys_location: Option<&str>,
  key_name: Option<&str>,
) -> anyhow::Result<HashMap<String, String>> {
  let mut env_vars = HashMap::new();

  for path in paths {
    for value in load_secret_values(path, base_dir, vault_location, keys_location, key_name)? {
      env_vars.extend(parse_env_contents(&value));
    }
  }

  Ok(env_vars)
}

fn default_vault_location(base_dir: &Path) -> PathBuf {
  resolve_path(base_dir, "./.mk/vault")
}

fn default_keys_location() -> PathBuf {
  let home_dir = if cfg!(target_os = "windows") {
    env::var("USERPROFILE").unwrap_or_else(|_| "./.mk/priv".to_string())
  } else {
    env::var("HOME").unwrap_or_else(|_| "./.mk/priv".to_string())
  };

  let mut path = PathBuf::from(home_dir);
  path.push(".config");
  path.push("mk");
  path.push("priv");
  path
}

fn verify_vault(vault_location: &Path) -> anyhow::Result<()> {
  if !vault_location.exists() || !vault_location.is_dir() {
    anyhow::bail!("The store does not exist");
  }

  Ok(())
}

fn load_secret_key(config: &SecretConfig) -> anyhow::Result<SignedSecretKey> {
  if !config.keys_location.exists() || !config.keys_location.is_dir() {
    anyhow::bail!("The keys location does not exist");
  }

  let key_path = config.keys_location.join(format!("{}.key", config.key_name));
  if !key_path.exists() || !key_path.is_file() {
    anyhow::bail!("The key does not exist");
  }

  let mut secret_key_string = File::open(key_path)?;
  let (signed_secret_key, _) = SignedSecretKey::from_armor_single(&mut secret_key_string)?;
  signed_secret_key.verify()?;
  Ok(signed_secret_key)
}

fn collect_secret_paths(vault_root: &Path, dir: &Path, secret_paths: &mut Vec<String>) -> anyhow::Result<()> {
  let data_path = dir.join("data.asc");
  if data_path.exists() && data_path.is_file() {
    let relative = dir
      .strip_prefix(vault_root)
      .map_err(|_| anyhow::anyhow!("Failed to resolve secret path relative to vault"))?;
    secret_paths.push(relative.to_utf8().unwrap_or("<non-utf8-path>").to_string());
  }

  for entry in fs::read_dir(dir)?.filter_map(Result::ok) {
    let path = entry.path();
    if path.is_dir() {
      collect_secret_paths(vault_root, &path, secret_paths)?;
    }
  }

  Ok(())
}
