use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{
  Arc,
  Mutex,
};

use hashbrown::HashMap;
use indicatif::{
  MultiProgress,
  ProgressDrawTarget,
};
use serde::Serialize;

use crate::cache::CacheStore;
use crate::defaults::{
  default_ignore_errors,
  default_shell,
  default_verbose,
};

use super::{
  ActiveTasks,
  CompletedTasks,
  ContainerRuntime,
  Shell,
  TaskRoot,
};

/// Used to pass information to tasks
/// This use arc to allow for sharing of data between tasks
/// and allow parallel runs of tasks
#[derive(Clone)]
pub struct TaskContext {
  pub task_root: Arc<TaskRoot>,
  pub active_tasks: ActiveTasks,
  pub completed_tasks: CompletedTasks,
  pub multi: Arc<MultiProgress>,
  pub env_vars: HashMap<String, String>,
  pub shell: Option<Arc<Shell>>,
  pub container_runtime: Option<ContainerRuntime>,
  pub ignore_errors: Option<bool>,
  pub verbose: Option<bool>,
  pub force: bool,
  pub json_events: bool,
  pub is_nested: bool,
  pub cache_store: Arc<Mutex<CacheStore>>,
  pub current_task_name: Option<String>,
}

impl TaskContext {
  pub fn empty() -> Self {
    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::hidden());
    Self {
      task_root: Arc::new(TaskRoot::default()),
      active_tasks: Arc::new(Mutex::new(HashSet::new())),
      completed_tasks: Arc::new(Mutex::new(HashSet::new())),
      multi: Arc::new(mp),
      env_vars: HashMap::new(),
      shell: None,
      container_runtime: None,
      ignore_errors: None,
      verbose: None,
      force: false,
      json_events: false,
      is_nested: false,
      cache_store: Arc::new(Mutex::new(CacheStore::default())),
      current_task_name: None,
    }
  }

  pub fn empty_with_root(task_root: Arc<TaskRoot>) -> Self {
    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::hidden());
    Self {
      task_root: task_root.clone(),
      active_tasks: Arc::new(Mutex::new(HashSet::new())),
      completed_tasks: Arc::new(Mutex::new(HashSet::new())),
      multi: Arc::new(mp),
      env_vars: HashMap::new(),
      shell: None,
      container_runtime: None,
      ignore_errors: None,
      verbose: None,
      force: false,
      json_events: false,
      is_nested: false,
      cache_store: Arc::new(Mutex::new(CacheStore::default())),
      current_task_name: None,
    }
  }

  pub fn new(task_root: Arc<TaskRoot>) -> Self {
    let cache_store = CacheStore::load_in_dir(&task_root.cache_base_dir()).unwrap_or_default();
    Self {
      task_root: task_root.clone(),
      active_tasks: Arc::new(Mutex::new(HashSet::new())),
      completed_tasks: Arc::new(Mutex::new(HashSet::new())),
      multi: Arc::new(MultiProgress::new()),
      env_vars: HashMap::new(),
      shell: None,
      container_runtime: task_root.container_runtime.clone(),
      ignore_errors: None,
      verbose: None,
      force: false,
      json_events: false,
      is_nested: false,
      cache_store: Arc::new(Mutex::new(cache_store)),
      current_task_name: None,
    }
  }

  pub fn new_with_options(task_root: Arc<TaskRoot>, force: bool, json_events: bool) -> Self {
    let cache_store = CacheStore::load_in_dir(&task_root.cache_base_dir()).unwrap_or_default();
    let multi = if json_events {
      Arc::new(MultiProgress::with_draw_target(ProgressDrawTarget::hidden()))
    } else {
      Arc::new(MultiProgress::new())
    };
    Self {
      task_root: task_root.clone(),
      active_tasks: Arc::new(Mutex::new(HashSet::new())),
      completed_tasks: Arc::new(Mutex::new(HashSet::new())),
      multi,
      env_vars: HashMap::new(),
      shell: None,
      container_runtime: task_root.container_runtime.clone(),
      ignore_errors: None,
      verbose: None,
      force,
      json_events,
      is_nested: false,
      cache_store: Arc::new(Mutex::new(cache_store)),
      current_task_name: None,
    }
  }

  pub fn from_context(context: &TaskContext) -> Self {
    Self {
      task_root: context.task_root.clone(),
      active_tasks: context.active_tasks.clone(),
      completed_tasks: context.completed_tasks.clone(),
      multi: context.multi.clone(),
      env_vars: context.env_vars.clone(),
      shell: context.shell.clone(),
      container_runtime: context.container_runtime.clone(),
      ignore_errors: context.ignore_errors,
      verbose: context.verbose,
      force: context.force,
      json_events: context.json_events,
      is_nested: true,
      cache_store: context.cache_store.clone(),
      current_task_name: context.current_task_name.clone(),
    }
  }

  pub fn from_context_with_args(context: &TaskContext, ignore_errors: bool, verbose: bool) -> Self {
    Self {
      task_root: context.task_root.clone(),
      active_tasks: context.active_tasks.clone(),
      completed_tasks: context.completed_tasks.clone(),
      multi: context.multi.clone(),
      env_vars: context.env_vars.clone(),
      shell: context.shell.clone(),
      container_runtime: context.container_runtime.clone(),
      ignore_errors: Some(ignore_errors),
      verbose: Some(verbose),
      force: context.force,
      json_events: context.json_events,
      is_nested: true,
      cache_store: context.cache_store.clone(),
      current_task_name: context.current_task_name.clone(),
    }
  }

  pub fn extend_env_vars<I>(&mut self, iter: I)
  where
    I: IntoIterator<Item = (String, String)>,
  {
    self.env_vars.extend(iter);
  }

  pub fn set_shell(&mut self, shell: &Shell) {
    let shell = Arc::new(Shell::from_shell(shell));
    self.shell = Some(shell);
  }

  pub fn set_container_runtime(&mut self, runtime: &ContainerRuntime) {
    self.container_runtime = Some(runtime.clone());
  }

  pub fn set_ignore_errors(&mut self, ignore_errors: bool) {
    self.ignore_errors = Some(ignore_errors);
  }

  pub fn set_verbose(&mut self, verbose: bool) {
    self.verbose = Some(verbose);
  }

  pub fn shell(&self) -> Arc<Shell> {
    self.shell.clone().unwrap_or_else(|| Arc::new(default_shell()))
  }

  pub fn ignore_errors(&self) -> bool {
    self.ignore_errors.unwrap_or(default_ignore_errors())
  }

  pub fn verbose(&self) -> bool {
    self.verbose.unwrap_or(default_verbose())
  }

  pub fn is_task_active(&self, task_name: &str) -> anyhow::Result<bool> {
    let active = self
      .active_tasks
      .lock()
      .map_err(|e| anyhow::anyhow!("Failed to lock active tasks - {}", e))?;
    Ok(active.contains(task_name))
  }

  pub fn is_task_completed(&self, task_name: &str) -> anyhow::Result<bool> {
    let completed = self
      .completed_tasks
      .lock()
      .map_err(|e| anyhow::anyhow!("Failed to lock completed tasks - {}", e))?;
    Ok(completed.contains(task_name))
  }

  pub fn mark_task_active(&self, task_name: &str) -> anyhow::Result<()> {
    let mut active = self
      .active_tasks
      .lock()
      .map_err(|e| anyhow::anyhow!("Failed to lock active tasks - {}", e))?;
    active.insert(task_name.to_string());
    Ok(())
  }

  pub fn unmark_task_active(&self, task_name: &str) -> anyhow::Result<()> {
    let mut active = self
      .active_tasks
      .lock()
      .map_err(|e| anyhow::anyhow!("Failed to lock active tasks - {}", e))?;
    active.remove(task_name);
    Ok(())
  }

  pub fn mark_task_complete(&self, task_name: &str) -> anyhow::Result<()> {
    let mut completed = self
      .completed_tasks
      .lock()
      .map_err(|e| anyhow::anyhow!("Failed to lock completed tasks - {}", e))?;
    completed.insert(task_name.to_string());
    Ok(())
  }

  pub fn emit_event<T: Serialize>(&self, value: &T) -> anyhow::Result<()> {
    if self.json_events {
      println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
  }

  pub fn set_current_task_name(&mut self, task_name: &str) {
    self.current_task_name = Some(task_name.to_string());
  }

  pub fn resolve_from_config(&self, value: &str) -> PathBuf {
    self.task_root.resolve_from_config(value)
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_task_context_1() -> anyhow::Result<()> {
    {
      let context = TaskContext::empty();
      assert_eq!(context.shell().cmd(), "sh".to_string());
      assert!(!context.ignore_errors());
      assert!(context.verbose());
    }

    Ok(())
  }

  #[test]
  fn test_task_context_2() -> anyhow::Result<()> {
    {
      let mut context = TaskContext::empty();
      context.set_shell(&Shell::String("bash".to_string()));
      assert_eq!(context.shell().cmd(), "bash".to_string());
    }

    Ok(())
  }

  #[test]
  fn test_task_context_3() -> anyhow::Result<()> {
    {
      let mut context = TaskContext::empty();
      context.extend_env_vars(vec![("key".to_string(), "value".to_string())]);
      assert_eq!(context.env_vars.get("key"), Some(&"value".to_string()));
    }

    Ok(())
  }

  #[test]
  fn test_task_context_4() -> anyhow::Result<()> {
    {
      let mut context = TaskContext::empty();
      context.set_ignore_errors(true);
      assert!(context.ignore_errors());
    }

    Ok(())
  }

  #[test]
  fn test_task_context_5() -> anyhow::Result<()> {
    {
      let mut context = TaskContext::empty();
      context.set_verbose(true);
      assert!(context.verbose());
    }

    Ok(())
  }
}
