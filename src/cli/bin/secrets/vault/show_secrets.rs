use std::fs::{
  self,
  File,
};
use std::path::Path;

use anyhow::Context as _;
use clap::Args;
use console::style;
use pgp::{
  Deserializable as _,
  Message,
  SignedSecretKey,
};
use prettytable::format::consts;
use prettytable::{
  row,
  Table,
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
    let key_name_with_ext = format!("{key_name}.key");
    let key_path = Path::new(keys_location).join(key_name_with_ext);
    let mut secret_key_string = File::open(key_path)?;
    let (signed_secret_key, _) = SignedSecretKey::from_armor_single(&mut secret_key_string)?;
    signed_secret_key.verify()?;

    let secret_path = Path::new(vault_location).join(path);
    let mut values = Vec::new();

    if secret_path.exists() && secret_path.is_dir() {
      // Check for file and subdirectories
      let entries = fs::read_dir(secret_path.clone())?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

      // Check for data files in the subdirectories
      for entry in entries {
        let data_path = if entry.path().is_dir() {
          entry.path().join("data.asc")
        } else {
          entry.path()
        };

        // Read the data file
        if data_path.exists() && data_path.is_file() {
          let mut data_file = File::open(data_path)?;
          let (message, _) = Message::from_armor_single(&mut data_file)?;
          let (decrypted_message, _) = message.decrypt(String::new, &[&signed_secret_key])?;
          let value = decrypted_message
            .get_literal()
            .ok_or_else(|| anyhow::anyhow!("Secret value is not a literal"))?
            .to_string()
            .context("Failed to read secret value")?;

          values.push(value);
        }
      }

      if values.is_empty() {
        println!("No secrets found for path: {}", path);
      } else {
        let mut table = Table::new();
        table.set_format(*consts::FORMAT_CLEAN);
        table.set_titles(row![Fbb->"Name", Fbb->"Value"]);
        for value in values {
          table.add_row(row![b->&path, Fg->&value]);
        }
        let msg = style("Available secrets:").bold().cyan();
        println!();
        println!("{msg}");
        println!();
        table.printstd();
      }
    } else {
      println!("Path does not exist or is not a directory");
    }

    Ok(())
  }
}
