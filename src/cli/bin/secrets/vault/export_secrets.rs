use std::fs::File;
use std::io::Write as _;
use std::path::Path;

use clap::Args;
use mk_lib::secrets::load_secret_value;

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct ExportSecret {
  #[arg(help = "The secret identifier")]
  path: String,

  #[arg(short, long, help = "The output file")]
  output: Option<String>,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(long, help = "The keys location")]
  keys_location: Option<String>,

  #[arg(short, long, help = "The key name")]
  key_name: Option<String>,
}

impl ExportSecret {
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

    if let Some(output) = &self.output {
      let mut output_file = File::create(output)?;
      writeln!(output_file, "{}", value)?;
      output_file.flush()?;
    } else {
      println!("{}", value);
    }

    Ok(())
  }
}
