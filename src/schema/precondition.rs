use anyhow::Context as _;
use serde::Deserialize;
use std::io::{
  BufRead as _,
  BufReader,
};
use std::process::Command as ProcessCommand;
use std::thread;

use super::TaskContext;
use crate::defaults::{
  default_shell,
  default_verbose,
};
use crate::handle_output;
use crate::schema::get_output_handler;

/// This struct represents a precondition that must be met before a task can be
/// executed.
#[derive(Debug, Default, Deserialize)]
pub struct Precondition {
  /// The command to run
  pub command: String,

  //// The message to display if the command fails
  #[serde(default)]
  pub message: Option<String>,

  /// The shell to use to run the command
  #[serde(default = "default_shell")]
  pub shell: String,

  /// The working directory to run the command in
  #[serde(default)]
  pub work_dir: Option<String>,

  /// Show verbose output
  #[serde(default)]
  pub verbose: Option<bool>,
}

impl Precondition {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.command.is_empty());
    assert!(!self.shell.is_empty());

    let verbose = self.verbose(context);

    let stdout = get_output_handler(verbose);
    let stderr = get_output_handler(verbose);

    let shell = &self.shell;
    let mut cmd = ProcessCommand::new(shell);
    cmd
      .arg("-c")
      .arg(self.command.clone())
      .stdout(stdout)
      .stderr(stderr);

    if self.work_dir.is_some() {
      cmd.current_dir(self.work_dir.as_ref().with_context(|| "Failed to get work_dir")?);
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
    if !status.success() {
      if let Some(message) = &self.message {
        anyhow::bail!("Precondition failed - {}", message);
      } else {
        anyhow::bail!("Precondition failed - {}", self.command);
      }
    }

    Ok(())
  }

  fn verbose(&self, context: &TaskContext) -> bool {
    self.verbose.or(context.verbose).unwrap_or(default_verbose())
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_precondition_1() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        message: 'This is a message'
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml)?;

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, Some("This is a message".into()));
      assert_eq!(precondition.shell, "sh");
      assert_eq!(precondition.work_dir, None);
      assert_eq!(precondition.verbose, None);

      Ok(())
    }
  }

  #[test]
  fn test_precondition_2() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml)?;

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, None);
      assert_eq!(precondition.shell, "sh");
      assert_eq!(precondition.work_dir, None);
      assert_eq!(precondition.verbose, None);

      Ok(())
    }
  }

  #[test]
  fn test_precondition_3() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        message: null
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml)?;

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, None);
      assert_eq!(precondition.shell, "sh");
      assert_eq!(precondition.work_dir, None);
      assert_eq!(precondition.verbose, None);

      Ok(())
    }
  }

  #[test]
  fn test_precondition_4() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        work_dir: /tmp
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml)?;

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, None);
      assert_eq!(precondition.shell, "sh");
      assert_eq!(precondition.work_dir, Some("/tmp".into()));
      assert_eq!(precondition.verbose, None);

      Ok(())
    }
  }

  #[test]
  fn test_precondition_5() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        verbose: true
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml)?;

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, None);
      assert_eq!(precondition.shell, "sh");
      assert_eq!(precondition.work_dir, None);
      assert_eq!(precondition.verbose, Some(true));

      Ok(())
    }
  }
}
