use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TaskDependency {
  pub name: String,
}

mod test {
  use super::*;

  #[test]
  fn test_task_dependency() {
    let yaml = "
      name: task1
    ";
    let task_dependency = serde_yaml::from_str::<TaskDependency>(yaml).unwrap();

    assert_eq!(task_dependency.name, "task1");
  }
}
