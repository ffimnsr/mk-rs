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

/// Metadata stored inside a vault directory that describes how the vault should be accessed.
/// Written by `mk secrets vault init --gpg-key-id` so subsequent commands
/// (store, show, export, …) pick up the GPG key automatically without flags.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct VaultMeta {
  /// GPG key ID or fingerprint used to encrypt/decrypt secrets in this vault
  #[serde(skip_serializing_if = "Option::is_none")]
  pub gpg_key_id: Option<String>,
}

/// Read the GPG key ID stored in a vault's metadata file, if present.
/// Returns `None` when the file does not exist or cannot be parsed.
pub fn read_vault_gpg_key_id(vault_location: &Path) -> Option<String> {
  let content = fs::read_to_string(vault_location.join(VAULT_META_FILE)).ok()?;
  let meta: VaultMeta = toml::from_str(&content).ok()?;
  meta.gpg_key_id
}

/// Write (or overwrite) the vault's metadata file with the supplied GPG key ID.
pub fn write_vault_meta(vault_location: &Path, gpg_key_id: &str) -> anyhow::Result<()> {
  let meta = VaultMeta {
    gpg_key_id: Some(gpg_key_id.to_string()),
  };
  let content = toml::to_string_pretty(&meta).context("Failed to serialize vault metadata")?;
  let meta_path = vault_location.join(VAULT_META_FILE);
  let mut file = File::create(&meta_path)?;
  file.write_all(content.as_bytes())?;
  file.flush()?;
  Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretConfig {
  pub vault_location: PathBuf,
  pub keys_location: PathBuf,
  pub key_name: String,
  pub gpg_key_id: Option<String>,
}

impl SecretConfig {
  pub fn resolve(
    base_dir: &Path,
    vault_location: Option<&str>,
    keys_location: Option<&str>,
    key_name: Option<&str>,
    gpg_key_id: Option<&str>,
  ) -> Self {
    let vault_location = vault_location
      .map(|path| resolve_path(base_dir, path))
      .unwrap_or_else(|| default_vault_location(base_dir));
    let keys_location = keys_location
      .map(|path| resolve_path(base_dir, path))
      .unwrap_or_else(default_keys_location);
    let key_name = key_name.unwrap_or("default").to_string();
    // Resolve gpg_key_id: explicit argument > vault metadata file
    let gpg_key_id = gpg_key_id
      .map(|s| s.to_string())
      .or_else(|| read_vault_gpg_key_id(&vault_location));

    Self {
      vault_location,
      keys_location,
      key_name,
      gpg_key_id,
    }
  }
}

pub fn load_secret_values(
  path: &str,
  base_dir: &Path,
  vault_location: Option<&str>,
  keys_location: Option<&str>,
  key_name: Option<&str>,
  gpg_key_id: Option<&str>,
) -> anyhow::Result<Vec<String>> {
  let config = SecretConfig::resolve(base_dir, vault_location, keys_location, key_name, gpg_key_id);
  verify_vault(&config.vault_location)?;

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

  let use_gpg = config.gpg_key_id.is_some();
  let signed_secret_key = if !use_gpg {
    Some(load_secret_key(&config)?)
  } else {
    check_gpg_available()?;
    None
  };

  let mut values = Vec::with_capacity(data_paths.len());
  for data_path in data_paths {
    let value = if use_gpg {
      decrypt_with_gpg(&data_path)?
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
  gpg_key_id: Option<&str>,
) -> anyhow::Result<String> {
  let values = load_secret_values(
    path,
    base_dir,
    vault_location,
    keys_location,
    key_name,
    gpg_key_id,
  )?;
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
  let config = SecretConfig::resolve(base_dir, vault_location, None, None, None);
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
  gpg_key_id: Option<&str>,
) -> anyhow::Result<HashMap<String, String>> {
  let mut env_vars = HashMap::new();

  for path in paths {
    for value in load_secret_values(
      path,
      base_dir,
      vault_location,
      keys_location,
      key_name,
      gpg_key_id,
    )? {
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
fn decrypt_with_gpg(data_path: &Path) -> anyhow::Result<String> {
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
    write_vault_meta(vault_dir, "ABC123DEF456").unwrap();

    // Read it back
    assert_eq!(
      read_vault_gpg_key_id(vault_dir),
      Some("ABC123DEF456".to_string())
    );
  }

  #[test]
  fn test_vault_meta_overwrite() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path();

    write_vault_meta(vault_dir, "FIRST_KEY").unwrap();
    write_vault_meta(vault_dir, "SECOND_KEY").unwrap();

    assert_eq!(
      read_vault_gpg_key_id(vault_dir),
      Some("SECOND_KEY".to_string())
    );
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

  // ── SecretConfig::resolve ─────────────────────────────────────────────────

  #[test]
  fn test_secret_config_explicit_gpg_key_id() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    let base = Path::new(".");
    let config = SecretConfig::resolve(base, Some(vault_dir), None, None, Some("EXPLICIT_ID"));
    assert_eq!(config.gpg_key_id, Some("EXPLICIT_ID".to_string()));
  }

  #[test]
  fn test_secret_config_gpg_key_id_from_vault_metadata() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    write_vault_meta(dir.path(), "META_ID").unwrap();

    let base = Path::new(".");
    let config = SecretConfig::resolve(base, Some(vault_dir), None, None, None);
    assert_eq!(config.gpg_key_id, Some("META_ID".to_string()));
  }

  #[test]
  fn test_secret_config_explicit_gpg_key_id_overrides_metadata() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    write_vault_meta(dir.path(), "META_ID").unwrap();

    let base = Path::new(".");
    let config = SecretConfig::resolve(base, Some(vault_dir), None, None, Some("EXPLICIT_ID"));
    // Explicit arg wins over metadata
    assert_eq!(config.gpg_key_id, Some("EXPLICIT_ID".to_string()));
  }

  #[test]
  fn test_secret_config_no_gpg_key_id() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    let base = Path::new(".");
    // Empty vault dir — no .vault-meta.toml written
    let config = SecretConfig::resolve(base, Some(vault_dir), None, None, None);
    assert_eq!(config.gpg_key_id, None);
  }

  #[test]
  fn test_secret_config_key_name_default() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    let base = Path::new(".");
    let config = SecretConfig::resolve(base, Some(vault_dir), None, None, None);
    assert_eq!(config.key_name, "default");
  }

  #[test]
  fn test_secret_config_key_name_custom() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path().to_str().unwrap();
    let base = Path::new(".");
    let config = SecretConfig::resolve(base, Some(vault_dir), None, Some("mykey"), None);
    assert_eq!(config.key_name, "mykey");
  }

  // ── VaultMeta serialization ───────────────────────────────────────────────

  #[test]
  fn test_vault_meta_toml_no_gpg_key_id() {
    // When gpg_key_id is None, the field is skipped in the TOML output
    let meta = VaultMeta { gpg_key_id: None };
    let s = toml::to_string_pretty(&meta).unwrap();
    assert!(!s.contains("gpg_key_id"), "unexpected field in: {s}");
  }

  #[test]
  fn test_vault_meta_toml_with_gpg_key_id() {
    let meta = VaultMeta {
      gpg_key_id: Some("FINGERPRINT".to_string()),
    };
    let s = toml::to_string_pretty(&meta).unwrap();
    assert!(s.contains("gpg_key_id"), "field missing from: {s}");
    assert!(s.contains("FINGERPRINT"));
  }
}
