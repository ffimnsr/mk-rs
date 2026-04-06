use std::fs::{
  self,
  File,
};
use std::io::{
  self,
  IsTerminal,
  Read as _,
  Write as _,
};
use std::path::Path;

use clap::Args;
use mk_lib::file::ToUtf8 as _;
use mk_lib::secrets::{
  encrypt_with_gpg,
  read_vault_gpg_key_id,
};
use pgp::composed::{
  ArmorOptions,
  Deserializable,
  SignedSecretKey,
};
use pgp::crypto::sym::SymmetricKeyAlgorithm;
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

  #[arg(
    long,
    help = "GPG key ID or fingerprint for hardware/passphrase-protected keys. Cannot be combined with --key-name."
  )]
  gpg_key_id: Option<String>,

  /// If the secret already exists, it will be overwritten
  #[arg(short, long, help = "Force overwrite the secret")]
  force: bool,
}

impl StoreSecret {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let path = self.path.as_str();
    let value: String = match &self.value {
      Some(value) => value.clone(),
      None => {
        let stdin = io::stdin();
        if stdin.is_terminal() {
          return Err(anyhow::anyhow!(
            "No secret value provided. Pass a value as the second argument or pipe it via stdin."
          ));
        }

        let mut buffer = String::new();
        let mut handle = stdin.lock();
        match handle.read_to_string(&mut buffer) {
          Ok(0) => {
            return Err(anyhow::anyhow!(
              "No secret value provided. Pass a value as the second argument or pipe it via stdin."
            ))
          },
          Ok(_) => buffer.trim().to_string(),
          Err(e) => return Err(anyhow::anyhow!("Failed to read from stdin: {}", e)),
        }
      },
    };

    let context_vault_location;
    let vault_location = match self.vault_location.as_deref() {
      Some(vault_location) => vault_location,
      None => {
        context_vault_location = context.vault_location();
        context_vault_location.as_str()
      },
    };
    let context_keys_location;
    let keys_location = match self.keys_location.as_deref() {
      Some(keys_location) => keys_location,
      None => {
        context_keys_location = context.keys_location();
        context_keys_location.as_str()
      },
    };
    if self.key_name.is_some() && self.gpg_key_id.is_some() {
      anyhow::bail!("--key-name and --gpg-key-id are mutually exclusive");
    }
    let context_key_name;
    let key_name = match self.key_name.as_deref() {
      Some(key_name) => key_name,
      None => {
        context_key_name = context.key_name();
        context_key_name.as_str()
      },
    };
    let gpg_key_id = self.gpg_key_id.clone().or_else(|| context.gpg_key_id());

    assert!(!path.is_empty(), "Path must be provided");
    assert!(!value.is_empty(), "Value must be provided");
    assert!(!vault_location.is_empty(), "Store location must be provided");
    assert!(!keys_location.is_empty(), "Keys location must be provided");
    assert!(!key_name.is_empty(), "Key name must be provided");

    verify_vault(vault_location)?;
    // Auto-resolve gpg_key_id from vault metadata when not set by flag or context
    let gpg_key_id = gpg_key_id.or_else(|| read_vault_gpg_key_id(Path::new(vault_location)));
    if gpg_key_id.is_none() {
      verify_key(keys_location, key_name)?;
    }

    let secret_path = Path::new(vault_location).join(path);
    let data_path = secret_path.join("data.asc");
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
      fs::create_dir_all(&secret_path)?;

      if let Some(gpg_id) = &gpg_key_id {
        // GPG path: encrypt via system gpg binary (supports YubiKey and passphrase-protected keys)
        let encrypted = encrypt_with_gpg(gpg_id, value.as_bytes())?;
        let mut writer = File::create(data_path)?;
        writer.write_all(&encrypted)?;
        writer.flush()?;
      } else {
        // Built-in pgp path: key file must exist in keys_location
        let key_name = format!("{}.key", key_name);
        let key_path = Path::new(keys_location).join(key_name);
        let mut secret_key_string = File::open(key_path)?;
        let (signed_secret_key, _) = SignedSecretKey::from_armor_single(&mut secret_key_string)?;
        signed_secret_key.verify_bindings()?;

        // Get the public key (signed form implements PublicKeyTrait)
        let pubkey = signed_secret_key.to_public_key();

        // Encrypt the value using MessageBuilder and write armored output
        let mut rng = thread_rng();
        let builder = pgp::composed::MessageBuilder::from_bytes("", value.into_bytes())
          .seipd_v1(&mut rng, SymmetricKeyAlgorithm::AES128);
        // Add recipient public key(s)
        let mut builder = builder;
        builder.encrypt_to_key(&mut rng, &pubkey)?;
        let armored = builder.to_armored_string(&mut rng, ArmorOptions::default())?;

        // Save the armored encrypted message to a file
        let mut writer = File::create(data_path)?;
        write!(writer, "{}", armored)?;
        writer.flush()?;
      }

      println!("Secret stored at {}", secret_path.to_utf8()?);
    }
    Ok(())
  }
}
