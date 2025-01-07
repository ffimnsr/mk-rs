use std::io::{
  BufRead as _,
  BufReader,
};
use std::process::Command as ProcessCommand;

use std::thread;

use crate::handle_output;
use crate::schema::get_output_handler;
use anyhow::Context;

use super::TaskContext;
use serde::Deserialize;

mod container_build;
mod container_run;
mod local_run;
mod task_run;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CommandRunner {
  ContainerBuild(container_build::ContainerBuild),
  ContainerRun(container_run::ContainerRun),
  LocalRun(local_run::LocalRun),
  TaskRun(task_run::TaskRun),
  CommandRun(String),
}

impl CommandRunner {
  pub fn execute(&self, context: &mut TaskContext) -> anyhow::Result<()> {
    match self {
      CommandRunner::ContainerBuild(container_build) => container_build.execute(context),
      CommandRunner::ContainerRun(container_run) => container_run.execute(context),
      CommandRunner::LocalRun(local_run) => local_run.execute(context),
      CommandRunner::TaskRun(task_run) => task_run.execute(context),
      CommandRunner::CommandRun(command) => self.execute_command(context, command),
    }
  }

  fn execute_command(&self, context: &TaskContext, command: &str) -> anyhow::Result<()> {
    assert!(!command.is_empty());

    let ignore_errors = context.ignore_errors();
    let verbose = context.verbose();
    let shell: &str = &context.shell();

    let stdout = get_output_handler(verbose);
    let stderr = get_output_handler(verbose);

    let mut cmd = ProcessCommand::new(shell);
    cmd.arg("-c").arg(command).stdout(stdout).stderr(stderr);

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
      anyhow::bail!("Command failed - {}", command);
    }

    Ok(())
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_command_1() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        ignore_errors: false
        verbose: false
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;

      if let CommandRunner::LocalRun(local_run) = command {
        assert_eq!(local_run.command, "echo \"Hello, World!\"");
        assert_eq!(local_run.shell, "sh");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.ignore_errors, Some(false));
        assert_eq!(local_run.verbose, Some(false));
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }

  #[test]
  fn test_command_2() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;

      if let CommandRunner::LocalRun(local_run) = command {
        assert_eq!(local_run.command, "echo \"Hello, World!\"");
        assert_eq!(local_run.shell, "sh");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.ignore_errors, None);
        assert_eq!(local_run.verbose, None);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }

  #[test]
  fn test_command_3() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        ignore_errors: true
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;
      if let CommandRunner::LocalRun(local_run) = command {
        assert_eq!(local_run.command, "echo \"Hello, World!\"");
        assert_eq!(local_run.shell, "sh");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.ignore_errors, Some(true));
        assert_eq!(local_run.verbose, None);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }

  #[test]
  fn test_command_4() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        verbose: false
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;
      if let CommandRunner::LocalRun(local_run) = command {
        assert_eq!(local_run.command, "echo \"Hello, World!\"");
        assert_eq!(local_run.shell, "sh");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.ignore_errors, None);
        assert_eq!(local_run.verbose, Some(false));
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }

  #[test]
  fn test_command_5() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        work_dir: /tmp
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;
      if let CommandRunner::LocalRun(local_run) = command {
        assert_eq!(local_run.command, "echo \"Hello, World!\"");
        assert_eq!(local_run.shell, "sh");
        assert_eq!(local_run.work_dir, Some("/tmp".into()));
        assert_eq!(local_run.ignore_errors, None);
        assert_eq!(local_run.verbose, None);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }

  #[test]
  fn test_command_6() -> anyhow::Result<()> {
    {
      let yaml = "
        echo 'Hello, World!'
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;
      if let CommandRunner::CommandRun(command) = command {
        assert_eq!(command, "echo 'Hello, World!'");
      } else {
        panic!("Expected CommandRunner::CommandRun");
      }

      Ok(())
    }
  }
}
