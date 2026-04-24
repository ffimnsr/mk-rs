use clap::Args;

use mk_lib::secrets::{
  SecretBackend,
  SecretValueSource,
};

use crate::secrets::context::Context;

#[derive(Debug, Args)]
pub struct Doctor {}

impl Doctor {
  pub fn execute(&self, context: &Context) -> anyhow::Result<()> {
    let config = context.resolved_config();
    let active_config_path = context
      .active_config_path()
      .map(|path| path.to_string_lossy().to_string())
      .unwrap_or_else(|| String::from("<none>"));

    println!("Active config file: {active_config_path}");
    println!("Resolved backend: {}", render_backend(&config.backend));
    println!(
      "Resolved backend source: {}",
      render_source(config.backend_source)
    );
    println!("Resolved vault path: {}", config.vault_location.to_string_lossy());
    println!(
      "Resolved vault path source: {}",
      render_source(config.vault_location_source)
    );
    println!("Resolved keys path: {}", config.keys_location.to_string_lossy());
    println!(
      "Resolved keys path source: {}",
      render_source(config.keys_location_source)
    );
    println!("Resolved key name: {}", config.key_name);
    println!(
      "Resolved key name source: {}",
      render_source(config.key_name_source)
    );
    println!(
      "Resolved gpg key id: {}",
      config.gpg_key_id.as_deref().unwrap_or("<none>")
    );
    println!(
      "Resolved gpg key id source: {}",
      render_optional_source(config.gpg_key_id_source)
    );
    println!(
      "Vault metadata used: {}",
      if config.vault_meta_used { "yes" } else { "no" }
    );

    Ok(())
  }
}

fn render_backend(backend: &SecretBackend) -> &'static str {
  match backend {
    SecretBackend::BuiltInPgp => "pgp",
    SecretBackend::Gpg => "gpg",
  }
}

fn render_source(source: SecretValueSource) -> &'static str {
  match source {
    SecretValueSource::Cli => "cli",
    SecretValueSource::Task => "task",
    SecretValueSource::Root => "root",
    SecretValueSource::VaultMeta => "vault-meta",
    SecretValueSource::Default => "default",
  }
}

fn render_optional_source(source: Option<SecretValueSource>) -> &'static str {
  source.map(render_source).unwrap_or("<none>")
}
