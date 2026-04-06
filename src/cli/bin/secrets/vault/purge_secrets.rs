use std::fs;
use std::path::Path;

use clap::Args;

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
    let path = self.path.as_str();
    let context_vault_location;
    let vault_location = match self.vault_location.as_deref() {
      Some(vault_location) => vault_location,
      None => {
        context_vault_location = context.vault_location();
        context_vault_location.as_str()
      },
    };

    assert!(!path.is_empty(), "Path or prefix must be provided");
    assert!(!vault_location.is_empty(), "Vault location must be provided");

    verify_vault(vault_location)?;

    let path = Path::new(vault_location).join(path);
    let data_path = path.join("data.asc");
    if path.exists() && path.is_dir() && data_path.exists() && data_path.is_file() {
      fs::remove_dir_all(&path)?;
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
