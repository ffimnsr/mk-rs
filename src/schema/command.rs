use anyhow::Context;
use indicatif::MultiProgress;
use serde::{
    Deserialize,
    Serialize,
};
use std::collections::HashMap;
use std::io::{
    BufRead as _,
    BufReader,
};
use std::process::{
    Command as ProcessCommand,
    Stdio,
};
use std::sync::Arc;
use std::thread;

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
    pub fn execute(
        &self,
        multi: Arc<MultiProgress>,
        env_vars: Option<&HashMap<String, String>>,
    ) -> anyhow::Result<()> {
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
        cmd.arg("-c").arg(&self.command).stdout(stdout).stderr(stderr);

        // Inject environment variables
        if let Some(env_vars) = env_vars {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        let mut cmd = cmd.spawn()?;

        let stdout = cmd.stdout.take().with_context(|| "Failed to open stdout")?;
        let stderr = cmd.stderr.take().with_context(|| "Failed to open stderr")?;

        let multi_clone = multi.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let _ = multi_clone.println(line);
            }
        });

        let multi_clone = multi.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = multi_clone.println(line);
            }
        });

        let status = cmd.wait()?;
        if !status.success() && !self.ignore_errors {
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
    fn test_command() {
        {
            let yaml = "
        command: 'echo \"Hello, World!\"'
        ignore_errors: false
        verbose: false
      ";
            let command = serde_yaml::from_str::<Command>(yaml).unwrap();

            assert_eq!(command.command, "echo \"Hello, World!\"");
            assert!(!command.ignore_errors);
            assert!(!command.verbose);
        }

        {
            let yaml = "
        command: 'echo \"Hello, World!\"'
      ";
            let command = serde_yaml::from_str::<Command>(yaml).unwrap();

            assert_eq!(command.command, "echo \"Hello, World!\"");
            assert!(!command.ignore_errors);
            assert!(!command.verbose);
        }

        {
            let yaml = "
        command: 'echo \"Hello, World!\"'
        ignore_errors: true
      ";
            let command = serde_yaml::from_str::<Command>(yaml).unwrap();

            assert_eq!(command.command, "echo \"Hello, World!\"");
            assert!(command.ignore_errors);
            assert!(!command.verbose);
        }

        {
            let yaml = "
        command: 'echo \"Hello, World!\"'
        verbose: true
      ";
            let command = serde_yaml::from_str::<Command>(yaml).unwrap();

            assert_eq!(command.command, "echo \"Hello, World!\"");
            assert!(!command.ignore_errors);
            assert!(command.verbose);
        }
    }
}
