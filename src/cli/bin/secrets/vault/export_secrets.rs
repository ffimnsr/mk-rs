use clap::Args;
use mk_lib::secrets::{load_secret_value, verify_vault};
use std::fs::File;
use std::io::Write as _;

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct ExportSecret {
  #[arg(help = "The secret identifier")]
  path: String,

  #[arg(short, long, help = "The output file")]
  output: Option<String>,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(long, help = "The keys location")]
  keys_location: Option<String>,

  #[arg(short, long, help = "The key name", conflicts_with = "gpg_key_id")]
  key_name: Option<String>,

  #[arg(
    long,
    conflicts_with = "key_name",
    help = "GPG key ID or fingerprint for hardware/passphrase-protected keys. Cannot be combined with --key-name."
  )]
  gpg_key_id: Option<String>,
}

impl ExportSecret {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let path: &str = &self.path.clone();
    let mut cli_overrides = context.settings().clone();
    if let Some(vault_location) = &self.vault_location {
      cli_overrides.vault_location = Some(vault_location.clone());
    }
    if let Some(keys_location) = &self.keys_location {
      cli_overrides.keys_location = Some(keys_location.clone());
    }
    if let Some(key_name) = &self.key_name {
      cli_overrides.key_name = Some(key_name.clone());
    }
    if let Some(gpg_key_id) = &self.gpg_key_id {
      cli_overrides.gpg_key_id = Some(gpg_key_id.clone());
    }
    let secret_config = context.resolve_with_settings(&cli_overrides);

    assert!(!path.is_empty(), "Path or prefix must be provided");

    verify_vault(&secret_config.vault_location)?;

    let value = load_secret_value(path, &secret_config)?;

    if let Some(output) = &self.output {
      let mut output_file = File::create(output)?;
      write!(output_file, "{}", *value)?;
      output_file.flush()?;
    } else {
      println!("{}", *value);
    }

    Ok(())
  }
}
