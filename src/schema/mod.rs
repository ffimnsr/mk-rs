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
use std::fmt;
use std::process::Stdio;
use std::sync::{
  Arc,
  Mutex,
};

use once_cell::sync::Lazy;
use regex::Regex;

pub type ActiveTasks = Arc<Mutex<HashSet<String>>>;
pub type CompletedTasks = Arc<Mutex<HashSet<String>>>;

#[derive(Debug)]
pub struct ExecutionInterrupted;

impl fmt::Display for ExecutionInterrupted {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Execution interrupted")
  }
}

impl std::error::Error for ExecutionInterrupted {}

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

static TEMPLATE_COMMAND_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^\$\{\{.+\}\}$").expect("valid template regex"));
static TEMPLATE_EXPR_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"\$\{\{\s*(.+?)\s*\}\}").expect("valid template expression regex"));

pub fn is_shell_command(value: &str) -> anyhow::Result<bool> {
  let re = Regex::new(r"^\$\(.+\)$")?;
  Ok(re.is_match(value))
}

pub fn is_template_command(value: &str) -> anyhow::Result<bool> {
  Ok(TEMPLATE_COMMAND_RE.is_match(value))
}

pub fn resolve_template_command_value(value: &str, context: &TaskContext) -> anyhow::Result<String> {
  let value = value.trim_start_matches("${{").trim_end_matches("}}").trim();
  resolve_template_expression(value, context)
}

pub fn resolve_template_expression(value: &str, context: &TaskContext) -> anyhow::Result<String> {
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
  } else if value.starts_with("outputs.") {
    let name = value.trim_start_matches("outputs.");
    context
      .get_task_output(name)?
      .ok_or_else(|| anyhow::anyhow!("Failed to find task output - {}", name))
  } else {
    Ok(value.to_string())
  }
}

pub fn interpolate_template_string(value: &str, context: &TaskContext) -> anyhow::Result<String> {
  let mut result = String::with_capacity(value.len());
  let mut last_end = 0usize;
  for captures in TEMPLATE_EXPR_RE.captures_iter(value) {
    let Some(full_match) = captures.get(0) else {
      continue;
    };
    let Some(expr) = captures.get(1) else {
      continue;
    };
    result.push_str(&value[last_end..full_match.start()]);
    result.push_str(&resolve_template_expression(expr.as_str().trim(), context)?);
    last_end = full_match.end();
  }
  result.push_str(&value[last_end..]);
  Ok(result)
}

pub fn extract_output_references(value: &str) -> Vec<String> {
  TEMPLATE_EXPR_RE
    .captures_iter(value)
    .filter_map(|captures| captures.get(1))
    .map(|expr| expr.as_str().trim())
    .filter_map(|expr| expr.strip_prefix("outputs."))
    .map(str::to_string)
    .collect()
}

pub fn contains_output_reference(value: &str) -> bool {
  !extract_output_references(value).is_empty()
}

pub fn get_output_handler(verbose: bool) -> Stdio {
  if verbose {
    Stdio::piped()
  } else {
    Stdio::null()
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use super::*;

  #[test]
  fn test_interpolate_template_string_resolves_outputs() -> anyhow::Result<()> {
    let root = Arc::new(TaskRoot::default());
    let context = TaskContext::empty_with_root(root);
    context.insert_task_output("version", "v1.2.3")?;
    assert_eq!(
      interpolate_template_string("tag=${{ outputs.version }}", &context)?,
      "tag=v1.2.3"
    );
    Ok(())
  }

  #[test]
  fn test_extract_output_references_finds_all_output_templates() {
    assert_eq!(
      extract_output_references("${{ outputs.first }}-${{ outputs.second }}"),
      vec!["first".to_string(), "second".to_string()]
    );
  }
}
