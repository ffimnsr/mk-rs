use std::fs;
use std::path::Path;

use clap::Args;
use mk_lib::secrets::write_vault_meta;

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct InitVault {
  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(short, long, help = "The key name")]
  key_name: Option<String>,

  #[arg(
    long,
    help = "GPG key ID or fingerprint to associate with this vault. When set, all vault commands (store, show, export, …) will use gpg automatically without needing the --gpg-key-id flag. Cannot be combined with --key-name."
  )]
  gpg_key_id: Option<String>,
}

impl InitVault {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    if self.key_name.is_some() && self.gpg_key_id.is_some() {
      anyhow::bail!("--key-name and --gpg-key-id are mutually exclusive");
    }

    let vault_location: &str = &self
      .vault_location
      .clone()
      .unwrap_or_else(|| context.vault_location());
    let gpg_key_id = self.gpg_key_id.clone().or_else(|| context.gpg_key_id());

    assert!(!vault_location.is_empty(), "Vault location must be provided");

    let path = Path::new(vault_location);
    if path.exists() {
      println!("Vault already exists at {vault_location}");
    } else {
      fs::create_dir_all(path)?;
      println!("Vault created at {vault_location}");
    }

    if let Some(id) = &gpg_key_id {
      write_vault_meta(path, id)?;
      println!("Vault configured to use GPG key: {id}");
    }

    Ok(())
  }
}
