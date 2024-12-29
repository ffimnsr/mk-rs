use std::collections::HashMap;
use std::sync::Arc;

use indicatif::MultiProgress;

use super::{
  ExecutionStack,
  TaskRoot,
};

pub struct TaskContext {
  pub task_root: Arc<TaskRoot>,
  pub execution_stack: ExecutionStack,
  pub multi: Arc<MultiProgress>,
  pub env_vars: HashMap<String, String>,
  pub ignore_errors: bool,
  pub verbose: bool,
  pub is_nested: bool,
}

impl TaskContext {
  pub fn new(task_root: Arc<TaskRoot>, execution_stack: ExecutionStack) -> Self {
    Self {
      task_root: task_root.clone(),
      execution_stack,
      multi: Arc::new(MultiProgress::new()),
      env_vars: HashMap::new(),
      ignore_errors: false,
      verbose: false,
      is_nested: false,
    }
  }

  pub fn from_context(context: &TaskContext) -> Self {
    Self {
      task_root: context.task_root.clone(),
      execution_stack: context.execution_stack.clone(),
      multi: context.multi.clone(),
      env_vars: context.env_vars.clone(),
      ignore_errors: context.ignore_errors,
      verbose: context.verbose,
      is_nested: true,
    }
  }

  pub fn from_context_with_args(context: &TaskContext, ignore_errors: bool, verbose: bool) -> Self {
    Self {
      task_root: context.task_root.clone(),
      execution_stack: context.execution_stack.clone(),
      multi: context.multi.clone(),
      env_vars: context.env_vars.clone(),
      ignore_errors,
      verbose,
      is_nested: true,
    }
  }

  pub fn set_ignore_errors(&mut self, ignore_errors: bool) {
    self.ignore_errors = ignore_errors;
  }

  pub fn set_verbose(&mut self, verbose: bool) {
    self.verbose = verbose;
  }
}
