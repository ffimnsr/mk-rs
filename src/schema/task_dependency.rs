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
    run_task_by_name(context, self.resolve_name())
  }

  pub fn resolve_name(&self) -> &str {
    match self {
      TaskDependency::String(name) => name,
      TaskDependency::TaskDependency(args) => &args.name,
    }
  }
}

impl TaskDependencyArgs {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    run_task_by_name(context, &self.name)
  }
}

pub fn run_task_by_name(context: &TaskContext, task_name: &str) -> anyhow::Result<()> {
  assert!(!task_name.is_empty());

  if context.is_task_completed(task_name)? {
    log::trace!("Skipping completed task: {}", task_name);
    return Ok(());
  }

  if context.is_task_active(task_name)? {
    anyhow::bail!("Circular dependency detected - {}", task_name);
  }

  let task = context.task_root.tasks.get(task_name).ok_or_else(|| {
    anyhow::anyhow!(
      "Task '{}' not found. Run 'mk list' to see available tasks.",
      task_name
    )
  })?;

  log::trace!("Task: {:?}", task);

  context.mark_task_active(task_name)?;

  let result = {
    let mut child_context = TaskContext::from_context(context);
    child_context.set_current_task_name(task_name);
    child_context.emit_event(&serde_json::json!({
      "event": "task_started",
      "task": task_name,
    }))?;
    task.run(&mut child_context)
  };

  context.unmark_task_active(task_name)?;

  if result.is_ok() {
    context.mark_task_complete(task_name)?;
    context.emit_event(&serde_json::json!({
      "event": "task_finished",
      "task": task_name,
      "success": true,
    }))?;
  } else {
    context.emit_event(&serde_json::json!({
      "event": "task_finished",
      "task": task_name,
      "success": false,
    }))?;
  }

  result
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

  #[test]
  fn test_task_dependency_6_shared_dependency_is_not_a_cycle() -> anyhow::Result<()> {
    let root_yaml = "
      tasks:
        root:
          commands:
            - command: echo root
              verbose: false
          depends_on:
            - left
            - right
        left:
          commands:
            - command: echo left
              verbose: false
          depends_on:
            - shared
        right:
          commands:
            - command: echo right
              verbose: false
          depends_on:
            - shared
        shared:
          commands:
            - command: echo shared
              verbose: false
    ";

    let root = Arc::new(serde_yaml::from_str::<TaskRoot>(root_yaml)?);
    let context = TaskContext::empty_with_root(root);
    let result = run_task_by_name(&context, "root");
    assert!(result.is_ok());
    assert!(context.is_task_completed("shared")?);

    Ok(())
  }
}
