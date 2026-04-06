use std::io::{
  BufRead as _,
  BufReader,
};
use std::process::Command as ProcessCommand;
use std::thread;

use anyhow::Context;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::defaults::{
  default_ignore_errors,
  default_verbose,
};
use crate::file::ToUtf8 as _;
use crate::handle_output;
use crate::schema::{
  get_output_handler,
  ContainerRuntime,
  TaskContext,
};

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub struct ContainerRun {
  /// The command to run in the container
  pub container_command: Vec<String>,

  /// The container image to use
  pub image: String,

  /// The mounted paths to bind mount into the container
  #[serde(default)]
  pub mounted_paths: Vec<String>,

  /// The container runtime to use
  #[serde(default)]
  pub runtime: Option<ContainerRuntime>,

  /// Ignore errors if the command fails
  #[serde(default)]
  pub ignore_errors: Option<bool>,

  /// Show verbose output
  #[serde(default)]
  pub verbose: Option<bool>,
}

impl ContainerRun {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.image.is_empty());
    assert!(!self.container_command.is_empty());

    let ignore_errors = self.ignore_errors(context);
    let verbose = self.verbose(context);

    let stdout = get_output_handler(verbose);
    let stderr = get_output_handler(verbose);

    let container_runtime =
      ContainerRuntime::resolve(self.runtime.as_ref().or(context.container_runtime.as_ref()))?;

    let mut cmd = ProcessCommand::new(container_runtime);
    cmd.arg("run").arg("--rm").arg("-i").stdout(stdout).stderr(stderr);

    let workdir = context.task_root.config_base_dir();
    cmd.arg("-v").arg(format!("{}:/workdir:z", workdir.to_utf8()?));
    cmd.arg("-w").arg("/workdir");

    for mounted_path in self.resolved_mounted_paths(context) {
      cmd.arg("-v").arg(mounted_path);
    }

    // Inject environment variables in both container and command
    for (key, value) in context.env_vars.iter() {
      cmd.env(key, value);
      cmd.arg("-e").arg(format!("{}={}", key, value));
    }

    cmd.arg(&self.image).args(&self.container_command);

    log::trace!("Running command: {:?}", cmd);

    let mut cmd = cmd.spawn()?;
    if verbose {
      handle_output!(cmd.stdout, context);
      handle_output!(cmd.stderr, context);
    }

    let status = cmd.wait()?;
    if !status.success() && !ignore_errors {
      anyhow::bail!("Command failed - {}", self.container_command.join(" "));
    }

    Ok(())
  }

  fn ignore_errors(&self, context: &TaskContext) -> bool {
    self
      .ignore_errors
      .or(context.ignore_errors)
      .unwrap_or(default_ignore_errors())
  }

  fn verbose(&self, context: &TaskContext) -> bool {
    self.verbose.or(context.verbose).unwrap_or(default_verbose())
  }

  pub fn resolved_mounted_paths(&self, context: &TaskContext) -> Vec<String> {
    self
      .mounted_paths
      .iter()
      .map(|mounted_path| resolve_mount_spec(context, mounted_path))
      .collect()
  }
}

fn resolve_mount_spec(context: &TaskContext, mounted_path: &str) -> String {
  let mut parts = mounted_path.splitn(3, ':');
  let host = parts.next().unwrap_or_default();
  let second = parts.next();
  let third = parts.next();

  if let Some(container_path) = second {
    if !should_resolve_bind_host(host, container_path) {
      return mounted_path.to_string();
    }

    let resolved_host = context.resolve_from_config(host);
    match third {
      Some(options) => format!(
        "{}:{}:{}",
        resolved_host.to_string_lossy(),
        container_path,
        options
      ),
      None => format!("{}:{}", resolved_host.to_string_lossy(), container_path),
    }
  } else {
    mounted_path.to_string()
  }
}

fn should_resolve_bind_host(host: &str, container_path: &str) -> bool {
  if host.is_empty() || container_path.is_empty() {
    return false;
  }

  host.starts_with('.')
    || host.starts_with('/')
    || host.contains('/')
    || host == "~"
    || host.starts_with("~/")
}
