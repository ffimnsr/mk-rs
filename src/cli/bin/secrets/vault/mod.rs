use std::path::Path;

use clap::{
  Args,
  Subcommand,
};

pub use export_secrets::ExportSecrets;
pub use init_vault::InitVault;
pub use purge_secrets::PurgeSecrets;
pub use show_secrets::ShowSecrets;
pub use store_secret::StoreSecret;

use super::context::Context;

mod export_secrets;
mod init_vault;
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

  #[command(visible_aliases = ["store", "set"], arg_required_else_help = true, about = "Store a secret")]
  StoreSecret(StoreSecret),

  #[command(visible_aliases = ["show", "get", "s"], arg_required_else_help = true, about = "Retrieve a secret")]
  ShowSecrets(ShowSecrets),

  #[command(visible_aliases = ["purge", "rm"], arg_required_else_help = true, about = "Purge and delete a secret")]
  PurgeSecrets(PurgeSecrets),

  #[command(
    visible_aliases = ["export", "e"],
    arg_required_else_help = true,
    about = "Export a secrets to dotenv file"
  )]
  ExportSecrets(ExportSecrets),
}

impl Vault {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    match &self.command {
      Some(command) => command.run(context),
      None => Err(anyhow::anyhow!("No subcommand provided")),
    }
  }
}

impl VaultCommand {
  pub fn run(&self, context: &Context) -> anyhow::Result<()> {
    match self {
      VaultCommand::InitVault(init_vault) => init_vault.execute(context),
      VaultCommand::StoreSecret(store_secret) => store_secret.execute(context),
      VaultCommand::ShowSecrets(show_secrets) => show_secrets.execute(context),
      VaultCommand::PurgeSecrets(purge_secrets) => purge_secrets.execute(context),
      VaultCommand::ExportSecrets(export_secrets) => export_secrets.execute(context),
    }
  }
}

fn verify_vault(vault_location: &str) -> anyhow::Result<()> {
  let path = Path::new(vault_location);
  if !path.exists() || !path.is_dir() {
    anyhow::bail!("The store does not exist");
  }

  Ok(())
}

fn verify_key(keys_location: &str, key_name: &str) -> anyhow::Result<()> {
  let keys_path = Path::new(keys_location);
  if !keys_path.exists() || !keys_path.is_dir() {
    anyhow::bail!("The keys location does not exist");
  }

  let key_name = format!("{key_name}.key", );
  let key_path = keys_path.join(key_name);
  if !key_path.exists() || !key_path.is_file() {
    anyhow::bail!("The key does not exist");
  }

  Ok(())
}
