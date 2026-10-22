use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use serde::Serialize;
use std::collections::HashMap;

use crate::cache::CacheStore;
use crate::defaults::{default_ignore_errors, default_shell, default_verbose};
use crate::secrets::SecretConfig;

use super::{ActiveTasks, CompletedTasks, ContainerRuntime, MatrixSelector, Shell, TaskRoot};

const STATUS_SPINNER_TEMPLATE: &str = "{spinner:.green} [{prefix:.bold.dim}] {wide_msg:.cyan/blue} ";
const STATUS_SPINNER_TICK_CHARS: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏⦿";
const STATUS_SPINNER_TICK_INTERVAL: Duration = Duration::from_millis(80);

/// Used to pass information to tasks
/// This use arc to allow for sharing of data between tasks
/// and allow parallel runs of tasks
#[derive(Clone)]
pub struct TaskContext {
  pub task_root: Arc<TaskRoot>,
  pub active_tasks: ActiveTasks,
  pub completed_tasks: CompletedTasks,
  pub multi: Arc<MultiProgress>,
  pub status_pb: Arc<Mutex<Option<ProgressBar>>>,
  pub env_vars: HashMap<String, String>,
  pub matrix_vars: HashMap<String, String>,
  pub matrix_selectors: Vec<MatrixSelector>,
  pub matrix_selector_task_name: Option<String>,
  pub task_outputs: Arc<Mutex<HashMap<String, String>>>,
  pub secret_config: Option<SecretConfig>,
  pub shell: Option<Arc<Shell>>,
  pub container_runtime: Option<ContainerRuntime>,
  pub ignore_errors: Option<bool>,
  pub verbose: Option<bool>,
  pub force: bool,
  pub json_events: bool,
  pub is_nested: bool,
  pub forwarded_args: Vec<String>,
  pub cache_store: Arc<Mutex<CacheStore>>,
  pub current_task_name: Option<String>,
  pub cancellation_requested: Arc<AtomicBool>,
}

impl TaskContext {
  pub const NESTED_PROCESS_ENV: &'static str = "MK_NESTED_PROCESS";

  fn new_multi_progress(json_events: bool) -> Arc<MultiProgress> {
    if json_events || std::env::var_os(Self::NESTED_PROCESS_ENV).is_some() {
      Arc::new(MultiProgress::with_draw_target(ProgressDrawTarget::hidden()))
    } else {
      Arc::new(MultiProgress::new())
    }
  }

  pub fn empty() -> Self {
    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::hidden());
    Self {
      task_root: Arc::new(TaskRoot::default()),
      active_tasks: Arc::new(Mutex::new(HashSet::new())),
      completed_tasks: Arc::new(Mutex::new(HashSet::new())),
      multi: Arc::new(mp),
      status_pb: Arc::new(Mutex::new(None)),
      env_vars: HashMap::new(),
      matrix_vars: HashMap::new(),
      matrix_selectors: Vec::new(),
      matrix_selector_task_name: None,
      task_outputs: Arc::new(Mutex::new(HashMap::new())),
      secret_config: None,
      shell: None,
      container_runtime: None,
      ignore_errors: None,
      verbose: None,
      force: false,
      json_events: false,
      is_nested: false,
      forwarded_args: Vec::new(),
      cache_store: Arc::new(Mutex::new(CacheStore::default())),
      current_task_name: None,
      cancellation_requested: Arc::new(AtomicBool::new(false)),
    }
  }

  pub fn empty_with_root(task_root: Arc<TaskRoot>) -> Self {
    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::hidden());
    Self {
      task_root: task_root.clone(),
      active_tasks: Arc::new(Mutex::new(HashSet::new())),
      completed_tasks: Arc::new(Mutex::new(HashSet::new())),
      multi: Arc::new(mp),
      status_pb: Arc::new(Mutex::new(None)),
      env_vars: HashMap::new(),
      matrix_vars: HashMap::new(),
      matrix_selectors: Vec::new(),
      matrix_selector_task_name: None,
      task_outputs: Arc::new(Mutex::new(HashMap::new())),
      secret_config: None,
      shell: None,
      container_runtime: None,
      ignore_errors: None,
      verbose: None,
      force: false,
      json_events: false,
      is_nested: false,
      forwarded_args: Vec::new(),
      cache_store: Arc::new(Mutex::new(CacheStore::default())),
      current_task_name: None,
      cancellation_requested: Arc::new(AtomicBool::new(false)),
    }
  }

  pub fn new(task_root: Arc<TaskRoot>) -> Self {
    let cache_store = CacheStore::load_in_dir(&task_root.cache_base_dir()).unwrap_or_default();
    Self {
      task_root: task_root.clone(),
      active_tasks: Arc::new(Mutex::new(HashSet::new())),
      completed_tasks: Arc::new(Mutex::new(HashSet::new())),
      multi: Self::new_multi_progress(false),
      status_pb: Arc::new(Mutex::new(None)),
      env_vars: HashMap::new(),
      matrix_vars: HashMap::new(),
      matrix_selectors: Vec::new(),
      matrix_selector_task_name: None,
      task_outputs: Arc::new(Mutex::new(HashMap::new())),
      secret_config: None,
      shell: None,
      container_runtime: task_root.container_runtime.clone(),
      ignore_errors: None,
      verbose: None,
      force: false,
      json_events: false,
      is_nested: false,
      forwarded_args: Vec::new(),
      cache_store: Arc::new(Mutex::new(cache_store)),
      current_task_name: None,
      cancellation_requested: Arc::new(AtomicBool::new(false)),
    }
  }

  pub fn new_with_options(
    task_root: Arc<TaskRoot>,
    force: bool,
    json_events: bool,
    forwarded_args: Vec<String>,
  ) -> Self {
    let cache_store = CacheStore::load_in_dir(&task_root.cache_base_dir()).unwrap_or_default();
    let multi = Self::new_multi_progress(json_events);
    let mut env_vars: HashMap<String, String> = HashMap::new();
    env_vars.insert("MK_RUN_ARGC".to_string(), forwarded_args.len().to_string());
    for (i, arg) in forwarded_args.iter().enumerate() {
      env_vars.insert(format!("MK_RUN_ARG_{i}"), arg.clone());
    }
    Self {
      task_root: task_root.clone(),
      active_tasks: Arc::new(Mutex::new(HashSet::new())),
      completed_tasks: Arc::new(Mutex::new(HashSet::new())),
      multi,
      status_pb: Arc::new(Mutex::new(None)),
      env_vars,
      matrix_vars: HashMap::new(),
      matrix_selectors: Vec::new(),
      matrix_selector_task_name: None,
      task_outputs: Arc::new(Mutex::new(HashMap::new())),
      secret_config: None,
      shell: None,
      container_runtime: task_root.container_runtime.clone(),
      ignore_errors: None,
      verbose: None,
      force,
      json_events,
      is_nested: false,
      forwarded_args,
      cache_store: Arc::new(Mutex::new(cache_store)),
      current_task_name: None,
      cancellation_requested: Arc::new(AtomicBool::new(false)),
    }
  }

  pub fn from_context(context: &TaskContext) -> Self {
    Self {
      task_root: context.task_root.clone(),
      active_tasks: context.active_tasks.clone(),
      completed_tasks: context.completed_tasks.clone(),
      multi: context.multi.clone(),
      status_pb: context.status_pb.clone(),
      env_vars: context.env_vars.clone(),
      matrix_vars: context.matrix_vars.clone(),
      matrix_selectors: context.matrix_selectors.clone(),
      matrix_selector_task_name: context.matrix_selector_task_name.clone(),
      task_outputs: Arc::new(Mutex::new(HashMap::new())),
      secret_config: context.secret_config.clone(),
      shell: context.shell.clone(),
      container_runtime: context.container_runtime.clone(),
      ignore_errors: context.ignore_errors,
      verbose: context.verbose,
      force: context.force,
      json_events: context.json_events,
      is_nested: context.current_task_name.is_some(),
      forwarded_args: context.forwarded_args.clone(),
      cache_store: context.cache_store.clone(),
      current_task_name: context.current_task_name.clone(),
      cancellation_requested: context.cancellation_requested.clone(),
    }
  }

  pub fn from_context_with_args(context: &TaskContext, ignore_errors: bool, verbose: bool) -> Self {
    Self {
      task_root: context.task_root.clone(),
      active_tasks: context.active_tasks.clone(),
      completed_tasks: context.completed_tasks.clone(),
      multi: context.multi.clone(),
      status_pb: context.status_pb.clone(),
      env_vars: context.env_vars.clone(),
      matrix_vars: context.matrix_vars.clone(),
      matrix_selectors: context.matrix_selectors.clone(),
      matrix_selector_task_name: context.matrix_selector_task_name.clone(),
      task_outputs: Arc::new(Mutex::new(HashMap::new())),
      secret_config: context.secret_config.clone(),
      shell: context.shell.clone(),
      container_runtime: context.container_runtime.clone(),
      ignore_errors: Some(ignore_errors),
      verbose: Some(verbose),
      force: context.force,
      json_events: context.json_events,
      is_nested: true,
      forwarded_args: context.forwarded_args.clone(),
      cache_store: context.cache_store.clone(),
      current_task_name: context.current_task_name.clone(),
      cancellation_requested: context.cancellation_requested.clone(),
    }
  }

  pub fn format_status_message(&self, detail: &str) -> String {
    match self.current_task_name.as_deref() {
      Some(name) if !name.is_empty() && name != "<task>" => format!("{}: {}", name, detail),
      _ => detail.to_string(),
    }
  }

  pub fn update_status(&self, prefix: impl Into<String>, detail: impl AsRef<str>) {
    let Ok(mut guard) = self.status_pb.lock() else {
      return;
    };
    let prefix = prefix.into();
    let message = self.format_status_message(detail.as_ref());

    let pb = guard.get_or_insert_with(|| {
      let pb = self.multi.add(ProgressBar::new_spinner());
      if let Ok(style) = ProgressStyle::with_template(STATUS_SPINNER_TEMPLATE) {
        pb.set_style(style.tick_chars(STATUS_SPINNER_TICK_CHARS));
      }
      pb.set_prefix(prefix.clone());
      pb.set_message(message.clone());
      pb.enable_steady_tick(STATUS_SPINNER_TICK_INTERVAL);
      pb
    });

    pb.set_prefix(prefix);
    pb.set_message(message);
  }

  pub fn finish_status_with_message(&self, detail: impl AsRef<str>) {
    let Ok(mut guard) = self.status_pb.lock() else {
      return;
    };
    if let Some(pb) = guard.take() {
      pb.finish_with_message(self.format_status_message(detail.as_ref()));
    }
  }

  pub fn clear_status(&self) {
    let Ok(mut guard) = self.status_pb.lock() else {
      return;
    };
    if let Some(pb) = guard.take() {
      pb.finish_and_clear();
      self.multi.remove(&pb);
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

  pub fn set_secret_config(&mut self, secret_config: SecretConfig) {
    self.secret_config = Some(secret_config);
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

  pub fn insert_task_output(&self, name: impl Into<String>, value: impl Into<String>) -> anyhow::Result<()> {
    let name = name.into();
    let mut outputs = self
      .task_outputs
      .lock()
      .map_err(|e| anyhow::anyhow!("Failed to lock task outputs - {}", e))?;
    if outputs.contains_key(&name) {
      anyhow::bail!("Task output already exists - {}", name);
    }
    outputs.insert(name, value.into());
    Ok(())
  }

  pub fn get_task_output(&self, name: &str) -> anyhow::Result<Option<String>> {
    let outputs = self
      .task_outputs
      .lock()
      .map_err(|e| anyhow::anyhow!("Failed to lock task outputs - {}", e))?;
    Ok(outputs.get(name).cloned())
  }

  pub fn has_task_output(&self, name: &str) -> anyhow::Result<bool> {
    let outputs = self
      .task_outputs
      .lock()
      .map_err(|e| anyhow::anyhow!("Failed to lock task outputs - {}", e))?;
    Ok(outputs.contains_key(name))
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

  pub fn request_cancellation(&self) {
    self.cancellation_requested.store(true, Ordering::SeqCst);
  }

  pub fn cancellation_requested(&self) -> bool {
    self.cancellation_requested.load(Ordering::SeqCst)
  }

  pub fn clear_cancellation(&self) {
    self.cancellation_requested.store(false, Ordering::SeqCst);
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
      let expected = if cfg!(windows) { "cmd" } else { "sh" };
      assert_eq!(context.shell().cmd(), expected.to_string());
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

  #[test]
  fn test_task_context_outputs_are_stored() -> anyhow::Result<()> {
    let context = TaskContext::empty();
    context.insert_task_output("tag", "v1.0.0")?;
    assert_eq!(context.get_task_output("tag")?, Some("v1.0.0".to_string()));
    assert!(context.has_task_output("tag")?);
    Ok(())
  }

  #[test]
  fn test_task_context_single_status_spinner_updates_in_place() {
    let mut root_context = TaskContext::empty();
    root_context.set_current_task_name("dev:oracle");
    root_context.update_status("1/2", "Running task dependencies...");

    let child_context = TaskContext::from_context(&root_context);
    assert!(child_context.is_nested);
    child_context.update_status("0/3", "Running task commands in parallel...");

    let guard = root_context.status_pb.lock().expect("status lock");
    let pb = guard.as_ref().expect("shared spinner should exist");
    assert_eq!(pb.prefix(), "0/3");
    assert_eq!(pb.message(), "dev:oracle: Running task commands in parallel...");
    drop(guard);

    root_context.finish_status_with_message("Commands completed.");
    assert!(root_context.status_pb.lock().expect("status lock").is_none());
  }
}
