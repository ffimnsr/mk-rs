use serde::Deserialize;

use crate::defaults::default_true;
use crate::schema::TaskContext;

#[derive(Debug, Deserialize)]
pub struct TaskRun {
  /// The name of the task to run
  pub task: String,

  /// Ignore errors if the task commands fail
  #[serde(default)]
  pub ignore_errors: bool,

  /// Show verbose output
  #[serde(default = "default_true")]
  pub verbose: bool,
}

impl TaskRun {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.task.is_empty());

    let task = context
      .task_root
      .tasks
      .get(&self.task)
      .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

    log::trace!("Task: {:?}", task);

    {
      let mut stack = context
        .execution_stack
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock execution stack - {}", e))?;

      if stack.contains(&self.task) {
        anyhow::bail!("Circular dependency detected - {}", &self.task);
      }

      stack.insert(self.task.clone());
    }

    let mut context = TaskContext::from_context_with_args(context, self.ignore_errors, self.verbose);
    task.run(&mut context)?;

    Ok(())
  }
}
