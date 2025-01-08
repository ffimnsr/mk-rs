use std::fs;
use std::path::Path;

use clap::Args;

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

      for entry in entries {
        println!("{}", entry);
      }
    } else {
      println!("Location does not exist or is not a directory");
    }

    Ok(())
  }
}
