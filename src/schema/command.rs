use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    process::{Command as ProcessCommand, Stdio},
};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Command {
    pub command: String,

    #[serde(default = "default_shell")]
    pub shell: String,

    #[serde(default)]
    pub ignore_errors: bool,

    #[serde(default)]
    pub verbose: bool,
}

impl Command {
    pub fn execute(&self, env_vars: Option<&HashMap<String, String>>) -> anyhow::Result<()> {
        let stdout = if self.verbose {
            Stdio::inherit()
        } else {
            Stdio::null()
        };
        let stderr = if self.verbose {
            Stdio::inherit()
        } else {
            Stdio::null()
        };

        let shell = &self.shell;
        let mut cmd = ProcessCommand::new(shell);
        cmd.arg("-c").arg(&self.command).stdout(stdout).stderr(stderr);

        if let Some(env_vars) = env_vars {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        let mut cmd = cmd.spawn()?;
        let status = cmd.wait()?;
        if !status.success() && !self.ignore_errors {
            anyhow::bail!("Command failed: {}", self.command);
        }

        Ok(())
    }
}

fn default_shell() -> String {
    return "sh".to_string();
}

mod test {
    #[allow(unused_imports)]
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
