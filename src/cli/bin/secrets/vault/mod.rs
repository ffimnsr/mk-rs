use std::path::Path;

use clap::{
  Args,
  Subcommand,
};

pub use export_secrets::ExportSecret;
pub use init_vault::InitVault;
pub use list_secrets::ListSecrets;
pub use purge_secrets::PurgeSecret;
pub use show_secrets::ShowSecret;
pub use store_secret::StoreSecret;

use super::context::Context;

mod export_secrets;
mod init_vault;
mod list_secrets;
mod purge_secrets;
mod show_secrets;
mod store_secret;

#[derive(Debug, Args)]
pub struct Vault {
  #[command(subcommand)]
  command: Option<VaultCommand>,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,
}

#[derive(Debug, Subcommand)]
enum VaultCommand {
  #[command(visible_aliases = ["init"], about = "Initialize a new secret vault")]
  InitVault(InitVault),

  #[command(visible_aliases = ["list", "ls"], about = "List available secrets")]
  ListSecrets(ListSecrets),

  #[command(visible_aliases = ["store", "set"], arg_required_else_help = true, about = "Store a secret")]
  StoreSecret(StoreSecret),

  #[command(visible_aliases = ["show", "get", "s"], arg_required_else_help = true, about = "Retrieve a secret")]
  ShowSecret(ShowSecret),

  #[command(visible_aliases = ["purge", "rm"], arg_required_else_help = true, about = "Purge and delete a secret")]
  PurgeSecret(PurgeSecret),

  #[command(
    visible_aliases = ["export", "e"],
    arg_required_else_help = true,
    about = "Export a secret to a file"
  )]
  ExportSecret(ExportSecret),
}

impl Vault {
  pub fn execute(&self, context: &mut Context) -> anyhow::Result<()> {
    if let Some(vault_location) = &self.vault_location {
      context.set_vault_location(vault_location);
    }

    match &self.command {
      Some(command) => command.run(context),
      None => Err(anyhow::anyhow!(
        "No vault subcommand given. Run 'mk secrets vault --help' to see available subcommands."
      )),
    }
  }
}

impl VaultCommand {
  pub fn run(&self, context: &Context) -> anyhow::Result<()> {
    match self {
      VaultCommand::InitVault(init_vault) => init_vault.execute(context),
      VaultCommand::ListSecrets(list_secrets) => list_secrets.execute(context),
      VaultCommand::StoreSecret(store_secret) => store_secret.execute(context),
      VaultCommand::ShowSecret(show_secret) => show_secret.execute(context),
      VaultCommand::PurgeSecret(purge_secret) => purge_secret.execute(context),
      VaultCommand::ExportSecret(export_secret) => export_secret.execute(context),
    }
  }
}

fn verify_vault(vault_location: &str) -> anyhow::Result<()> {
  let path = Path::new(vault_location);
  if !path.exists() || !path.is_dir() {
    anyhow::bail!(
      "Vault not found at '{}'. Initialize it first with: mk secrets vault init",
      vault_location
    );
  }

  Ok(())
}

fn verify_key(keys_location: &str, key_name: &str) -> anyhow::Result<()> {
  let keys_path = Path::new(keys_location);
  if !keys_path.exists() || !keys_path.is_dir() {
    anyhow::bail!(
      "Keys directory not found at '{}'. Generate a key first with: mk secrets key gen",
      keys_location
    );
  }

  let key_filename = format!("{key_name}.key");
  let key_path = keys_path.join(&key_filename);
  if !key_path.exists() || !key_path.is_file() {
    anyhow::bail!(
      "Key '{}' not found in '{}'. Generate it with: mk secrets key gen --name {}",
      key_name,
      keys_location,
      key_name
    );
  }

  Ok(())
}
