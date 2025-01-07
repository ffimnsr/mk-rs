use std::io::{
  BufRead as _,
  BufReader,
};
use std::process::Command as ProcessCommand;
use std::thread;

use anyhow::Context as _;
use serde::Deserialize;

use crate::defaults::{
  default_ignore_errors,
  default_shell,
  default_verbose,
};
use crate::handle_output;
use crate::schema::{
  get_output_handler,
  TaskContext,
};

#[derive(Debug, Deserialize)]
pub struct LocalRun {
  /// The command to run
  pub command: String,

  /// The shell to use to run the command
  #[serde(default = "default_shell")]
  pub shell: String,

  /// The test to run before running command
  /// If the test fails, the command will not run
  #[serde(default)]
  pub test: Option<String>,

  /// The working directory to run the command in
  #[serde(default)]
  pub work_dir: Option<String>,

  /// Ignore errors if the command fails
  #[serde(default)]
  pub ignore_errors: Option<bool>,

  /// Show verbose output
  #[serde(default)]
  pub verbose: Option<bool>,
}

impl LocalRun {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.command.is_empty());
    assert!(!self.shell.is_empty());

    let ignore_errors = self.ignore_errors(context);
    let verbose = self.verbose(context);

    // Skip the command if the test fails
    if self.test(context).is_err() {
      return Ok(());
    }

    let stdout = get_output_handler(verbose);
    let stderr = get_output_handler(verbose);

    let mut cmd = ProcessCommand::new(&self.shell);
    cmd.arg("-c").arg(&self.command).stdout(stdout).stderr(stderr);

    if let Some(work_dir) = &self.work_dir.clone() {
      cmd.current_dir(work_dir);
    }

    // Inject environment variables
    for (key, value) in context.env_vars.iter() {
      cmd.env(key, value);
    }

    let mut cmd = cmd.spawn()?;
    if verbose {
      handle_output!(cmd.stdout, context);
      handle_output!(cmd.stderr, context);
    }

    let status = cmd.wait()?;
    if !status.success() && !ignore_errors {
      anyhow::bail!("Command failed - {}", self.command);
    }

    Ok(())
  }

  fn test(&self, context: &TaskContext) -> anyhow::Result<()> {
    let verbose = self.verbose(context);

    let stdout = get_output_handler(verbose);
    let stderr = get_output_handler(verbose);

    if let Some(test) = &self.test {
      let mut cmd = ProcessCommand::new(&self.shell);
      cmd.arg("-c").arg(test).stdout(stdout).stderr(stderr);

      let mut cmd = cmd.spawn()?;
      if verbose {
        handle_output!(cmd.stdout, context);
        handle_output!(cmd.stderr, context);
      }

      let status = cmd.wait()?;

      log::trace!("Test status: {:?}", status.success());
      if !status.success() {
        anyhow::bail!("Command test failed - {}", test);
      }
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

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_local_run_1() -> anyhow::Result<()> {
    {
      let yaml = "
        command: echo 'Hello, World!'
        ignore_errors: false
        verbose: false
      ";
      let local_run = serde_yaml::from_str::<LocalRun>(yaml)?;

      assert_eq!(local_run.command, "echo 'Hello, World!'");
      assert_eq!(local_run.shell, "sh");
      assert_eq!(local_run.work_dir, None);
      assert_eq!(local_run.ignore_errors, Some(false));
      assert_eq!(local_run.verbose, Some(false));

      Ok(())
    }
  }

  #[test]
  fn test_local_run_2() -> anyhow::Result<()> {
    {
      let yaml = "
        command: echo 'Hello, World!'
        test: test $(uname) = 'Linux'
        ignore_errors: false
        verbose: false
      ";
      let local_run = serde_yaml::from_str::<LocalRun>(yaml)?;

      assert_eq!(local_run.command, "echo 'Hello, World!'");
      assert_eq!(local_run.test, Some("test $(uname) = 'Linux'".to_string()));
      assert_eq!(local_run.shell, "sh");
      assert_eq!(local_run.work_dir, None);
      assert_eq!(local_run.ignore_errors, Some(false));
      assert_eq!(local_run.verbose, Some(false));

      Ok(())
    }
  }
}
