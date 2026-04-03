use std::path::Path;

use clap::Args;
use console::style;
use mk_lib::secrets::load_secret_value;
use prettytable::format::consts;
use prettytable::{
  row,
  Table,
};

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct ShowSecret {
  #[arg(help = "The secret identifier")]
  path: String,

  #[arg(short, long, help = "The path to the secret store")]
  vault_location: Option<String>,

  #[arg(long, help = "The keys location")]
  keys_location: Option<String>,

  #[arg(short, long, help = "The key name")]
  key_name: Option<String>,
}

impl ShowSecret {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let path: &str = &self.path.clone();
    let vault_location: &str = &self
      .vault_location
      .clone()
      .unwrap_or_else(|| context.vault_location());
    let keys_location: &str = &self
      .keys_location
      .clone()
      .unwrap_or_else(|| context.keys_location());
    let key_name: &str = &self.key_name.clone().unwrap_or_else(|| context.key_name());

    assert!(!path.is_empty(), "Path or prefix must be provided");
    assert!(!vault_location.is_empty(), "Vault location must be provided");
    assert!(!keys_location.is_empty(), "Keys location must be provided");
    assert!(!key_name.is_empty(), "Key name must be provided");

    let value = load_secret_value(
      path,
      Path::new("."),
      Some(vault_location),
      Some(keys_location),
      Some(key_name),
    )?;

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
