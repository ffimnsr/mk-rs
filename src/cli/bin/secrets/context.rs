use std::path::{
  Path,
  PathBuf,
};

use mk_lib::schema::TaskRoot;
use mk_lib::secrets::{
  resolve_secret_config,
  SecretBackend,
  SecretConfig,
  SecretSettings,
};

pub(super) struct Context {
  settings: SecretSettings,
  base_dir: PathBuf,
  root_settings: Option<SecretSettings>,
  active_config_path: Option<PathBuf>,
}

impl Context {
  pub fn new(task_root: &TaskRoot) -> Self {
    Self {
      settings: SecretSettings::default(),
      base_dir: task_root.config_base_dir(),
      root_settings: task_root.normalized_secret_settings(),
      active_config_path: task_root.source_path.clone(),
    }
  }

  pub fn set_keys_location(&mut self, keys_location: &str) {
    self.settings.keys_location = Some(keys_location.to_string());
  }

  pub fn set_vault_location(&mut self, vault_location: &str) {
    self.settings.vault_location = Some(vault_location.to_string());
  }

  pub fn set_key_name(&mut self, key_name: &str) {
    self.settings.key_name = Some(key_name.to_string());
    self.settings.backend = Some(SecretBackend::BuiltInPgp);
  }

  pub fn set_gpg_key_id(&mut self, gpg_key_id: &str) {
    self.settings.gpg_key_id = Some(gpg_key_id.to_string());
    self.settings.backend = Some(SecretBackend::Gpg);
  }

  pub fn settings(&self) -> &SecretSettings {
    &self.settings
  }

  pub fn resolved_config(&self) -> SecretConfig {
    self.resolve_with_settings(&self.settings)
  }

  pub fn resolve_with_settings(&self, settings: &SecretSettings) -> SecretConfig {
    resolve_secret_config(&self.base_dir, Some(settings), None, self.root_settings.as_ref())
  }

  pub fn active_config_path(&self) -> Option<&Path> {
    self.active_config_path.as_deref()
  }

  pub fn config_base_dir(&self) -> &Path {
    &self.base_dir
  }

  pub fn root_settings(&self) -> Option<&SecretSettings> {
    self.root_settings.as_ref()
  }

  pub fn keys_location(&self) -> String {
    self.resolved_config().keys_location.to_string_lossy().to_string()
  }

  pub fn vault_location(&self) -> String {
    self
      .resolved_config()
      .vault_location
      .to_string_lossy()
      .to_string()
  }

  pub fn key_name(&self) -> String {
    self.resolved_config().key_name
  }
}
