use std::fs::{
  self,
  File,
};
use std::path::Path;

use clap::Args;
use console::style;
use pgp::composed::{
  Deserializable as _,
  SignedSecretKey,
};
use pgp::types::KeyDetails;
use prettytable::format::consts;
use prettytable::{
  row,
  Table,
};

use crate::secrets::context::Context;

use super::KEY_LOCATION_HELP;

#[derive(Debug, Args)]
pub struct ListKeys {
  /// The location to store the private key
  #[arg(short, long, help = KEY_LOCATION_HELP)]
  location: Option<String>,
}

impl ListKeys {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let location: &str = &self.location.clone().unwrap_or_else(|| context.keys_location());

    assert!(!location.is_empty(), "Location must be provided");

    let path = Path::new(location);
    if path.exists() && path.is_dir() {
      let entries = fs::read_dir(path)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("key"))
        .map(|entry| {
          entry
            .path()
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("")
            .to_string()
        })
        .collect::<Vec<_>>();

      if entries.is_empty() {
        println!("No keys found in location");
        return Ok(());
      }

      let mut table = Table::new();
      table.set_format(*consts::FORMAT_CLEAN);
      table.set_titles(row![Fbb->"Name", Fbb->"Key ID", Fbb->"Fingerprint"]);
      for entry in entries {
        let key_name: &str = &entry.clone();
        let filename_with_ext = format!("{key_name}.key");
        let key_path = Path::new(location).join(filename_with_ext);
        let mut secret_key_string = File::open(key_path)?;
        let (signed_secret_key, _) = SignedSecretKey::from_armor_single(&mut secret_key_string)?;
        signed_secret_key.verify()?;

        let key_id = hex::encode(signed_secret_key.key_id());
        let fingerprint = hex::encode(signed_secret_key.fingerprint().as_bytes());

        table.add_row(row![b->&key_name, Fg->&key_id, Fg->&fingerprint]);
      }
      let msg = style("Available keys:").bold().cyan();
      println!();
      println!("{msg}");
      println!();
      table.printstd();
    } else {
      println!("Location does not exist or is not a directory");
    }

    Ok(())
  }
}
