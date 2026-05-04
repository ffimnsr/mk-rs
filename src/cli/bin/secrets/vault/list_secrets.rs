use clap::Args;
use console::style;
use mk_lib::secrets::list_secret_paths;
use prettytable::format::consts;
use prettytable::{row, Table};

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct ListSecrets {
  #[arg(help = "Optional secret path prefix")]
  path: Option<String>,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(short, long, help = "Print plain output without headers")]
  plain: bool,
}

impl ListSecrets {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let path = self.path.as_deref();
    let mut cli_overrides = context.settings().clone();
    if let Some(vault_location) = &self.vault_location {
      cli_overrides.vault_location = Some(vault_location.clone());
    }
    let secret_config = context.resolve_with_settings(&cli_overrides);
    let secret_paths = list_secret_paths(path, &secret_config)?;
    if self.plain {
      for secret_path in secret_paths {
        println!("{}", secret_path);
      }
      return Ok(());
    }

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
