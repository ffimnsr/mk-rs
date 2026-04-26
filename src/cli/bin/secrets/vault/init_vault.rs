use std::fs;
use std::path::Path;

use clap::Args;
use mk_lib::file::ToUtf8 as _;
use mk_lib::secrets::{
  read_vault_gpg_key_id,
  write_vault_meta,
  SecretBackend,
  SecretConfig,
  SecretSettings,
  VaultMeta,
};
use serde_yaml::{
  Mapping,
  Value,
};

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct InitVault {
  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(short, long, help = "The key name", conflicts_with = "gpg_key_id")]
  key_name: Option<String>,

  #[arg(
    long,
    conflicts_with = "key_name",
    help = "GPG key ID or fingerprint to associate with this vault. When set, all vault commands (store, show, export, …) will use gpg automatically without needing the --gpg-key-id flag. Cannot be combined with --key-name."
  )]
  gpg_key_id: Option<String>,

  #[arg(
    long,
    help = "Update the active config file with a root secrets block after vault initialization"
  )]
  write_config: bool,
}

impl InitVault {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let mut secret_config = context.resolved_config();
    if let Some(vault_location) = &self.vault_location {
      secret_config.vault_location = Path::new(vault_location).to_path_buf();
    }
    let gpg_key_id = self
      .gpg_key_id
      .clone()
      .or_else(|| secret_config.gpg_key_id.clone());
    secret_config.backend = if gpg_key_id.is_some() {
      SecretBackend::Gpg
    } else {
      SecretBackend::BuiltInPgp
    };
    secret_config.gpg_key_id = gpg_key_id.clone();
    let vault_location = secret_config.vault_location.to_string_lossy().to_string();

    if self.write_config {
      ensure_config_mutation_supported(context.active_config_path())?;
    }

    assert!(!vault_location.is_empty(), "Vault location must be provided");

    let path = Path::new(&vault_location);
    if path.exists() {
      println!("Vault already exists at {vault_location}");
    } else {
      fs::create_dir_all(path)?;
      println!("Vault created at {vault_location}");
    }

    if let Some(id) = &gpg_key_id {
      if let Some(existing_id) = read_vault_gpg_key_id(path) {
        if existing_id != *id {
          eprintln!(
            "Warning: vault GPG key changed from '{}' to '{}'. Overwriting metadata.",
            existing_id, id
          );
        }
      }
      println!("Vault configured to use GPG key: {id}");
    }

    write_vault_meta(path, &vault_meta_from_config(&secret_config))?;

    if self.write_config {
      let config_path = context
        .active_config_path()
        .ok_or_else(|| anyhow::anyhow!("No active config file available for --write-config"))?;
      let secrets = secret_settings_for_config_write(context, &secret_config);
      update_yaml_config(config_path, &secrets)?;
      println!(
        "Config updated at {}",
        config_path.to_utf8().unwrap_or("<non-utf8-path>")
      );
    }

    Ok(())
  }
}

fn ensure_config_mutation_supported(config_path: Option<&Path>) -> anyhow::Result<()> {
  let Some(config_path) = config_path else {
    anyhow::bail!("No active config file available for --write-config");
  };

  match config_path.extension().and_then(|ext| ext.to_str()) {
    Some("yaml") | Some("yml") => Ok(()),
    Some(ext) => anyhow::bail!(
      "Config mutation via --write-config only supports YAML files. Refusing to update .{} config: {}",
      ext,
      config_path.to_utf8().unwrap_or("<non-utf8-path>")
    ),
    None => anyhow::bail!(
      "Config mutation via --write-config only supports YAML files. Refusing to update config without extension: {}",
      config_path.to_utf8().unwrap_or("<non-utf8-path>")
    ),
  }
}

fn vault_meta_from_config(secret_config: &SecretConfig) -> VaultMeta {
  match secret_config.backend {
    SecretBackend::Gpg => VaultMeta {
      backend: Some(SecretBackend::Gpg),
      keys_location: None,
      key_name: None,
      gpg_key_id: secret_config.gpg_key_id.clone(),
    },
    SecretBackend::BuiltInPgp => VaultMeta {
      backend: Some(SecretBackend::BuiltInPgp),
      keys_location: Some(secret_config.keys_location.to_string_lossy().to_string()),
      key_name: Some(secret_config.key_name.clone()),
      gpg_key_id: None,
    },
  }
}

fn secret_settings_for_config_write(context: &Context, secret_config: &SecretConfig) -> SecretSettings {
  let secrets_path = context
    .root_settings()
    .and_then(|settings| settings.secrets_path.clone());
  let vault_location = path_for_config(context.config_base_dir(), &secret_config.vault_location);

  match secret_config.backend {
    SecretBackend::Gpg => SecretSettings {
      backend: Some(SecretBackend::Gpg),
      vault_location: Some(vault_location),
      keys_location: None,
      key_name: None,
      gpg_key_id: secret_config.gpg_key_id.clone(),
      secrets_path,
    },
    SecretBackend::BuiltInPgp => SecretSettings {
      backend: Some(SecretBackend::BuiltInPgp),
      vault_location: Some(vault_location),
      keys_location: Some(path_for_config(
        context.config_base_dir(),
        &secret_config.keys_location,
      )),
      key_name: Some(secret_config.key_name.clone()),
      gpg_key_id: None,
      secrets_path,
    },
  }
}

fn path_for_config(base_dir: &Path, path: &Path) -> String {
  if let Ok(relative) = path.strip_prefix(base_dir) {
    let rendered = relative.to_string_lossy().replace('\\', "/");
    if !rendered.is_empty() {
      return rendered;
    }
  }

  if let Some(relative) = canonical_relative_path(base_dir, path) {
    let rendered = relative.to_string_lossy().replace('\\', "/");
    if !rendered.is_empty() {
      return rendered;
    }
  }

  path.to_string_lossy().replace('\\', "/")
}

fn canonical_relative_path(base_dir: &Path, path: &Path) -> Option<std::path::PathBuf> {
  let canonical_base = fs::canonicalize(base_dir).ok()?;
  let canonical_path = fs::canonicalize(path).ok()?;
  canonical_path
    .strip_prefix(canonical_base)
    .ok()
    .map(std::path::Path::to_path_buf)
}

fn update_yaml_config(config_path: &Path, secrets: &SecretSettings) -> anyhow::Result<()> {
  let (mut value, prefix) = if config_path.exists() {
    let contents = fs::read_to_string(config_path)?;
    let prefix = leading_yaml_preamble(&contents);
    let trimmed = contents.trim();
    let value = if trimmed.is_empty() {
      Value::Mapping(Mapping::new())
    } else {
      serde_yaml::from_str::<Value>(&contents)?
    };
    (value, prefix)
  } else {
    (Value::Mapping(Mapping::new()), String::new())
  };

  let mapping = value
    .as_mapping_mut()
    .ok_or_else(|| anyhow::anyhow!("Root config must be a YAML mapping to support --write-config"))?;

  remove_legacy_secret_fields(mapping);
  mapping.insert(
    Value::String(String::from("secrets")),
    secret_settings_to_yaml_value(secrets),
  );
  mapping
    .entry(Value::String(String::from("tasks")))
    .or_insert_with(|| Value::Mapping(Mapping::new()));

  let mut rendered = String::new();
  if !prefix.is_empty() {
    rendered.push_str(&prefix);
    if !prefix.ends_with('\n') {
      rendered.push('\n');
    }
  }
  rendered.push_str(&serde_yaml::to_string(&value)?);
  fs::write(config_path, rendered)?;
  Ok(())
}

fn leading_yaml_preamble(contents: &str) -> String {
  let mut prefix = String::new();
  for line in contents.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
      prefix.push_str(line);
      prefix.push('\n');
      continue;
    }
    break;
  }
  prefix
}

fn remove_legacy_secret_fields(mapping: &mut Mapping) {
  for key in [
    "vault_location",
    "keys_location",
    "key_name",
    "gpg_key_id",
    "secrets_path",
  ] {
    mapping.remove(Value::String(key.to_string()));
  }
}

fn secret_settings_to_yaml_value(settings: &SecretSettings) -> Value {
  let mut mapping = Mapping::new();

  if let Some(backend) = &settings.backend {
    let backend = match backend {
      SecretBackend::BuiltInPgp => "built_in_pgp",
      SecretBackend::Gpg => "gpg",
    };
    mapping.insert(
      Value::String(String::from("backend")),
      Value::String(backend.to_string()),
    );
  }
  if let Some(vault_location) = &settings.vault_location {
    mapping.insert(
      Value::String(String::from("vault_location")),
      Value::String(vault_location.clone()),
    );
  }
  if let Some(keys_location) = &settings.keys_location {
    mapping.insert(
      Value::String(String::from("keys_location")),
      Value::String(keys_location.clone()),
    );
  }
  if let Some(key_name) = &settings.key_name {
    mapping.insert(
      Value::String(String::from("key_name")),
      Value::String(key_name.clone()),
    );
  }
  if let Some(gpg_key_id) = &settings.gpg_key_id {
    mapping.insert(
      Value::String(String::from("gpg_key_id")),
      Value::String(gpg_key_id.clone()),
    );
  }
  if let Some(secrets_path) = &settings.secrets_path {
    mapping.insert(
      Value::String(String::from("secrets_path")),
      Value::Sequence(
        secrets_path
          .iter()
          .map(|path| Value::String(path.clone()))
          .collect::<Vec<_>>(),
      ),
    );
  }

  Value::Mapping(mapping)
}
