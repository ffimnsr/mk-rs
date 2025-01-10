use clap::{
  Args,
  Subcommand,
};

use super::context::Context;

pub use export_key::ExportKey;
pub use generate_key::GenerateKey;
pub use list_keys::ListKeys;
pub use purge_key::PurgeKey;

mod export_key;
mod generate_key;
mod list_keys;
mod purge_key;

/// The help message for the key location
pub const KEY_LOCATION_HELP: &str = "The path to where the private keys are stored";

/// The struct that represents the key command
#[derive(Debug, Args)]
pub struct Key {
  /// The subcommand to run
  #[command(subcommand)]
  command: Option<KeyCommand>,

  /// The location to store the private key
  /// This is a global option that can be used with any subcommand
  /// If not provided, the default location will be used
  /// If the default location does not exist, it will be created
  /// If the default location is not provided, the key will not be created
  /// If the key already exists, it will not be created
  #[arg(short, long, help = KEY_LOCATION_HELP)]
  location: Option<String>,
}

/// The available subcommands for the key command
#[derive(Debug, Subcommand)]
enum KeyCommand {
  /// Generate a new private key
  #[command(visible_aliases = ["gen"], about = "Generate a new private key")]
  GenerateKey(GenerateKey),

  /// List all private keys
  #[command(visible_aliases = ["K", "ls"], about = "List all private keys")]
  ListKeys(ListKeys),

  /// Purge and remove a private key
  #[command(visible_aliases = ["rm"], about = "Purge and remove a private key")]
  PurgeKey(PurgeKey),

  /// Export a private key
  #[command(visible_aliases = ["export", "e"], about = "Export selected private key")]
  ExportKey(ExportKey),
}

impl Key {
  pub fn execute(&self, context: &mut Context) -> anyhow::Result<()> {
    if let Some(location) = &self.location {
      context.set_keys_location(location);
    }

    match &self.command {
      Some(command) => command.run(context),
      None => Err(anyhow::anyhow!("No subcommand provided")),
    }
  }
}

impl KeyCommand {
  pub fn run(&self, context: &Context) -> anyhow::Result<()> {
    match self {
      KeyCommand::GenerateKey(args) => args.execute(context),
      KeyCommand::ListKeys(args) => args.execute(context),
      KeyCommand::PurgeKey(args) => args.execute(context),
      KeyCommand::ExportKey(args) => args.execute(context),
    }
  }
}
