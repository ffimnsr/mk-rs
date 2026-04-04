use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::Context as _;
use clap::Args;
use mk_lib::file::ToUtf8 as _;

use crate::secrets::context::Context;

use super::KEY_LOCATION_HELP;

#[derive(Debug, Args)]
pub struct ImportKey {
  /// The GPG key ID or fingerprint to import from the local keyring
  #[arg(
    long,
    help = "GPG key ID or fingerprint from your local keyring (e.g. a YubiKey-backed key)"
  )]
  gpg: String,

  /// The location where the key reference will be stored
  #[arg(short, long, help = KEY_LOCATION_HELP)]
  location: Option<String>,

  /// The name assigned to this key reference (default: "default")
  #[arg(short, long, help = "The key name")]
  name: Option<String>,
}

impl ImportKey {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let location: &str = &self.location.clone().unwrap_or_else(|| context.keys_location());
    let name: &str = &self.name.clone().unwrap_or("default".to_string());
    let gpg_key_id: &str = &self.gpg;

    println!("Importing GPG key '{gpg_key_id}' as '{name}' into {location}");

    // Verify the key exists in the local GPG keyring
    let check = Command::new("gpg")
      .args(["--batch", "--list-keys", gpg_key_id])
      .output()
      .context("Failed to run gpg — is it installed and available in PATH?")?;
    if !check.status.success() {
      let stderr = String::from_utf8_lossy(&check.stderr);
      anyhow::bail!("GPG key '{}' not found in keyring: {}", gpg_key_id, stderr.trim());
    }

    // Create the keys directory if it does not exist
    let location_path = Path::new(location);
    if !location_path.exists() {
      fs::create_dir_all(location_path)?;
    }

    // Write a .gpg metadata file containing just the key ID/fingerprint.
    // The vault uses this to know which GPG key to call when gpg_key_id is set.
    let meta_path = location_path.join(format!("{name}.gpg"));
    let mut meta_file = fs::File::create(&meta_path)?;
    writeln!(meta_file, "{gpg_key_id}")?;
    meta_file.flush()?;
    println!("Key reference saved to {}", meta_path.to_utf8()?);

    // Export and store the public key (ASCII-armored) for auditing / re-encryption
    let pub_path = location_path.join(format!("{name}.pub"));
    let export = Command::new("gpg")
      .args(["--batch", "--export", "--armor", gpg_key_id])
      .output()
      .context("Failed to export GPG public key")?;
    if !export.status.success() {
      let stderr = String::from_utf8_lossy(&export.stderr);
      anyhow::bail!("Failed to export GPG public key: {}", stderr.trim());
    }
    let mut pub_file = fs::File::create(&pub_path)?;
    pub_file.write_all(&export.stdout)?;
    pub_file.flush()?;
    println!("Public key exported to {}", pub_path.to_utf8()?);

    println!();
    println!("To use this key, add the following to your tasks.yaml:");
    println!("  gpg_key_id: {gpg_key_id}");

    Ok(())
  }
}
