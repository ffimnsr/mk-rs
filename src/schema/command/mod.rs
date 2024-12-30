use super::TaskContext;
use serde::Deserialize;

mod container_run;
mod local_run;
mod task_run;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CommandRunner {
  ContainerRun(container_run::ContainerRun),
  LocalRun(local_run::LocalRun),
  TaskRun(task_run::TaskRun),
}

impl CommandRunner {
  pub fn execute(&self, context: &mut TaskContext) -> anyhow::Result<()> {
    match self {
      CommandRunner::ContainerRun(container_run) => container_run.execute(context),
      CommandRunner::LocalRun(local_run) => local_run.execute(context),
      CommandRunner::TaskRun(task_run) => task_run.execute(context),
    }
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
        assert!(!local_run.ignore_errors);
        assert!(!local_run.verbose);
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
        assert!(!local_run.ignore_errors);
        assert!(local_run.verbose);
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
        assert!(local_run.ignore_errors);
        assert!(local_run.verbose);
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
        assert!(!local_run.ignore_errors);
        assert!(!local_run.verbose);
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
        assert!(!local_run.ignore_errors);
        assert!(local_run.verbose);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }
}
