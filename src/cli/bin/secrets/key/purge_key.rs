use std::fs;
use std::path::Path;

use clap::Args;

use crate::secrets::context::Context;

use super::KEY_LOCATION_HELP;

#[derive(Debug, Args)]
pub struct PurgeKey {
  /// The location to store the private key
  #[arg(short, long, help = KEY_LOCATION_HELP)]
  location: Option<String>,

  /// If not provided, the key will be named "default"
  #[arg(short, long, help = "The key name")]
  name: Option<String>,
}

impl PurgeKey {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let location: &str = &self.location.clone().unwrap_or_else(|| context.keys_location());
    let name: &str = &self.name.clone().unwrap_or_else(|| context.key_name());

    assert!(!location.is_empty(), "Location must be provided");
    assert!(!name.is_empty(), "Key name must be provided");

    let file_path: &str = &format!("{location}/{name}.key");
    let path = Path::new(file_path);
    if path.exists() {
      fs::remove_file(path)?;
      println!("Key {name} deleted successfully.");
    } else {
      println!("Key {name} does not exist.");
    }

    Ok(())
  }
}
