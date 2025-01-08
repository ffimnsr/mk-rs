use std::fs;
use std::path::Path;

use clap::Args;

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct InitVault {
  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(short, long, help = "The key name")]
  key_name: Option<String>,
}

impl InitVault {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let vault_location: &str = &self
      .vault_location
      .clone()
      .unwrap_or_else(|| context.vault_location());
    let key_name: &str = &self.key_name.clone().unwrap_or_else(|| context.key_name());

    assert!(!vault_location.is_empty(), "Vault location must be provided");
    assert!(!key_name.is_empty(), "Key name must be provided");

    let path = Path::new(vault_location);
    if path.exists() {
      println!("Vault already exists at {vault_location}");
    } else {
      fs::create_dir_all(path)?;
      println!("Vault created at {vault_location}");
    }
    Ok(())
  }
}
