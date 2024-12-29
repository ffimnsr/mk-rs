use anyhow::Context;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use super::Task;

/// This struct represents the root of the task schema. It contains all the tasks
/// that can be executed.
#[derive(Debug, Default, Deserialize)]
pub struct TaskRoot {
  pub tasks: HashMap<String, Task>,
}

impl TaskRoot {
  pub fn from_file(file: &str) -> anyhow::Result<Self> {
    let file = File::open(file).with_context(|| format!("Failed to open file: {}", file))?;
    let reader = BufReader::new(file);
    let task_root = serde_yaml::from_reader(reader)?;

    Ok(task_root)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::schema::CommandRunner;

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

    if let CommandRunner::LocalRun(local_run) = &task_root.tasks["task1"].commands[0] {
      assert_eq!(local_run.command, "echo \"Hello, World 1!\"");
      assert_eq!(local_run.work_dir, None);
      assert_eq!(local_run.shell, "sh");
      assert!(!local_run.ignore_errors);
      assert!(!local_run.verbose);
    } else {
      panic!("Expected CommandRunner::LocalRun");
    }

    assert_eq!(task_root.tasks["task1"].depends_on[0].name, "task2");
    assert_eq!(task_root.tasks["task1"].labels.len(), 0);
    assert_eq!(task_root.tasks["task1"].description, "This is a task");
    assert_eq!(task_root.tasks["task1"].environment.len(), 1);
    assert_eq!(task_root.tasks["task1"].env_file.len(), 1);

    if let CommandRunner::LocalRun(local_run) = &task_root.tasks["task2"].commands[0] {
      assert_eq!(local_run.command, "echo \"Hello, World 2!\"");
      assert_eq!(local_run.work_dir, None);
      assert_eq!(local_run.shell, "sh");
      assert!(!local_run.ignore_errors);
      assert!(!local_run.verbose);
    } else {
      panic!("Expected CommandRunner::LocalRun");
    }

    assert_eq!(task_root.tasks["task2"].depends_on[0].name, "task1");
    assert_eq!(task_root.tasks["task2"].labels.len(), 0);
    assert_eq!(task_root.tasks["task2"].description, "This is a task");
    assert_eq!(task_root.tasks["task2"].environment.len(), 0);
    assert_eq!(task_root.tasks["task2"].env_file.len(), 0);

    if let CommandRunner::LocalRun(local_run) = &task_root.tasks["task3"].commands[0] {
      assert_eq!(local_run.command, "echo \"Hello, World 3!\"");
      assert_eq!(local_run.work_dir, None);
      assert_eq!(local_run.shell, "sh");
      assert!(!local_run.ignore_errors);
      assert!(!local_run.verbose);
    } else {
      panic!("Expected CommandRunner::LocalRun");
    }

    assert_eq!(task_root.tasks["task3"].depends_on.len(), 0);
    assert_eq!(task_root.tasks["task3"].labels.len(), 0);
    assert_eq!(task_root.tasks["task3"].description.len(), 0);
    assert_eq!(task_root.tasks["task3"].environment.len(), 0);
    assert_eq!(task_root.tasks["task3"].env_file.len(), 0);

    Ok(())
  }
}
