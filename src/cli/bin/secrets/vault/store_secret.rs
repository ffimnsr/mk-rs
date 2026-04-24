use clap::Args;
use mk_lib::file::DisplayPath as _;
use mk_lib::secrets::{
  encrypt_with_gpg,
  verify_vault,
  SecretBackend,
};
use pgp::composed::{
  ArmorOptions,
  Deserializable,
  SignedSecretKey,
};
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use rand::thread_rng;
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

use crate::secrets::context::Context;
use crate::secrets::vault::verify_key;

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

  #[arg(short, long, help = "The key name", conflicts_with = "gpg_key_id")]
  key_name: Option<String>,

  #[arg(
    long,
    conflicts_with = "key_name",
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

    let mut cli_overrides = context.settings().clone();
    if let Some(vault_location) = &self.vault_location {
      cli_overrides.vault_location = Some(vault_location.clone());
    }
    if let Some(keys_location) = &self.keys_location {
      cli_overrides.keys_location = Some(keys_location.clone());
    }
    if let Some(key_name) = &self.key_name {
      cli_overrides.key_name = Some(key_name.clone());
    }
    if let Some(gpg_key_id) = &self.gpg_key_id {
      cli_overrides.gpg_key_id = Some(gpg_key_id.clone());
    }
    let secret_config = context.resolve_with_settings(&cli_overrides);
    let vault_location = secret_config.vault_location.to_string_lossy().to_string();
    let keys_location = secret_config.keys_location.to_string_lossy().to_string();
    let key_name = secret_config.key_name.clone();
    let gpg_key_id = secret_config.gpg_key_id.clone();

    assert!(!path.is_empty(), "Path must be provided");
    assert!(!value.is_empty(), "Value must be provided");
    assert!(!vault_location.is_empty(), "Store location must be provided");
    assert!(!keys_location.is_empty(), "Keys location must be provided");
    assert!(!key_name.is_empty(), "Key name must be provided");

    verify_vault(Path::new(&vault_location))?;
    let backend = secret_config.backend.clone();
    if matches!(backend, SecretBackend::BuiltInPgp) {
      verify_key(&keys_location, &key_name)?;
    }

    let secret_path = Path::new(&vault_location).join(path);
    let data_path = secret_path.join("data.asc");
    if secret_path.exists()
      && secret_path.is_dir()
      && data_path.exists()
      && data_path.is_file()
      && !self.force
    {
      println!(
        "Secret already exists at path {path} in {}",
        secret_path.display_lossy()
      );
    } else {
      fs::create_dir_all(&secret_path)?;

      if matches!(backend, SecretBackend::Gpg) {
        let gpg_id = gpg_key_id
          .as_deref()
          .ok_or_else(|| anyhow::anyhow!("GPG backend selected but no gpg_key_id is configured"))?;
        // GPG path: encrypt via system gpg binary (supports YubiKey and passphrase-protected keys)
        let encrypted = encrypt_with_gpg(gpg_id, value.as_bytes())?;
        let mut writer = File::create(data_path)?;
        writer.write_all(&encrypted)?;
        writer.flush()?;
      } else {
        // Built-in pgp path: key file must exist in keys_location
        let key_name = format!("{}.key", key_name);
        let key_path = Path::new(&keys_location).join(key_name);
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

      println!("Secret stored at {}", secret_path.display_lossy());
    }
    Ok(())
  }
}
