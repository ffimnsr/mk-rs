use std::{fs::{self, File}, io::Write as _, path::Path};

use clap::Args;
use pgp::{ArmorOptions, KeyType, SecretKeyParamsBuilder};
use rand::thread_rng;

use crate::secrets::context::Context;

use super::KEY_LOCATION_HELP;

#[derive(Debug, Args)]
pub struct GenerateKey {
  /// The location to store the private key
  #[arg(short, long, help = KEY_LOCATION_HELP)]
  location: Option<String>,

  /// If not provided, the key will be named "default"
  /// If the key already exists, it will not be created
  #[arg(short, long, help = "The key name")]
  name: Option<String>,

  /// If the key already exists, it will be overwritten
  #[arg(short, long, help = "Force overwrite the key")]
  force: bool,
}

impl GenerateKey {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let location: &str = &self.location.clone().unwrap_or_else(|| context.keys_location());
    let name: &str = &self.name.clone().unwrap_or_else(|| context.key_name());

    assert!(!location.is_empty(), "Location must be provided");
    assert!(!name.is_empty(), "Key name must be provided");

    let file_path: &str = &format!("{location}/{name}.key");

    // Create the directory if it does not exist
    if !Path::new(location).exists() {
      fs::create_dir_all(location)?;
    }

    // Check if the file already exists
    if Path::new(file_path).exists() && !self.force {
      return Err(anyhow::anyhow!("File {file_path} already exists. Aborting."));
    }

    let primary_user_id = format!("Me <{name}@mk.local>");
    let mut key_params = SecretKeyParamsBuilder::default();
    key_params
      .key_type(KeyType::Rsa(2048))
      .can_certify(false)
      .can_encrypt(true)
      .can_sign(true)
      .primary_user_id(primary_user_id);
    let private_key_params = key_params.build()?;
    let private_key = private_key_params.generate(thread_rng())?;

    // Use the private key to sign itself and put empty password
    let signed_private_key = private_key.sign(&mut thread_rng(), String::new)?;

    // Save the armored private key to a file
    let mut file = File::create(file_path)?;
    signed_private_key.to_armored_writer(&mut file, ArmorOptions::default())?;
    file.flush()?;
    println!("Private key saved to {}", file_path);

    Ok(())
  }
}
