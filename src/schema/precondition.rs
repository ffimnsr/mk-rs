use anyhow::Context as _;
use serde::{
  Deserialize,
  Serialize,
};
use std::io::{
  BufRead as _,
  BufReader,
};
use std::process::{
  Command as ProcessCommand,
  Stdio,
};
use std::thread;

use super::TaskContext;
use crate::defaults::{
  default_shell,
  default_true,
};

/// This struct represents a precondition that must be met before a task can be
/// executed.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Precondition {
  pub command: String,

  #[serde(default)]
  pub message: Option<String>,

  #[serde(default = "default_shell")]
  pub shell: String,

  #[serde(default)]
  pub work_dir: Option<String>,

  #[serde(default = "default_true")]
  pub verbose: bool,
}

impl Precondition {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
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

    if self.verbose {
      let stdout = cmd.stdout.take().with_context(|| "Failed to open stdout")?;
      let stderr = cmd.stderr.take().with_context(|| "Failed to open stderr")?;

      let multi_clone = context.multi.clone();
      thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
          let _ = multi_clone.println(line);
        }
      });

      let multi_clone = context.multi.clone();
      thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
          let _ = multi_clone.println(line);
        }
      });
    }

    let status = cmd.wait()?;
    if !status.success() {
      anyhow::bail!("Command failed: {}", self.command);
    }

    Ok(())
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
      assert!(precondition.verbose);

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
      assert!(precondition.verbose);

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
      assert!(precondition.verbose);

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
      assert!(precondition.verbose);

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
      assert!(precondition.verbose);

      Ok(())
    }
  }
}
