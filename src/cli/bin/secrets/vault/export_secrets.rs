use std::fs::{
  self,
  File,
};
use std::io::Write as _;
use std::path::Path;

use anyhow::Context as _;
use clap::Args;
use pgp::composed::{
  Deserializable as _,
  Message,
  SignedSecretKey,
};

use crate::secrets::context::Context;
use crate::secrets::vault::{
  verify_key,
  verify_vault,
};

#[derive(Debug, Args)]
pub struct ExportSecrets {
  #[arg(help = "The secret identifier or prefix to export")]
  path: String,

  #[arg(short, long, help = "The output file")]
  output: Option<String>,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(long, help = "The keys location")]
  keys_location: Option<String>,

  #[arg(short, long, help = "The key name")]
  key_name: Option<String>,
}

impl ExportSecrets {
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
          let mut data_file = std::io::BufReader::new(File::open(data_path)?);
          let (message, _) = Message::from_armor(&mut data_file)?;
          let mut decrypted_message = message.decrypt(&pgp::types::Password::empty(), &signed_secret_key)?;
          let value = decrypted_message
            .as_data_string()
            .context("Failed to read secret value")?;

          values.push(value);
        }
      }

      if values.is_empty() {
        println!("No secrets found for path: {}", path);
      } else {
        // Write the values to the output file if provided
        // Otherwise, print the values to stdout which can be redirected
        // to a file
        if let Some(output) = &self.output {
          let mut output_file = File::create(output)?;
          for value in values {
            writeln!(output_file, "{}", value)?;
          }
          output_file.flush()?;
        } else {
          for value in values {
            println!("{}", value);
          }
        }
      }
    } else {
      println!("Path does not exist or is not a directory");
    }

    Ok(())
  }
}
