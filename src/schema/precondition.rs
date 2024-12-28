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

  #[serde(default)]
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

fn default_shell() -> String {
  "sh".to_string()
}

mod test {
  #[allow(unused_imports)]
  use super::*;

  #[test]
  fn test_precondition() {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        message: 'This is a message'
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml).unwrap();

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, Some("This is a message".into()));
    }

    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml).unwrap();

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, None);
    }

    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        message: null
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml).unwrap();

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, None);
    }
  }
}
