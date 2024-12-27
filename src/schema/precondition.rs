use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    process::{Command as ProcessCommand, Stdio},
};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Precondition {
    pub command: String,

    #[serde(default)]
    pub message: Option<String>,

    #[serde(default = "default_shell")]
    pub shell: String,

    #[serde(default)]
    pub verbose: bool,
}

impl Precondition {
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
        if !status.success() {
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
