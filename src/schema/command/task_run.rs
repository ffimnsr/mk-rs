use serde::Deserialize;

use crate::defaults::{
  default_ignore_errors,
  default_verbose,
};
use crate::schema::{
  run_task_by_name,
  TaskContext,
};

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

    let context = TaskContext::from_context_with_args(context, ignore_errors, verbose);
    run_task_by_name(&context, &self.task)
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
