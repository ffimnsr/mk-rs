use std::fs::{
  self,
  File,
};
use std::io::{
  self,
  Read as _,
  Write as _,
};
use std::path::Path;

use clap::Args;
use mk_lib::file::ToUtf8 as _;
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::types::SecretKeyTrait;
use pgp::{
  ArmorOptions,
  Deserializable,
  Message,
  SignedSecretKey,
};
use rand::thread_rng;

use crate::secrets::context::Context;
use crate::secrets::vault::{
  verify_key,
  verify_vault,
};

#[derive(Debug, Args)]
pub struct StoreSecret {
  #[arg(help = "The secret identifier")]
  path: String,

  #[arg(help = "The secret value")]
  value: Option<String>,

  #[arg(short, long, help = "The path to the secret vault")]
  vault_location: Option<String>,

  #[arg(long, help = "The key location")]
  keys_location: Option<String>,

  #[arg(short, long, help = "The key name")]
  key_name: Option<String>,

  /// If the secret already exists, it will be overwritten
  #[arg(short, long, help = "Force overwrite the secret")]
  force: bool,
}

impl StoreSecret {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let path: &str = &self.path.clone();
    let value: &str = &match &self.value {
      Some(value) => value.clone(),
      None => {
        if atty::is(atty::Stream::Stdin) {
          return Err(anyhow::anyhow!("No value provided"));
        }

        let mut buffer = String::new();
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        match handle.read_to_string(&mut buffer) {
          Ok(0) => return Err(anyhow::anyhow!("No value provided")),
          Ok(_) => buffer.trim().to_string(),
          Err(e) => return Err(anyhow::anyhow!("Failed to read from stdin: {}", e)),
        }
      },
    };

    let vault_location: &str = &self
      .vault_location
      .clone()
      .unwrap_or_else(|| context.vault_location());
    let keys_location: &str = &self
      .keys_location
      .clone()
      .unwrap_or_else(|| context.keys_location());
    let key_name: &str = &self.key_name.clone().unwrap_or_else(|| context.key_name());

    assert!(!path.is_empty(), "Path must be provided");
    assert!(!value.is_empty(), "Value must be provided");
    assert!(!vault_location.is_empty(), "Store location must be provided");
    assert!(!keys_location.is_empty(), "Keys location must be provided");
    assert!(!key_name.is_empty(), "Key name must be provided");

    verify_vault(vault_location)?;
    verify_key(keys_location, key_name)?;

    let secret_path = Path::new(vault_location).join(path);
    let data_path = secret_path.clone().join("data.asc");
    if secret_path.exists()
      && secret_path.is_dir()
      && data_path.exists()
      && data_path.is_file()
      && !self.force
    {
      println!(
        "Secret already exists at path {path} in {}",
        secret_path.to_utf8()?
      );
    } else {
      fs::create_dir_all(secret_path.clone())?;

      // Open the secret key file
      let key_name = format!("{}.key", key_name);
      let key_path = Path::new(keys_location).join(key_name);
      let mut secret_key_string = File::open(key_path)?;
      let (signed_secret_key, _) = SignedSecretKey::from_armor_single(&mut secret_key_string)?;
      signed_secret_key.verify()?;

      // Get the public key
      let pubkey = signed_secret_key.public_key();

      // Encrypt the value
      let message = Message::new_literal("none", value);
      let encrypted_message =
        message.encrypt_to_keys_seipdv1(&mut thread_rng(), SymmetricKeyAlgorithm::AES128, &[&pubkey])?;

      // Save the encrypted message to a file
      let mut writer = File::create(data_path)?;
      encrypted_message.to_armored_writer(&mut writer, ArmorOptions::default())?;
      writer.flush()?;

      println!("Secret stored at {}", secret_path.to_utf8()?);
    }
    Ok(())
  }
}
