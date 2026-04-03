use std::path::Path;

use clap::Args;
use console::style;
use mk_lib::secrets::list_secret_paths;
use prettytable::format::consts;
use prettytable::{
  row,
  Table,
};

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct ListSecrets {
  #[arg(help = "Optional secret path prefix")]
  path: Option<String>,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,
}

impl ListSecrets {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let path = self.path.as_deref();
    let vault_location = self
      .vault_location
      .clone()
      .unwrap_or_else(|| context.vault_location());

    let secret_paths = list_secret_paths(path, Path::new("."), Some(&vault_location))?;
    let mut table = Table::new();
    table.set_format(*consts::FORMAT_CLEAN);
    table.set_titles(row![Fbb->"Name"]);

    for secret_path in secret_paths {
      table.add_row(row![Fg->secret_path]);
    }

    let msg = style("Available secrets:").bold().cyan();
    println!();
    println!("{msg}");
    println!();
    table.printstd();

    Ok(())
  }
}
