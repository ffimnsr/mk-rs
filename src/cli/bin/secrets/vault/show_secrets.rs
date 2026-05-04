use clap::Args;
use console::style;
use mk_lib::secrets::{load_secret_value, verify_vault};
use prettytable::format::consts;
use prettytable::{row, Table};
use std::io::Write as _;

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct ShowSecret {
  #[arg(help = "The secret identifier")]
  path: String,

  #[arg(short, long, help = "The path to the secret store")]
  vault_location: Option<String>,

  #[arg(long, help = "The keys location")]
  keys_location: Option<String>,

  #[arg(short, long, help = "The key name", conflicts_with = "gpg_key_id")]
  key_name: Option<String>,

  #[arg(
    long,
    conflicts_with = "key_name",
    help = "GPG key ID or fingerprint for hardware/passphrase-protected keys. Cannot be combined with --key-name."
  )]
  gpg_key_id: Option<String>,

  #[arg(short, long, help = "Print raw secret value without table formatting")]
  plain: bool,
}

impl ShowSecret {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let path: &str = &self.path.clone();
    let mut cli_overrides = context.settings().clone();
    if let Some(vault_location) = &self.vault_location {
      cli_overrides.vault_location = Some(vault_location.clone());
    }
    if let Some(keys_location) = &self.keys_location {
      cli_overrides.keys_location = Some(keys_location.clone());
    }
    if let Some(key_name) = &self.key_name {
      cli_overrides.key_name = Some(key_name.clone());
    }
    if let Some(gpg_key_id) = &self.gpg_key_id {
      cli_overrides.gpg_key_id = Some(gpg_key_id.clone());
    }
    let secret_config = context.resolve_with_settings(&cli_overrides);

    assert!(!path.is_empty(), "Path or prefix must be provided");

    verify_vault(&secret_config.vault_location)?;

    let value = load_secret_value(path, &secret_config)?;

    if self.plain {
      let mut stdout = std::io::stdout().lock();
      write!(stdout, "{}", value)?;
      stdout.flush()?;
      return Ok(());
    }

    let mut table = Table::new();
    table.set_format(*consts::FORMAT_CLEAN);
    table.set_titles(row![Fbb->"Name", Fbb->"Value"]);
    table.add_row(row![b->&path, Fg->&value]);

    let msg = style("Available secret:").bold().cyan();
    println!();
    println!("{msg}");
    println!();
    table.printstd();

    Ok(())
  }
}
