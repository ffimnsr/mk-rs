use std::env;
use std::path::PathBuf;

pub(super) struct Context {
  keys_location: Option<String>,
  vault_location: Option<String>,
  key_name: Option<String>,
  gpg_key_id: Option<String>,
}

impl Context {
  pub fn new() -> Self {
    Self {
      keys_location: None,
      vault_location: None,
      key_name: None,
      gpg_key_id: None,
    }
  }

  pub fn set_keys_location(&mut self, keys_location: &str) {
    self.keys_location = Some(keys_location.to_string());
  }

  pub fn set_vault_location(&mut self, vault_location: &str) {
    self.vault_location = Some(vault_location.to_string());
  }

  pub fn set_key_name(&mut self, key_name: &str) {
    self.key_name = Some(key_name.to_string());
  }

  pub fn set_gpg_key_id(&mut self, gpg_key_id: &str) {
    self.gpg_key_id = Some(gpg_key_id.to_string());
  }

  pub fn keys_location(&self) -> String {
    self.keys_location.clone().unwrap_or_else(|| {
      let home_dir = if cfg!(target_os = "windows") {
        env::var("USERPROFILE").unwrap_or_else(|_| "./.mk/priv".to_string())
      } else {
        env::var("HOME").unwrap_or_else(|_| "./.mk/priv".to_string())
      };
      let mut path = PathBuf::from(home_dir);
      path.push(".config");
      path.push("mk");
      path.push("priv");
      path.to_string_lossy().to_string()
    })
  }

  pub fn vault_location(&self) -> String {
    self.vault_location.clone().unwrap_or("./.mk/vault".to_string())
  }

  pub fn key_name(&self) -> String {
    self.key_name.clone().unwrap_or("default".to_string())
  }

  pub fn gpg_key_id(&self) -> Option<String> {
    self.gpg_key_id.clone()
  }
}
