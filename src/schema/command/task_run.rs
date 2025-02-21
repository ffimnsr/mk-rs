use serde::Deserialize;

use crate::defaults::{
  default_ignore_errors,
  default_verbose,
};
use crate::schema::TaskContext;

#[derive(Debug, Deserialize, Clone)]
pub struct TaskRun {
  /// The name of the task to run
  pub task: String,

  /// Ignore errors if the task commands fail
  #[serde(default)]
  pub ignore_errors: Option<bool>,

  /// Show verbose output
  #[serde(default)]
  pub verbose: Option<bool>,
}

impl TaskRun {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.task.is_empty());

    let ignore_errors = self.ignore_errors(context);
    let verbose = self.verbose(context);

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

    let mut context = TaskContext::from_context_with_args(context, ignore_errors, verbose);
    task.run(&mut context)?;

    Ok(())
  }

  fn ignore_errors(&self, context: &TaskContext) -> bool {
    self
      .ignore_errors
      .or(context.ignore_errors)
      .unwrap_or(default_ignore_errors())
  }

  fn verbose(&self, context: &TaskContext) -> bool {
    self.verbose.or(context.verbose).unwrap_or(default_verbose())
  }
}
