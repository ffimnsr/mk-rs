use clap::{Args, Subcommand};
use context::Context;
use key::KEY_LOCATION_HELP;
use mk_lib::schema::TaskRoot;

mod context;
mod doctor;
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

  #[arg(long, help = "The key name", conflicts_with = "gpg_key_id")]
  key_name: Option<String>,

  #[arg(
    long,
    conflicts_with = "key_name",
    help = "GPG key ID or fingerprint for hardware/passphrase-protected keys (delegates crypto to the system gpg binary). Cannot be combined with --key-name."
  )]
  gpg_key_id: Option<String>,
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

  /// Print resolved secret configuration
  #[command(about = "Inspect resolved secret configuration")]
  Doctor(doctor::Doctor),

  /// Export a secret
  #[command(visible_aliases = ["export", "e"], about = "Export a secret to file")]
  ExportSecret(vault::ExportSecret),
}

impl Secrets {
  pub fn execute(&self, task_root: &TaskRoot) -> anyhow::Result<()> {
    let mut context = Context::new(task_root);
    if let Some(keys_location) = &self.keys_location {
      context.set_keys_location(keys_location);
    }

    if let Some(vault_location) = &self.vault_location {
      context.set_vault_location(vault_location);
    }

    if let Some(key_name) = &self.key_name {
      context.set_key_name(key_name);
    }

    if let Some(gpg_key_id) = &self.gpg_key_id {
      context.set_gpg_key_id(gpg_key_id);
    }

    match &self.command {
      Some(SecretsCommand::Key(key)) => key.execute(&mut context),
      Some(SecretsCommand::Vault(vault)) => vault.execute(&mut context),
      Some(SecretsCommand::ListKeys(list_keys)) => list_keys.execute(&context),
      Some(SecretsCommand::InitVault(init_store)) => init_store.execute(&context),
      Some(SecretsCommand::Doctor(doctor)) => doctor.execute(&context),
      Some(SecretsCommand::ExportSecret(export_secret)) => export_secret.execute(&context),
      None => Err(anyhow::anyhow!(
        "No secrets subcommand given. Run 'mk secrets --help' to see available subcommands."
      )),
    }
  }
}
