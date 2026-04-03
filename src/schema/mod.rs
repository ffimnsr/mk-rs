mod command;
mod include;
mod plan;
mod precondition;
mod shell;
mod task;
mod task_context;
mod task_dependency;
mod task_root;
mod use_cargo;
mod use_npm;
mod validation;

use std::collections::HashSet;
use std::process::Stdio;
use std::sync::{
  Arc,
  Mutex,
};

pub type ActiveTasks = Arc<Mutex<HashSet<String>>>;
pub type CompletedTasks = Arc<Mutex<HashSet<String>>>;

pub use command::*;
pub use include::*;
pub use plan::*;
pub use precondition::*;
pub use shell::*;
pub use task::*;
pub use task_context::*;
pub use task_dependency::*;
pub use task_root::*;
pub use use_cargo::*;
pub use use_npm::*;
pub use validation::*;

use crate::secrets::load_secret_value;

pub fn is_shell_command(value: &str) -> anyhow::Result<bool> {
  use regex::Regex;

  let re = Regex::new(r"^\$\(.+\)$")?;
  Ok(re.is_match(value))
}

pub fn is_template_command(value: &str) -> anyhow::Result<bool> {
  use regex::Regex;

  let re = Regex::new(r"^\$\{\{.+\}\}$")?;
  Ok(re.is_match(value))
}

pub fn resolve_template_command_value(value: &str, context: &TaskContext) -> anyhow::Result<String> {
  let value = value.trim_start_matches("${{").trim_end_matches("}}").trim();
  if value.starts_with("env.") {
    let value = value.trim_start_matches("env.");
    let value = context
      .env_vars
      .get(value)
      .ok_or_else(|| anyhow::anyhow!("Failed to find environment variable"))?;
    Ok(value.to_string())
  } else if value.starts_with("secrets.") {
    let path = value.trim_start_matches("secrets.");
    load_secret_value(
      path,
      &context.task_root.config_base_dir(),
      context.secret_vault_location.as_deref(),
      context.secret_keys_location.as_deref(),
      context.secret_key_name.as_deref(),
    )
  } else {
    Ok(value.to_string())
  }
}

pub fn get_output_handler(verbose: bool) -> Stdio {
  if verbose {
    Stdio::piped()
  } else {
    Stdio::null()
  }
}
