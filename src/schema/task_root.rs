use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use super::Task;


#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TaskRoot {
  pub tasks: HashMap<String, Task>,
}

impl TaskRoot {
  pub fn from_file(file: &str) -> anyhow::Result<Self> {
    let file = File::open(file)?;
    let reader = BufReader::new(file);
    let task_root = serde_yaml::from_reader(reader)?;

    Ok(task_root)
  }
}

mod test {
  use super::*;

  #[test]
  fn test_task_root() {
    let yaml = "
      tasks:
        task1:
          commands:
            - command: echo \"Hello, World!\"
              ignore_errors: false
              verbose: false
          depends_on:
            - name: task2
          description: 'This is a task'
          labels: {}
          environment: {}
          env_file: {}
          aliases:
            - t
          interactive: false
          ignore_errors: false
        task2:
          commands:
            - command: echo \"Hello, World!\"
              ignore_errors: false
              verbose: false
          depends_on:
            - name: task1
          description: 'This is a task'
          labels: {}
          environment: {}
          env_file: {}
          aliases:
            - t
          interactive: false
          ignore_errors: false
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml).unwrap();

    assert_eq!(task_root.tasks.len(), 2);
    assert_eq!(task_root.tasks["task1"].commands[0].command, "echo \"Hello, World!\"");
    assert_eq!(task_root.tasks["task1"].commands[0].ignore_errors, false);
    assert_eq!(task_root.tasks["task1"].commands[0].verbose, false);
    assert_eq!(task_root.tasks["task1"].depends_on[0].name, "task2");
    assert_eq!(task_root.tasks["task1"].labels.len(), 0);
    assert_eq!(task_root.tasks["task1"].description, "This is a task");
    assert_eq!(task_root.tasks["task1"].aliases[0], "t");
    assert_eq!(task_root.tasks["task1"].environment.len(), 0);
    assert_eq!(task_root.tasks["task1"].env_file.len(), 0);
    assert_eq!(task_root.tasks["task1"].interactive, false);
    assert_eq!(task_root.tasks["task1"].ignore_errors, false);

    assert_eq!(task_root.tasks["task2"].commands[0].command, "echo \"Hello, World!\"");
    assert_eq!(task_root.tasks["task2"].commands[0].ignore_errors, false);
    assert_eq!(task_root.tasks["task2"].commands[0].verbose, false);
  }
}
