use std::fs;
use std::path::Path;

use clap::Args;
use mk_lib::file::ToUtf8 as _;

use crate::secrets::context::Context;
use crate::secrets::vault::verify_vault;

#[derive(Debug, Args)]
pub struct PurgeSecret {
  #[arg(help = "The secret identifier")]
  path: String,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,
}

impl PurgeSecret {
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
    let data_path = path.join("data.asc");
    if path.exists() && path.is_dir() && data_path.exists() && data_path.is_file() {
      fs::remove_dir_all(path.clone())?;
      println!("Secret '{}' removed from vault.", self.path);
    } else {
      println!(
        "Secret '{}' not found in vault. List available secrets with: mk secrets vault list",
        self.path
      );
    }
    Ok(())
  }
}
