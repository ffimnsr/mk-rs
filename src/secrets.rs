use std::env;
use std::fs::{
  self,
  File,
};
use std::io::Write as _;
use std::path::{
  Path,
  PathBuf,
};
use std::process::{
  Command,
  Stdio,
};

use anyhow::Context as _;
use hashbrown::HashMap;
use pgp::composed::{
  Deserializable as _,
  Message,
  SignedSecretKey,
};
use schemars::JsonSchema;
use serde::{
  Deserialize,
  Serialize,
};

use crate::file::ToUtf8 as _;
use crate::utils::{
  parse_env_contents,
  resolve_path,
};

const VAULT_META_FILE: &str = ".vault-meta.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretValueSource {
  Cli,
  Task,
  Root,
  VaultMeta,
  Default,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SecretBackend {
  #[default]
  BuiltInPgp,
  Gpg,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
pub struct SecretSettings {
  #[serde(default)]
  pub backend: Option<SecretBackend>,

  #[serde(default)]
  pub vault_location: Option<String>,

  #[serde(default)]
  pub keys_location: Option<String>,

  #[serde(default)]
  pub key_name: Option<String>,

  #[serde(default)]
  pub gpg_key_id: Option<String>,

  #[serde(default)]
  pub secrets_path: Option<Vec<String>>,
}

impl SecretSettings {
  pub fn is_empty(&self) -> bool {
    self.backend.is_none()
      && self.vault_location.is_none()
      && self.keys_location.is_none()
      && self.key_name.is_none()
      && self.gpg_key_id.is_none()
      && self.secrets_path.is_none()
  }

  pub fn merge(&self, overlay: &Self) -> Self {
    let mut merged = self.clone();
    if overlay.backend.is_some() {
      merged.backend = overlay.backend.clone();
    }
    if overlay.vault_location.is_some() {
      merged.vault_location = overlay.vault_location.clone();
    }
    if overlay.keys_location.is_some() {
      merged.keys_location = overlay.keys_location.clone();
    }
    if overlay.key_name.is_some() {
      merged.key_name = overlay.key_name.clone();
    }
    if overlay.gpg_key_id.is_some() {
      merged.gpg_key_id = overlay.gpg_key_id.clone();
    }
    if overlay.secrets_path.is_some() {
      merged.secrets_path = overlay.secrets_path.clone();
    }
    merged.with_inferred_backend()
  }

  pub fn with_inferred_backend(mut self) -> Self {
    if self.backend.is_none() && self.gpg_key_id.is_some() {
      self.backend = Some(SecretBackend::Gpg);
    }
    self
  }

  pub fn resolved_backend(&self) -> SecretBackend {
    infer_secret_backend(self.backend.clone(), self.gpg_key_id.as_deref())
  }

  pub fn from_legacy(
    vault_location: Option<String>,
    keys_location: Option<String>,
    key_name: Option<String>,
    gpg_key_id: Option<String>,
    secrets_path: Vec<String>,
  ) -> Self {
    let secrets_path = if secrets_path.is_empty() {
      None
    } else {
      Some(secrets_path)
    };

    Self {
      backend: None,
      vault_location,
      keys_location,
      key_name,
      gpg_key_id,
      secrets_path,
    }
    .with_inferred_backend()
  }
}

pub fn merge_optional_secret_settings(
  base: Option<SecretSettings>,
  overlay: Option<SecretSettings>,
) -> Option<SecretSettings> {
  match (base, overlay) {
    (Some(base), Some(overlay)) => Some(base.merge(&overlay)),
    (None, Some(overlay)) => Some(overlay.with_inferred_backend()),
    (Some(base), None) => Some(base.with_inferred_backend()),
    (None, None) => None,
  }
}

pub fn infer_secret_backend(explicit: Option<SecretBackend>, gpg_key_id: Option<&str>) -> SecretBackend {
  explicit.unwrap_or_else(|| {
    if gpg_key_id.is_some() {
      SecretBackend::Gpg
    } else {
      SecretBackend::BuiltInPgp
    }
  })
}

/// Metadata stored inside a vault directory that describes how the vault should be accessed.
/// Written by `mk secrets vault init --gpg-key-id` so subsequent commands
/// (store, show, export, …) pick up the GPG key automatically without flags.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct VaultMeta {
  /// Explicit backend used to access this vault.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub backend: Option<SecretBackend>,

  /// Path to key directory for built-in PGP vaults.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub keys_location: Option<String>,

  /// Key name for built-in PGP vaults.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub key_name: Option<String>,

  /// GPG key ID or fingerprint used to encrypt/decrypt secrets in this vault
  #[serde(skip_serializing_if = "Option::is_none")]
  pub gpg_key_id: Option<String>,
}

pub fn read_vault_meta(vault_location: &Path) -> Option<VaultMeta> {
  let content = fs::read_to_string(vault_location.join(VAULT_META_FILE)).ok()?;
  toml::from_str(&content).ok()
}

/// Read the GPG key ID stored in a vault's metadata file, if present.
/// Returns `None` when the file does not exist or cannot be parsed.
pub fn read_vault_gpg_key_id(vault_location: &Path) -> Option<String> {
  read_vault_meta(vault_location)?.gpg_key_id
}

pub fn read_vault_backend(vault_location: &Path) -> Option<SecretBackend> {
  read_vault_meta(vault_location)?.backend
}

/// Write (or overwrite) the vault's metadata file with the supplied settings.
pub fn write_vault_meta(vault_location: &Path, meta: &VaultMeta) -> anyhow::Result<()> {
  let content = toml::to_string_pretty(&meta).context("Failed to serialize vault metadata")?;
  let meta_path = vault_location.join(VAULT_META_FILE);
  let mut file = File::create(&meta_path)?;
  file.write_all(content.as_bytes())?;
  file.flush()?;
  Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretConfig {
  pub backend: SecretBackend,
  pub vault_location: PathBuf,
  pub keys_location: PathBuf,
  pub key_name: String,
  pub gpg_key_id: Option<String>,
  pub secrets_path: Vec<String>,
  pub backend_source: SecretValueSource,
  pub vault_location_source: SecretValueSource,
  pub keys_location_source: SecretValueSource,
  pub key_name_source: SecretValueSource,
  pub gpg_key_id_source: Option<SecretValueSource>,
  pub secrets_path_source: Option<SecretValueSource>,
  pub vault_meta_used: bool,
}

impl SecretConfig {
  pub fn with_secrets_path(mut self, secrets_path: Vec<String>, source: Option<SecretValueSource>) -> Self {
    self.secrets_path = secrets_path;
    self.secrets_path_source = source;
    self
  }
}

fn pick_setting<'a, T: ?Sized>(
  cli_value: Option<&'a T>,
  task_value: Option<&'a T>,
  root_value: Option<&'a T>,
  meta_value: Option<&'a T>,
  default_value: &'a T,
) -> (&'a T, SecretValueSource) {
  if let Some(value) = cli_value {
    return (value, SecretValueSource::Cli);
  }
  if let Some(value) = task_value {
    return (value, SecretValueSource::Task);
  }
  if let Some(value) = root_value {
    return (value, SecretValueSource::Root);
  }
  if let Some(value) = meta_value {
    return (value, SecretValueSource::VaultMeta);
  }
  (default_value, SecretValueSource::Default)
}

pub fn resolve_secret_config(
  base_dir: &Path,
  cli_overrides: Option<&SecretSettings>,
  task_settings: Option<&SecretSettings>,
  root_settings: Option<&SecretSettings>,
) -> SecretConfig {
  let default_vault_location = default_vault_location(base_dir);
  let cli_vault_location = cli_overrides.and_then(|settings| settings.vault_location.as_deref());
  let task_vault_location = task_settings.and_then(|settings| settings.vault_location.as_deref());
  let root_vault_location = root_settings.and_then(|settings| settings.vault_location.as_deref());

  let vault_location = cli_vault_location
    .or(task_vault_location)
    .or(root_vault_location)
    .map(|path| resolve_path(base_dir, path))
    .unwrap_or_else(|| default_vault_location.clone());
  let vault_location_source = if cli_vault_location.is_some() {
    SecretValueSource::Cli
  } else if task_vault_location.is_some() {
    SecretValueSource::Task
  } else if root_vault_location.is_some() {
    SecretValueSource::Root
  } else {
    SecretValueSource::Default
  };

  let vault_meta = read_vault_meta(&vault_location);

  let default_backend = SecretBackend::BuiltInPgp;
  let (backend_value, backend_source) = pick_setting(
    cli_overrides.and_then(|settings| settings.backend.as_ref()),
    task_settings.and_then(|settings| settings.backend.as_ref()),
    root_settings.and_then(|settings| settings.backend.as_ref()),
    vault_meta.as_ref().and_then(|meta| meta.backend.as_ref()),
    &default_backend,
  );

  let default_keys_location = default_keys_location();
  let default_keys_location_str = default_keys_location.to_string_lossy().to_string();
  let (keys_location, keys_location_source) = pick_setting(
    cli_overrides.and_then(|settings| settings.keys_location.as_deref()),
    task_settings.and_then(|settings| settings.keys_location.as_deref()),
    root_settings.and_then(|settings| settings.keys_location.as_deref()),
    vault_meta.as_ref().and_then(|meta| meta.keys_location.as_deref()),
    default_keys_location_str.as_str(),
  );

  let default_key_name = String::from("default");
  let (key_name, key_name_source) = pick_setting(
    cli_overrides.and_then(|settings| settings.key_name.as_deref()),
    task_settings.and_then(|settings| settings.key_name.as_deref()),
    root_settings.and_then(|settings| settings.key_name.as_deref()),
    vault_meta.as_ref().and_then(|meta| meta.key_name.as_deref()),
    default_key_name.as_str(),
  );

  let gpg_key_id = cli_overrides
    .and_then(|settings| settings.gpg_key_id.as_ref())
    .map(|value| (value.clone(), SecretValueSource::Cli))
    .or_else(|| {
      task_settings
        .and_then(|settings| settings.gpg_key_id.as_ref())
        .map(|value| (value.clone(), SecretValueSource::Task))
    })
    .or_else(|| {
      root_settings
        .and_then(|settings| settings.gpg_key_id.as_ref())
        .map(|value| (value.clone(), SecretValueSource::Root))
    })
    .or_else(|| {
      vault_meta
        .as_ref()
        .and_then(|meta| meta.gpg_key_id.as_ref())
        .map(|value| (value.clone(), SecretValueSource::VaultMeta))
    });

  let secrets_path = task_settings
    .and_then(|settings| {
      settings
        .secrets_path
        .clone()
        .map(|paths| (paths, SecretValueSource::Task))
    })
    .or_else(|| {
      root_settings.and_then(|settings| {
        settings
          .secrets_path
          .clone()
          .map(|paths| (paths, SecretValueSource::Root))
      })
    });

  let backend = infer_secret_backend(
    Some(backend_value.clone()),
    gpg_key_id.as_ref().map(|(value, _)| value.as_str()),
  );

  SecretConfig {
    backend,
    vault_location,
    keys_location: resolve_path(base_dir, keys_location),
    key_name: key_name.to_string(),
    gpg_key_id: gpg_key_id.as_ref().map(|(value, _)| value.clone()),
    secrets_path: secrets_path
      .as_ref()
      .map(|(paths, _)| paths.clone())
      .unwrap_or_default(),
    backend_source,
    vault_location_source,
    keys_location_source,
    key_name_source,
    gpg_key_id_source: gpg_key_id.as_ref().map(|(_, source)| *source),
    secrets_path_source: secrets_path.as_ref().map(|(_, source)| *source),
    vault_meta_used: vault_meta.is_some(),
  }
}

pub fn load_secret_values(path: &str, config: &SecretConfig) -> anyhow::Result<Vec<String>> {
  verify_vault(&config.vault_location)?;

  let secret_path = config.vault_location.join(path);
  if !secret_path.exists() || !secret_path.is_dir() {
    anyhow::bail!(
      "Secret '{}' not found in vault. List available secrets with: mk secrets vault list",
      path
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

  let use_gpg = matches!(config.backend, SecretBackend::Gpg);
  let signed_secret_key = if !use_gpg {
    Some(load_secret_key(config)?)
  } else {
    check_gpg_available()?;
    None
  };

  let mut values = Vec::with_capacity(data_paths.len());
  for data_path in data_paths {
    let value = if use_gpg {
      decrypt_with_gpg(
        &data_path,
        config
          .gpg_key_id
          .as_deref()
          .ok_or_else(|| anyhow::anyhow!("GPG backend selected but no gpg_key_id is configured"))?,
      )?
    } else {
      let key = signed_secret_key.as_ref().unwrap();
      let mut data_file = std::io::BufReader::new(File::open(&data_path)?);
      let (message, _) = Message::from_armor(&mut data_file)?;
      let mut decrypted_message = message.decrypt(&pgp::types::Password::empty(), key)?;
      decrypted_message
        .as_data_string()
        .context("Failed to read secret value")?
    };
    values.push(value);
  }

  if values.is_empty() {
    anyhow::bail!(
      "No secrets found for path '{}'. List available secrets with: mk secrets vault list",
      path
    );
  }

  Ok(values)
}

pub fn load_secret_value(path: &str, config: &SecretConfig) -> anyhow::Result<String> {
  let values = load_secret_values(path, config)?;
  match values.as_slice() {
    [value] => Ok(value.clone()),
    [] => anyhow::bail!(
      "No secrets found for path '{}'. List available secrets with: mk secrets vault list",
      path
    ),
    _ => anyhow::bail!(
      "Secret path '{}' resolved to multiple values; use a more specific identifier",
      path
    ),
  }
}

pub fn list_secret_paths(path_prefix: Option<&str>, config: &SecretConfig) -> anyhow::Result<Vec<String>> {
  verify_vault(&config.vault_location)?;

  let root = match path_prefix {
    Some(path_prefix) if !path_prefix.is_empty() => config.vault_location.join(path_prefix),
    _ => config.vault_location.clone(),
  };

  if !root.exists() || !root.is_dir() {
    anyhow::bail!(
      "Secret prefix '{}' not found in vault. List available secrets with: mk secrets vault list",
      path_prefix.unwrap_or("<unknown>")
    );
  }

  let mut secret_paths = Vec::new();
  collect_secret_paths(&config.vault_location, &root, &mut secret_paths)?;
  secret_paths.sort();
  secret_paths.dedup();
  Ok(secret_paths)
}

pub fn load_secret_env(config: &SecretConfig) -> anyhow::Result<HashMap<String, String>> {
  let mut env_vars = HashMap::new();

  for path in &config.secrets_path {
    for value in load_secret_values(path, config)? {
      env_vars.extend(parse_env_contents(&value));
    }
  }

  Ok(env_vars)
}

/// Checks that the `gpg` binary is available in PATH, returning a clear error if not.
/// Called early when any GPG-backend vault operation is attempted.
fn check_gpg_available() -> anyhow::Result<()> {
  which::which("gpg")
    .context("gpg is not available in PATH — install GnuPG to use hardware key (YubiKey) support")?;
  Ok(())
}

fn default_vault_location(base_dir: &Path) -> PathBuf {
  resolve_path(base_dir, "./.mk/vault")
}

/// Encrypt `plaintext` using the system `gpg` binary for the given key ID or fingerprint.
/// The output is ASCII-armored PGP data suitable for storing as a `data.asc` vault file.
pub fn encrypt_with_gpg(gpg_key_id: &str, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
  check_gpg_available()?;
  let mut child = Command::new("gpg")
    .args([
      "--batch",
      "--yes",
      "--armor",
      "--encrypt",
      "--recipient",
      gpg_key_id,
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("Failed to spawn gpg — is it installed and in PATH?")?;

  if let Some(mut stdin) = child.stdin.take() {
    stdin
      .write_all(plaintext)
      .context("Failed to write plaintext to gpg stdin")?;
  }

  let output = child
    .wait_with_output()
    .context("Failed to wait for gpg encrypt")?;
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::bail!("gpg encryption failed: {}", stderr.trim());
  }
  Ok(output.stdout)
}

/// Decrypt a vault `data.asc` file using the system `gpg` binary.
/// GPG-agent handles PIN/passphrase prompts automatically (including YubiKey via pinentry).
fn decrypt_with_gpg(data_path: &Path, _gpg_key_id: &str) -> anyhow::Result<String> {
  let path_str = data_path
    .to_str()
    .ok_or_else(|| anyhow::anyhow!("Non-UTF-8 path: {:?}", data_path))?;

  let output = Command::new("gpg")
    .args(["--batch", "--decrypt", path_str])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("Failed to spawn gpg — is it installed and in PATH?")?
    .wait_with_output()
    .context("Failed to wait for gpg decrypt")?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::bail!("gpg decryption failed: {}", stderr.trim());
  }
  String::from_utf8(output.stdout).context("gpg decrypt output is not valid UTF-8")
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

pub fn verify_vault(vault_location: &Path) -> anyhow::Result<()> {
  if !vault_location.exists() || !vault_location.is_dir() {
    anyhow::bail!(
      "Vault not found at '{}'. Initialize it first with: mk secrets vault init",
      vault_location.to_utf8().unwrap_or("<non-utf8-path>")
    );
  }

  Ok(())
}

fn load_secret_key(config: &SecretConfig) -> anyhow::Result<SignedSecretKey> {
  if !config.keys_location.exists() || !config.keys_location.is_dir() {
    anyhow::bail!(
      "Keys directory not found at '{}'. Generate a key first with: mk secrets key gen",
      config.keys_location.to_utf8().unwrap_or("<non-utf8-path>")
    );
  }

  let key_path = config.keys_location.join(format!("{}.key", config.key_name));
  if !key_path.exists() || !key_path.is_file() {
    anyhow::bail!(
      "Key '{}' not found in '{}'. Generate it with: mk secrets key gen --name {}",
      config.key_name,
      config.keys_location.to_utf8().unwrap_or("<non-utf8-path>"),
      config.key_name
    );
  }

  let mut secret_key_string = File::open(key_path)?;
  let (signed_secret_key, _) = SignedSecretKey::from_armor_single(&mut secret_key_string)?;
  signed_secret_key.verify_bindings()?;
  Ok(signed_secret_key)
}

fn collect_secret_paths(vault_root: &Path, dir: &Path, secret_paths: &mut Vec<String>) -> anyhow::Result<()> {
  let data_path = dir.join("data.asc");
  if data_path.exists() && data_path.is_file() {
    let relative = dir.strip_prefix(vault_root).map_err(|_| {
      let dir = dir.to_utf8().unwrap_or("<non-utf8-path>");
      let vault_root = vault_root.to_utf8().unwrap_or("<non-utf8-path>");
      anyhow::anyhow!(
        "Failed to resolve secret path '{}' relative to vault root '{}'",
        dir,
        vault_root
      )
    })?;
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

#[cfg(test)]
mod tests {
  use std::fs;

  use assert_fs::TempDir;

  use super::*;

  // ── VaultMeta / write_vault_meta / read_vault_gpg_key_id ──────────────────

  #[test]
  fn test_vault_meta_roundtrip() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path();

    // Nothing written yet → returns None
    assert_eq!(read_vault_gpg_key_id(vault_dir), None);

    // Write a key ID
    write_vault_meta(
      vault_dir,
      &VaultMeta {
        backend: Some(SecretBackend::Gpg),
        keys_location: None,
        key_name: None,
        gpg_key_id: Some("ABC123DEF456".to_string()),
      },
    )
    .unwrap();

    // Read it back
    assert_eq!(read_vault_gpg_key_id(vault_dir), Some("ABC123DEF456".to_string()));
    assert_eq!(read_vault_backend(vault_dir), Some(SecretBackend::Gpg));
  }

  #[test]
  fn test_vault_meta_overwrite() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path();

    write_vault_meta(
      vault_dir,
      &VaultMeta {
        backend: Some(SecretBackend::Gpg),
        keys_location: None,
        key_name: None,
        gpg_key_id: Some("FIRST_KEY".to_string()),
      },
    )
    .unwrap();
    write_vault_meta(
      vault_dir,
      &VaultMeta {
        backend: Some(SecretBackend::Gpg),
        keys_location: None,
        key_name: None,
        gpg_key_id: Some("SECOND_KEY".to_string()),
      },
    )
    .unwrap();

    assert_eq!(read_vault_gpg_key_id(vault_dir), Some("SECOND_KEY".to_string()));
  }

  #[test]
  fn test_read_vault_gpg_key_id_missing_file() {
    let dir = TempDir::new().unwrap();
    assert_eq!(read_vault_gpg_key_id(dir.path()), None);
  }

  #[test]
  fn test_read_vault_gpg_key_id_invalid_toml() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(VAULT_META_FILE), b"not_valid [ toml {{").unwrap();
    // Should return None gracefully, no panic
    assert_eq!(read_vault_gpg_key_id(dir.path()), None);
  }

  #[test]
  fn test_verify_vault_accepts_existing_directory() {
    let dir = TempDir::new().unwrap();

    verify_vault(dir.path()).unwrap();
  }

  #[test]
  fn test_verify_vault_rejects_missing_directory() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("missing-vault");

    let error = verify_vault(&missing).unwrap_err();

    assert!(
      error.to_string().contains("Vault not found at '"),
      "unexpected error: {error}"
    );
    assert!(
      error
        .to_string()
        .contains("Initialize it first with: mk secrets vault init"),
      "unexpected error: {error}"
    );
  }

  // ── resolve_secret_config ────────────────────────────────────────────────

  #[test]
  fn test_secret_config_explicit_gpg_key_id() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    let base = Path::new(".");
    let config = resolve_secret_config(
      base,
      Some(&SecretSettings {
        backend: Some(SecretBackend::Gpg),
        vault_location: Some(vault_dir.to_string()),
        keys_location: None,
        key_name: None,
        gpg_key_id: Some("EXPLICIT_ID".to_string()),
        secrets_path: None,
      }),
      None,
      None,
    );
    assert_eq!(config.gpg_key_id, Some("EXPLICIT_ID".to_string()));
    assert_eq!(config.backend, SecretBackend::Gpg);
  }

  #[test]
  fn test_secret_config_gpg_key_id_from_vault_metadata() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    write_vault_meta(
      dir.path(),
      &VaultMeta {
        backend: Some(SecretBackend::Gpg),
        keys_location: None,
        key_name: None,
        gpg_key_id: Some("META_ID".to_string()),
      },
    )
    .unwrap();

    let base = Path::new(".");
    let config = resolve_secret_config(
      base,
      Some(&SecretSettings {
        backend: None,
        vault_location: Some(vault_dir.to_string()),
        keys_location: None,
        key_name: None,
        gpg_key_id: None,
        secrets_path: None,
      }),
      None,
      None,
    );
    assert_eq!(config.gpg_key_id, Some("META_ID".to_string()));
    assert_eq!(config.backend, SecretBackend::Gpg);
    assert_eq!(config.gpg_key_id_source, Some(SecretValueSource::VaultMeta));
  }

  #[test]
  fn test_secret_config_root_settings_allow_vault_metadata_backend() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    write_vault_meta(
      dir.path(),
      &VaultMeta {
        backend: Some(SecretBackend::Gpg),
        keys_location: None,
        key_name: None,
        gpg_key_id: Some("META_ID".to_string()),
      },
    )
    .unwrap();

    let config = resolve_secret_config(
      Path::new("."),
      None,
      None,
      Some(&SecretSettings {
        backend: None,
        vault_location: Some(vault_dir.to_string()),
        keys_location: None,
        key_name: None,
        gpg_key_id: None,
        secrets_path: None,
      }),
    );

    assert_eq!(config.backend, SecretBackend::Gpg);
    assert_eq!(config.backend_source, SecretValueSource::VaultMeta);
    assert_eq!(config.gpg_key_id.as_deref(), Some("META_ID"));
  }

  #[test]
  fn test_secret_config_explicit_gpg_key_id_overrides_metadata() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    write_vault_meta(
      dir.path(),
      &VaultMeta {
        backend: Some(SecretBackend::Gpg),
        keys_location: None,
        key_name: None,
        gpg_key_id: Some("META_ID".to_string()),
      },
    )
    .unwrap();

    let base = Path::new(".");
    let config = resolve_secret_config(
      base,
      Some(&SecretSettings {
        backend: Some(SecretBackend::Gpg),
        vault_location: Some(vault_dir.to_string()),
        keys_location: None,
        key_name: None,
        gpg_key_id: Some("EXPLICIT_ID".to_string()),
        secrets_path: None,
      }),
      None,
      None,
    );
    // Explicit arg wins over metadata
    assert_eq!(config.gpg_key_id, Some("EXPLICIT_ID".to_string()));
    assert_eq!(config.gpg_key_id_source, Some(SecretValueSource::Cli));
  }

  #[test]
  fn test_secret_config_no_gpg_key_id() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    let base = Path::new(".");
    // Empty vault dir — no .vault-meta.toml written
    let config = resolve_secret_config(
      base,
      Some(&SecretSettings {
        backend: None,
        vault_location: Some(vault_dir.to_string()),
        keys_location: None,
        key_name: None,
        gpg_key_id: None,
        secrets_path: None,
      }),
      None,
      None,
    );
    assert_eq!(config.gpg_key_id, None);
    assert_eq!(config.backend, SecretBackend::BuiltInPgp);
  }

  #[test]
  fn test_secret_config_key_name_default() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    let base = Path::new(".");
    let config = resolve_secret_config(
      base,
      Some(&SecretSettings {
        backend: None,
        vault_location: Some(vault_dir.to_string()),
        keys_location: None,
        key_name: None,
        gpg_key_id: None,
        secrets_path: None,
      }),
      None,
      None,
    );
    assert_eq!(config.key_name, "default");
  }

  #[test]
  fn test_secret_config_key_name_custom() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    let base = Path::new(".");
    let config = resolve_secret_config(
      base,
      Some(&SecretSettings {
        backend: None,
        vault_location: Some(vault_dir.to_string()),
        keys_location: None,
        key_name: Some("mykey".to_string()),
        gpg_key_id: None,
        secrets_path: None,
      }),
      None,
      None,
    );
    assert_eq!(config.key_name, "mykey");
  }

  #[test]
  fn test_secret_settings_merge_prefers_overlay() {
    let base = SecretSettings {
      backend: Some(SecretBackend::BuiltInPgp),
      vault_location: Some("root-vault".to_string()),
      keys_location: Some("root-keys".to_string()),
      key_name: Some("root".to_string()),
      gpg_key_id: None,
      secrets_path: Some(vec!["root/path".to_string()]),
    };
    let overlay = SecretSettings {
      backend: Some(SecretBackend::Gpg),
      vault_location: None,
      keys_location: None,
      key_name: None,
      gpg_key_id: Some("KEYID".to_string()),
      secrets_path: Some(vec!["task/path".to_string()]),
    };

    let merged = base.merge(&overlay);
    assert_eq!(merged.backend, Some(SecretBackend::Gpg));
    assert_eq!(merged.vault_location.as_deref(), Some("root-vault"));
    assert_eq!(merged.gpg_key_id.as_deref(), Some("KEYID"));
    assert_eq!(merged.secrets_path, Some(vec!["task/path".to_string()]));
  }

  // ── VaultMeta serialization ───────────────────────────────────────────────

  #[test]
  fn test_vault_meta_toml_no_gpg_key_id() {
    // When gpg_key_id is None, the field is skipped in the TOML output
    let meta = VaultMeta {
      backend: None,
      keys_location: None,
      key_name: None,
      gpg_key_id: None,
    };
    let s = toml::to_string_pretty(&meta).unwrap();
    assert!(!s.contains("gpg_key_id"), "unexpected field in: {s}");
  }

  #[test]
  fn test_vault_meta_toml_with_gpg_key_id() {
    let meta = VaultMeta {
      backend: Some(SecretBackend::Gpg),
      keys_location: Some("./keys".to_string()),
      key_name: Some("vault".to_string()),
      gpg_key_id: Some("FINGERPRINT".to_string()),
    };
    let s = toml::to_string_pretty(&meta).unwrap();
    assert!(s.contains("backend"), "field missing from: {s}");
    assert!(s.contains("keys_location"), "field missing from: {s}");
    assert!(s.contains("key_name"), "field missing from: {s}");
    assert!(s.contains("gpg_key_id"), "field missing from: {s}");
    assert!(s.contains("FINGERPRINT"));
  }
}
