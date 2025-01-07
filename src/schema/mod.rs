mod command;
mod include;
mod precondition;
mod task;
mod task_context;
mod task_dependency;
mod task_root;
mod use_cargo;
mod use_npm;

use std::collections::HashSet;
use std::sync::{
  Arc,
  Mutex,
};
use std::process::Stdio;

pub type ExecutionStack = Arc<Mutex<HashSet<String>>>;

pub use command::*;
pub use include::*;
pub use precondition::*;
pub use task::*;
pub use task_context::*;
pub use task_dependency::*;
pub use task_root::*;
pub use use_cargo::*;
pub use use_npm::*;

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

pub fn get_output_handler(verbose: bool) -> Stdio {
  if verbose {
    Stdio::piped()
  } else {
    Stdio::null()
  }
}
