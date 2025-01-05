use std::io::{
  BufRead as _,
  BufReader,
};
use std::process::{
  Command as ProcessCommand,
  Stdio,
};
use std::{
  env,
  thread,
};

use anyhow::Context as _;
use serde::Deserialize;
use which::which;

use crate::defaults::{
  default_ignore_errors,
  default_verbose,
};
use crate::file::ToUtf8 as _;
use crate::handle_output;
use crate::schema::TaskContext;

#[derive(Debug, Deserialize)]
pub struct ContainerRun {
  /// The command to run in the container
  pub container_command: Vec<String>,

  /// The container image to use
  pub image: String,

  /// The mounted paths to bind mount into the container
  #[serde(default)]
  pub mounted_paths: Vec<String>,

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

    let stdout = if verbose { Stdio::piped() } else { Stdio::null() };
    let stderr = if verbose { Stdio::piped() } else { Stdio::null() };

    let container_runtime = which("docker")
      .or_else(|_| which("podman"))
      .with_context(|| "Failed to find docker or podman")?;

    let mut cmd = ProcessCommand::new(container_runtime);
    cmd.arg("run").arg("--rm").arg("-i").stdout(stdout).stderr(stderr);

    let current_dir = env::current_dir()?;
    cmd
      .arg("-v")
      .arg(format!("{}:/workdir:z", current_dir.to_utf8()?));
    cmd.arg("-w").arg("/workdir");

    for mounted_path in self.mounted_paths.clone() {
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
}
