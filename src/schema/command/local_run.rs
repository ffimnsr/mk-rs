use std::io::{
  BufRead as _,
  BufReader,
};
use std::process::{
  Command as ProcessCommand,
  Stdio,
};
use std::thread;

use anyhow::Context as _;
use serde::Deserialize;

use crate::defaults::{
  default_shell,
  default_true,
};
use crate::handle_output;
use crate::schema::TaskContext;

#[derive(Debug, Deserialize)]
pub struct LocalRun {
  /// The command to run
  pub command: String,

  /// The shell to use to run the command
  #[serde(default = "default_shell")]
  pub shell: String,

  /// The working directory to run the command in
  #[serde(default)]
  pub work_dir: Option<String>,

  /// Ignore errors if the command fails
  #[serde(default)]
  pub ignore_errors: bool,

  /// Show verbose output
  #[serde(default = "default_true")]
  pub verbose: bool,
}

impl LocalRun {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.command.is_empty());
    assert!(!self.shell.is_empty());

    let stdout = if self.verbose {
      Stdio::piped()
    } else {
      Stdio::null()
    };
    let stderr = if self.verbose {
      Stdio::piped()
    } else {
      Stdio::null()
    };

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

    if self.verbose {
      handle_output!(cmd.stdout, context);
      handle_output!(cmd.stderr, context);
    }

    let status = cmd.wait()?;
    if !status.success() && !self.ignore_errors {
      anyhow::bail!("Command failed - {}", self.command);
    }

    Ok(())
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
      assert!(!local_run.ignore_errors);
      assert!(!local_run.verbose);

      Ok(())
    }
  }
}
