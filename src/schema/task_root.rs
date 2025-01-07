use anyhow::Context;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use super::{
  Include,
  Task,
  UseCargo,
  UseNpm,
};

/// This struct represents the root of the task schema. It contains all the tasks
/// that can be executed.
#[derive(Debug, Default, Deserialize)]
pub struct TaskRoot {
  /// The tasks that can be executed
  pub tasks: HashMap<String, Task>,

  /// This allows mk to use npm scripts as tasks
  #[serde(default)]
  pub use_npm: Option<UseNpm>,

  /// This allows mk to use cargo commands as tasks
  #[serde(default)]
  pub use_cargo: Option<UseCargo>,

  /// Includes additional files to be merged into the current file
  #[serde(default)]
  pub include: Option<Vec<Include>>,
}

impl TaskRoot {
  pub fn from_file(file: &str) -> anyhow::Result<Self> {
    let file = File::open(file).with_context(|| format!("Failed to open file - {}", file))?;
    let reader = BufReader::new(file);
    let root = serde_yaml::from_reader(reader)?;

    Ok(root)
  }

  pub fn from_hashmap(tasks: HashMap<String, Task>) -> Self {
    Self {
      tasks,
      use_npm: None,
      use_cargo: None,
      include: None,
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::schema::{
    CommandRunner,
    TaskDependency,
  };

  #[test]
  fn test_task_root_1() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        task1:
          commands:
            - command: echo \"Hello, World 1!\"
              ignore_errors: false
              verbose: false
          depends_on:
            - name: task2
          description: 'This is a task'
          labels: {}
          environment:
            FOO: bar
          env_file:
            - test.env
        task2:
          commands:
            - command: echo \"Hello, World 2!\"
              ignore_errors: false
              verbose: false
          depends_on:
            - name: task1
          description: 'This is a task'
          labels: {}
          environment: {}
        task3:
          commands:
            - command: echo \"Hello, World 3!\"
              ignore_errors: false
              verbose: false
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;

    assert_eq!(task_root.tasks.len(), 3);

    if let Task::Task(task) = &task_root.tasks["task1"] {
      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo \"Hello, World 1!\"");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.shell, "sh");
        assert_eq!(local_run.ignore_errors, Some(false));
        assert_eq!(local_run.verbose, Some(false));
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      if let TaskDependency::TaskDependency(args) = &task.depends_on[0] {
        assert_eq!(args.name, "task2");
      } else {
        panic!("Expected TaskDependency::TaskDependency");
      }
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "This is a task");
      assert_eq!(task.environment.len(), 1);
      assert_eq!(task.env_file.len(), 1);
    } else {
      panic!("Expected Task::Task");
    }

    if let Task::Task(task) = &task_root.tasks["task2"] {
      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo \"Hello, World 2!\"");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.shell, "sh");
        assert_eq!(local_run.ignore_errors, Some(false));
        assert_eq!(local_run.verbose, Some(false));
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      if let TaskDependency::TaskDependency(args) = &task.depends_on[0] {
        assert_eq!(args.name, "task1");
      } else {
        panic!("Expected TaskDependency::TaskDependency");
      }
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "This is a task");
      assert_eq!(task.environment.len(), 0);
      assert_eq!(task.env_file.len(), 0);
    } else {
      panic!("Expected Task::Task");
    }

    if let Task::Task(task) = &task_root.tasks["task3"] {
      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo \"Hello, World 3!\"");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.shell, "sh");
        assert_eq!(local_run.ignore_errors, Some(false));
        assert_eq!(local_run.verbose, Some(false));
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description.len(), 0);
      assert_eq!(task.environment.len(), 0);
      assert_eq!(task.env_file.len(), 0);
    } else {
      panic!("Expected Task::Task");
    }

    Ok(())
  }

  #[test]
  fn test_task_root_2() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        task1:
          commands:
            - command: echo \"Hello, World 1!\"
        task2:
          commands:
            - echo \"Hello, World 2!\"
        task3: echo \"Hello, World 3!\"
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;

    assert_eq!(task_root.tasks.len(), 3);

    if let Task::Task(task) = &task_root.tasks["task1"] {
      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo \"Hello, World 1!\"");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.shell, "sh");
        assert_eq!(local_run.ignore_errors, None);
        assert_eq!(local_run.verbose, None);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "");
      assert_eq!(task.environment.len(), 0);
      assert_eq!(task.env_file.len(), 0);
    } else {
      panic!("Expected Task::Task");
    }

    if let Task::Task(task) = &task_root.tasks["task2"] {
      if let CommandRunner::CommandRun(command) = &task.commands[0] {
        assert_eq!(command, "echo \"Hello, World 2!\"");
      } else {
        panic!("Expected CommandRunner::CommandRun");
      }

      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "");
      assert_eq!(task.environment.len(), 0);
      assert_eq!(task.env_file.len(), 0);
    } else {
      panic!("Expected Task::Task");
    }

    if let Task::String(command) = &task_root.tasks["task3"] {
      assert_eq!(command, "echo \"Hello, World 3!\"");
    } else {
      panic!("Expected Task::String");
    }

    Ok(())
  }

  #[test]
  fn test_task_root_3() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        task1: echo \"Hello, World 1!\"
        task2: echo \"Hello, World 2!\"
        task3: echo \"Hello, World 3!\"
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;

    assert_eq!(task_root.tasks.len(), 3);

    if let Task::String(command) = &task_root.tasks["task1"] {
      assert_eq!(command, "echo \"Hello, World 1!\"");
    } else {
      panic!("Expected Task::String");
    }

    if let Task::String(command) = &task_root.tasks["task2"] {
      assert_eq!(command, "echo \"Hello, World 2!\"");
    } else {
      panic!("Expected Task::String");
    }

    if let Task::String(command) = &task_root.tasks["task3"] {
      assert_eq!(command, "echo \"Hello, World 3!\"");
    } else {
      panic!("Expected Task::String");
    }

    Ok(())
  }
}
