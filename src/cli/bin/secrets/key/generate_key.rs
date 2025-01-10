use std::fs::{
  self,
  File,
};
use std::io::Write as _;
use std::path::Path;

use clap::Args;
use mk_lib::file::ToUtf8;
use pgp::{
  ArmorOptions,
  KeyType,
  SecretKeyParamsBuilder,
};
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
    println!("Generating key {} at {}", name, location);

    assert!(!location.is_empty(), "Location must be provided");
    assert!(!name.is_empty(), "Key name must be provided");

    // Create the directory if it does not exist
    let location = Path::new(location);
    if !location.exists() {
      fs::create_dir_all(location)?;
    }

    let filename_with_ext: &str = &format!("{name}.key");
    // Check if the file already exists
    let file_path = location.join(filename_with_ext);
    if file_path.exists() && !self.force {
      return Err(anyhow::anyhow!(
        "File {} already exists. Aborting.",
        file_path.to_utf8()?
      ));
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
    let mut file = File::create(file_path.clone())?;
    signed_private_key.to_armored_writer(&mut file, ArmorOptions::default())?;
    file.flush()?;
    println!("Key saved to {}", file_path.to_utf8()?);

    Ok(())
  }
}
