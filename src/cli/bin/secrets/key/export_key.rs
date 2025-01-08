use std::fs::File;
use std::io::Write as _;
use std::path::Path;

use clap::Args;
use pgp::{
  ArmorOptions,
  Deserializable as _,
  SignedSecretKey,
};

use crate::secrets::context::Context;

use super::KEY_LOCATION_HELP;

#[derive(Debug, Args)]
pub struct ExportKey {
  #[arg(short, long, help = "The output file")]
  output: String,

  /// The location to store the private key
  #[arg(short, long, help = KEY_LOCATION_HELP)]
  location: Option<String>,

  /// If not provided, the key will be named "default"
  #[arg(short, long, help = "The key name")]
  name: Option<String>,
}

impl ExportKey {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let location: &str = &self.location.clone().unwrap_or_else(|| context.keys_location());
    let name: &str = &self.name.clone().unwrap_or_else(|| context.key_name());
    let output: &str = &self.output.clone();

    assert!(!location.is_empty(), "Location must be provided");
    assert!(!name.is_empty(), "Key name must be provided");
    assert!(!output.is_empty(), "Output file must be provided");

    let filename_with_ext: &str = &format!("{name}.key");

    let path = Path::new(location).join(filename_with_ext);
    if path.exists() {
      // We opt to parse it rather than copy it directly to verify if it is a valid key
      let mut secret_key_string = File::open(path)?;
      let (signed_secret_key, _) = SignedSecretKey::from_armor_single(&mut secret_key_string)?;
      signed_secret_key.verify()?;

      // Save the armored private key to a file
      let mut file = File::open(output)?;
      signed_secret_key.to_armored_writer(&mut file, ArmorOptions::default())?;
      file.flush()?;

      println!("Key {name} exported to {output}.");
    } else {
      println!("Key {name} does not exist.");
    }

    Ok(())
  }
}
