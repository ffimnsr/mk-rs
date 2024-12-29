use serde::Deserialize;

use super::TaskContext;

/// This struct represents a task dependency. A task can depend on other tasks.
/// If a task depends on another task, the dependent task must be executed before
/// the dependent task.
#[derive(Debug, Default, Deserialize)]
pub struct TaskDependency {
  pub name: String,
}

impl TaskDependency {
  pub fn run(&self, context: &TaskContext) -> anyhow::Result<()> {
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
  use super::*;

  #[test]
  fn test_task_dependency_1() -> anyhow::Result<()> {
    let yaml = "
      name: task1
    ";
    let task_dependency = serde_yaml::from_str::<TaskDependency>(yaml)?;
    assert_eq!(task_dependency.name, "task1");

    Ok(())
  }
}
