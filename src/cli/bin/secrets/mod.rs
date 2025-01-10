use clap::{
  Args,
  Subcommand,
};
use context::Context;
use key::KEY_LOCATION_HELP;

mod context;
mod key;
mod utils;
mod vault;

/// The struct that represents the secrets command
#[derive(Debug, Args)]
pub struct Secrets {
  #[command(subcommand)]
  command: Option<SecretsCommand>,

  #[arg(long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(long, help = KEY_LOCATION_HELP)]
  keys_location: Option<String>,

  #[arg(long, help = "The key name")]
  key_name: Option<String>,
}

/// The available subcommands for the secrets command
#[derive(Debug, Subcommand)]
enum SecretsCommand {
  /// Access private keys using this subcommand
  #[command(visible_aliases = ["k"], arg_required_else_help = true, about = "Access private keys")]
  Key(key::Key),

  /// Access secret stores using this subcommand
  #[command(visible_aliases = ["v"], arg_required_else_help = true, about = "Access secret vault")]
  Vault(vault::Vault),

  /// List private keys
  #[command(visible_aliases = ["K"], about = "List private keys")]
  ListKeys(key::ListKeys),

  /// Initialize a new secret store
  #[command(visible_aliases = ["init"], about = "Initialize a new secret vault")]
  InitVault(vault::InitVault),

  /// Export a secret store
  #[command(visible_aliases = ["export", "e"], about = "Export secrets to file")]
  ExportSecrets(vault::ExportSecrets),
}

impl Secrets {
  pub fn execute(&self) -> anyhow::Result<()> {
    let mut context = Context::new();
    if let Some(keys_location) = &self.keys_location {
      context.set_keys_location(keys_location);
    }

    if let Some(vault_location) = &self.vault_location {
      context.set_vault_location(vault_location);
    }

    match &self.command {
      Some(SecretsCommand::Key(key)) => key.execute(&mut context),
      Some(SecretsCommand::Vault(vault)) => vault.execute(&mut context),
      Some(SecretsCommand::ListKeys(list_keys)) => list_keys.execute(&context),
      Some(SecretsCommand::InitVault(init_store)) => init_store.execute(&context),
      Some(SecretsCommand::ExportSecrets(export_secrets)) => export_secrets.execute(&context),
      None => Err(anyhow::anyhow!("No subcommand provided")),
    }
  }
}
