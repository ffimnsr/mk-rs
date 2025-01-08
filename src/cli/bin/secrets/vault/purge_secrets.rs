use std::fs;
use std::path::Path;

use clap::Args;
use mk_lib::file::ToUtf8 as _;

use crate::secrets::context::Context;
use crate::secrets::vault::verify_vault;

#[derive(Debug, Args)]
pub struct PurgeSecrets {
  #[arg(help = "The secret identifier or prefix to export")]
  path: String,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,
}

impl PurgeSecrets {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let path: &str = &self.path.clone();
    let vault_location: &str = &self
      .vault_location
      .clone()
      .unwrap_or_else(|| context.vault_location());

    assert!(!path.is_empty(), "Path or prefix must be provided");
    assert!(!vault_location.is_empty(), "Vault location must be provided");

    verify_vault(vault_location)?;

    let path = Path::new(vault_location).join(path);
    if path.exists() {
      fs::remove_dir_all(path.clone())?;
      println!("Secrets purged at {}", path.to_utf8()?);
    } else {
      println!("Secrets not found at {}", path.to_utf8()?);
    }
    Ok(())
  }
}
