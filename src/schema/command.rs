use serde::{Deserialize, Serialize};
use std::process::Command as ProcessCommand;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Command {
  pub command: String,

  #[serde(default)]
  pub ignore_errors: bool,

  #[serde(default)]
  pub verbose: bool,
}

impl Command {
  pub fn execute(&self) -> anyhow::Result<()> {
    let output = ProcessCommand::new("sh")
      .arg("-c")
      .arg(&self.command)
      .output()?;

    if self.verbose == true {
      println!("{}", String::from_utf8_lossy(&output.stdout));
      println!("{}", String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() && !self.ignore_errors {
      anyhow::bail!("Command failed: {}", self.command);
    }

    Ok(())
  }
}

mod test {
  use super::*;

  #[test]
  fn test_command() {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        ignore_errors: false
        verbose: false
      ";
      let command = serde_yaml::from_str::<Command>(yaml).unwrap();

      assert_eq!(command.command, "echo \"Hello, World!\"");
      assert_eq!(command.ignore_errors, false);
      assert_eq!(command.verbose, false);
    }

    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
      ";
      let command = serde_yaml::from_str::<Command>(yaml).unwrap();

      assert_eq!(command.command, "echo \"Hello, World!\"");
      assert_eq!(command.ignore_errors, false);
      assert_eq!(command.verbose, false);
    }

    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        ignore_errors: true
      ";
      let command = serde_yaml::from_str::<Command>(yaml).unwrap();

      assert_eq!(command.command, "echo \"Hello, World!\"");
      assert_eq!(command.ignore_errors, true);
      assert_eq!(command.verbose, false);
    }

    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        verbose: true
      ";
      let command = serde_yaml::from_str::<Command>(yaml).unwrap();

      assert_eq!(command.command, "echo \"Hello, World!\"");
      assert_eq!(command.ignore_errors, false);
      assert_eq!(command.verbose, true);
    }
  }
}
