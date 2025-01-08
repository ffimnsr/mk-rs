use std::fs::{
  self,
  File,
};
use std::path::Path;

use anyhow::Context as _;
use clap::Args;
use mk_lib::file::ToUtf8;
use pgp::{
  Deserializable as _, Message, SignedSecretKey
};

use crate::secrets::context::Context;
use crate::secrets::vault::{
  verify_key,
  verify_vault,
};

#[derive(Debug, Args)]
pub struct ShowSecrets {
  #[arg(help = "The secret identifier or prefix to export")]
  path: String,

  #[arg(short, long, help = "The path to the secret store")]
  vault_location: Option<String>,

  #[arg(long, help = "The keys location")]
  keys_location: Option<String>,

  #[arg(short, long, help = "The key name")]
  key_name: Option<String>,
}

impl ShowSecrets {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let path: &str = &self.path.clone();
    let vault_location: &str = &self
      .vault_location
      .clone()
      .unwrap_or_else(|| context.vault_location());
    let keys_location: &str = &self
      .keys_location
      .clone()
      .unwrap_or_else(|| context.keys_location());
    let key_name: &str = &self.key_name.clone().unwrap_or_else(|| context.key_name());

    assert!(!path.is_empty(), "Path or prefix must be provided");
    assert!(!vault_location.is_empty(), "Vault location must be provided");
    assert!(!keys_location.is_empty(), "Keys location must be provided");
    assert!(!key_name.is_empty(), "Key name must be provided");

    verify_vault(vault_location)?;
    verify_key(keys_location, key_name)?;

    // Open the secret key file
    let key_name = format!("{}.key", key_name);
    let key_path = Path::new(keys_location).join(key_name);
    let mut secret_key_string = File::open(key_path)?;
    let (signed_secret_key, _) = SignedSecretKey::from_armor_single(&mut secret_key_string)?;
    signed_secret_key.verify()?;

    let secret_path = Path::new(vault_location).join(path);
    let mut values = Vec::new();
    if secret_path.exists() && secret_path.is_dir() {
      let entries = fs::read_dir(secret_path.clone())?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

      if entries.is_empty() {
        println!("No secrets found at {}", secret_path.to_utf8()?);
      }

      
      for entry in entries {
        let data_path = entry.path().join("data.asc");
        if data_path.exists() && data_path.is_file() {
          let mut data_file = File::open(data_path)?;
          let (message, _) = Message::from_armor_single(&mut data_file)?;
          let (decrypted_message, _) = message.decrypt(String::new, &[&signed_secret_key])?;
          let value = decrypted_message.get_literal()
            .ok_or_else(|| anyhow::anyhow!("Secret value is not a literal"))?
            .to_string()
            .context("Failed to read secret value")?;

          values.push(value);
        }
      }

      for value in values {
        println!("{}", value);
      }
    } else {
      println!("Path does not exist or is not a directory");
    }

    Ok(())
  }
}
