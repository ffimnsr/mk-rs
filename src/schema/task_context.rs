use std::collections::HashMap;
use std::sync::Arc;

use indicatif::{
  MultiProgress,
  ProgressDrawTarget,
};

use crate::defaults::{
  default_ignore_errors,
  default_shell,
  default_verbose,
};

use super::{
  ExecutionStack,
  TaskRoot,
};

pub struct TaskContext {
  pub task_root: Arc<TaskRoot>,
  pub execution_stack: ExecutionStack,
  pub multi: Arc<MultiProgress>,
  pub env_vars: HashMap<String, String>,
  pub shell: Option<String>,
  pub ignore_errors: Option<bool>,
  pub verbose: Option<bool>,
  pub is_nested: bool,
}

impl TaskContext {
  pub fn empty() -> Self {
    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::hidden());
    Self {
      task_root: Arc::new(TaskRoot::default()),
      execution_stack: ExecutionStack::default(),
      multi: Arc::new(mp),
      env_vars: HashMap::new(),
      shell: None,
      ignore_errors: None,
      verbose: None,
      is_nested: false,
    }
  }

  pub fn empty_with_root(task_root: Arc<TaskRoot>) -> Self {
    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::hidden());
    Self {
      task_root: task_root.clone(),
      execution_stack: ExecutionStack::default(),
      multi: Arc::new(mp),
      env_vars: HashMap::new(),
      shell: None,
      ignore_errors: None,
      verbose: None,
      is_nested: false,
    }
  }

  pub fn new(task_root: Arc<TaskRoot>, execution_stack: ExecutionStack) -> Self {
    Self {
      task_root: task_root.clone(),
      execution_stack,
      multi: Arc::new(MultiProgress::new()),
      env_vars: HashMap::new(),
      shell: None,
      ignore_errors: None,
      verbose: None,
      is_nested: false,
    }
  }

  pub fn from_context(context: &TaskContext) -> Self {
    Self {
      task_root: context.task_root.clone(),
      execution_stack: context.execution_stack.clone(),
      multi: context.multi.clone(),
      env_vars: context.env_vars.clone(),
      shell: context.shell.clone(),
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
      shell: context.shell.clone(),
      ignore_errors: Some(ignore_errors),
      verbose: Some(verbose),
      is_nested: true,
    }
  }

  pub fn extend_env_vars<I>(&mut self, iter: I)
  where
    I: IntoIterator<Item = (String, String)>,
  {
    self.env_vars.extend(iter);
  }

  pub fn set_shell(&mut self, shell: &str) {
    self.shell = Some(shell.to_string());
  }

  pub fn set_ignore_errors(&mut self, ignore_errors: bool) {
    self.ignore_errors = Some(ignore_errors);
  }

  pub fn set_verbose(&mut self, verbose: bool) {
    self.verbose = Some(verbose);
  }

  pub fn shell(&self) -> String {
    self.shell.clone().unwrap_or(default_shell())
  }

  pub fn ignore_errors(&self) -> bool {
    self.ignore_errors.unwrap_or(default_ignore_errors())
  }

  pub fn verbose(&self) -> bool {
    self.verbose.unwrap_or(default_verbose())
  }
}
