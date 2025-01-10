use serde::Deserialize;

use super::TaskContext;

/// This struct represents a task dependency. A task can depend on other tasks.
/// If a task depends on another task, the dependent task must be executed before
/// the dependent task.
#[derive(Debug, Deserialize)]
pub struct TaskDependencyArgs {
  /// The name of the task to depend on
  pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TaskDependency {
  String(String),
  TaskDependency(Box<TaskDependencyArgs>),
}

impl TaskDependency {
  pub fn run(&self, context: &TaskContext) -> anyhow::Result<()> {
    match self {
      TaskDependency::String(name) => self.execute(context, name),
      TaskDependency::TaskDependency(args) => args.execute(context),
    }
  }

  fn execute(&self, context: &TaskContext, task_name: &str) -> anyhow::Result<()> {
    assert!(!task_name.is_empty());

    let task = context
      .task_root
      .tasks
      .get(task_name)
      .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

    log::trace!("Task: {:?}", task);

    {
      let mut stack = context
        .execution_stack
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock execution stack - {}", e))?;

      if stack.contains(task_name) {
        anyhow::bail!("Circular dependency detected - {}", task_name);
      }

      stack.insert(task_name.into());
    }

    let mut context = TaskContext::from_context(context);
    task.run(&mut context)?;

    Ok(())
  }
}

impl TaskDependencyArgs {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.name.is_empty());

    let task_name: &str = &self.name;
    let task = context
      .task_root
      .tasks
      .get(task_name)
      .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

    log::trace!("Task: {:?}", task);

    {
      let mut stack = context
        .execution_stack
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock execution stack - {}", e))?;

      if stack.contains(task_name) {
        anyhow::bail!("Circular dependency detected - {}", task_name);
      }

      stack.insert(task_name.into());
    }

    let mut context = TaskContext::from_context(context);
    task.run(&mut context)?;

    Ok(())
  }
}

#[cfg(test)]
mod test {
  use hashbrown::HashMap;
  use std::sync::Arc;

  use crate::schema::{
    Task,
    TaskRoot,
  };

  use super::*;

  #[test]
  fn test_task_dependency_1() -> anyhow::Result<()> {
    let yaml = "
      name: task1
    ";

    let task_dependency = serde_yaml::from_str::<TaskDependency>(yaml)?;
    if let TaskDependency::TaskDependency(args) = task_dependency {
      assert_eq!(args.name, "task1");
    } else {
      panic!("Expected TaskDependency::TaskDependency");
    }

    Ok(())
  }

  #[test]
  fn test_task_dependency_2() -> anyhow::Result<()> {
    let yaml = "\"task1\"";

    let task_dependency = serde_yaml::from_str::<TaskDependency>(yaml)?;
    if let TaskDependency::String(name) = task_dependency {
      assert_eq!(name, "task1");
    } else {
      panic!("Expected TaskDependency::TaskString");
    }

    Ok(())
  }

  #[test]
  fn test_task_dependency_3() -> anyhow::Result<()> {
    let yaml = "\"\"";

    let task_dependency = serde_yaml::from_str::<TaskDependency>(yaml)?;
    if let TaskDependency::String(name) = task_dependency {
      assert_eq!(name, "");
    } else {
      panic!("Expected TaskDependency::TaskString");
    }

    Ok(())
  }

  #[test]
  fn test_task_dependency_4() -> anyhow::Result<()> {
    let yaml = "
      name:
    ";

    let task_dependency = serde_yaml::from_str::<TaskDependencyArgs>(yaml)?;
    assert_eq!(task_dependency.name, "");

    Ok(())
  }

  #[test]
  fn test_task_dependency_5() -> anyhow::Result<()> {
    let yaml = "
      name: task_a
    ";

    let task_yaml = "
      commands:
        - command: echo 1
          verbose: false
    ";

    let task = serde_yaml::from_str::<Task>(task_yaml)?;
    let mut hm = HashMap::new();
    hm.insert("task_a".into(), task);

    let root = Arc::new(TaskRoot::from_hashmap(hm));
    let task_dependency = serde_yaml::from_str::<TaskDependencyArgs>(yaml)?;
    let result = task_dependency.execute(&TaskContext::empty_with_root(root));
    assert!(result.is_ok());

    Ok(())
  }
}
