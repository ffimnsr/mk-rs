use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use super::{
  Command,
  TaskDependency,
};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Task {
  pub commands: Vec<Command>,
  pub depends_on: Vec<TaskDependency>,
  pub labels: HashMap<String, String>,
  pub description: String,
  pub aliases: Vec<String>,
  pub environment: HashMap<String, String>,
  pub env_file: HashMap<String, String>,
  pub interactive: bool,
  pub ignore_errors: bool,
}

mod test {
  use super::*;

  #[test]
  fn test_task() {
    {
      let yaml = "
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

      let task = serde_yaml::from_str::<Task>(yaml).unwrap();

      assert_eq!(task.commands[0].command, "echo \"Hello, World!\"");
      assert_eq!(task.depends_on[0].name, "task1");
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "This is a task");
      assert_eq!(task.aliases[0], "t");
      assert_eq!(task.environment.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.interactive, false);
      assert_eq!(task.ignore_errors, false);
    }
  }
}
