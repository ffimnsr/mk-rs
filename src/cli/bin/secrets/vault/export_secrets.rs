use clap::Args;

use crate::secrets::context::Context;
use crate::secrets::vault::{
  verify_key,
  verify_vault,
};

#[derive(Debug, Args)]
pub struct ExportSecrets {
  #[arg(help = "The secret identifier or prefix to export")]
  path: String,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(short, long, help = "The keys location")]
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

    // TODO
    Ok(())
  }
}
