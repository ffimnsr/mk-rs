use hashbrown::HashMap;
use indicatif::{
  HumanDuration,
  ProgressBar,
  ProgressStyle,
};
use rand::Rng as _;
use serde::{
  Deserialize,
  Serialize,
};

use std::io::BufRead as _;
use std::sync::mpsc::{
  channel,
  Receiver,
  Sender,
};
use std::thread;
use std::time::{
  Duration,
  Instant,
};

use super::{
  is_shell_command,
  CommandRunner,
  Precondition,
  Shell,
  TaskContext,
  TaskDependency,
};
use crate::cache::{
  compute_fingerprint,
  expand_patterns_in_dir,
  CacheEntry,
};
use crate::defaults::default_verbose;
use crate::run_shell_command;
use crate::secrets::load_secret_env;
use crate::utils::{
  deserialize_environment,
  load_env_files_in_dir,
  resolve_path,
};

fn default_cache_enabled() -> bool {
  true
}

fn default_fail_fast() -> bool {
  true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
  Sequential,
  Parallel,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskExecution {
  #[serde(default)]
  pub mode: Option<ExecutionMode>,

  #[serde(default)]
  pub max_parallel: Option<usize>,

  #[serde(default = "default_fail_fast")]
  pub fail_fast: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskCache {
  #[serde(default = "default_cache_enabled")]
  pub enabled: bool,
}

/// This struct represents a task that can be executed. A task can contain multiple
/// commands that are executed sequentially. A task can also have preconditions that
/// must be met before the task can be executed.
#[derive(Debug, Default, Deserialize)]
pub struct TaskArgs {
  /// The commands to run
  pub commands: Vec<CommandRunner>,

  /// The preconditions that must be met before the task can be executed
  #[serde(default)]
  pub preconditions: Vec<Precondition>,

  /// The tasks that must be executed before this task can be executed
  #[serde(default)]
  pub depends_on: Vec<TaskDependency>,

  /// The labels for the task
  #[serde(default)]
  pub labels: HashMap<String, String>,

  /// The description of the task
  #[serde(default)]
  pub description: String,

  /// The environment variables to set before running the task
  #[serde(default, deserialize_with = "deserialize_environment")]
  pub environment: HashMap<String, String>,

  /// The environment files to load before running the task
  #[serde(default)]
  pub env_file: Vec<String>,

  /// Secret paths to load as dotenv-style environment entries before running the task
  #[serde(default)]
  pub secrets_path: Vec<String>,

  /// The path to the secret vault
  #[serde(default)]
  pub vault_location: Option<String>,

  /// The path to the private keys used for secret decryption
  #[serde(default)]
  pub keys_location: Option<String>,

  /// The key name to use for secret decryption
  #[serde(default)]
  pub key_name: Option<String>,

  /// The shell to use when running the task
  #[serde(default)]
  pub shell: Option<Shell>,

  /// Run the commands in parallel
  /// It should only work if the task are local_run commands
  #[serde(default)]
  pub parallel: Option<bool>,

  /// Richer execution configuration
  #[serde(default)]
  pub execution: Option<TaskExecution>,

  /// Task caching configuration
  #[serde(default)]
  pub cache: Option<TaskCache>,

  /// Files or glob patterns that affect task output
  #[serde(default)]
  pub inputs: Vec<String>,

  /// Files produced by the task
  #[serde(default)]
  pub outputs: Vec<String>,

  /// Ignore errors if the task fails
  #[serde(default)]
  pub ignore_errors: Option<bool>,

  /// Show verbose output
  #[serde(default)]
  pub verbose: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Task {
  String(String),
  Task(Box<TaskArgs>),
}

#[derive(Debug)]
pub struct CommandResult {
  index: usize,
  success: bool,
  message: String,
}

impl Task {
  pub fn run(&self, context: &mut TaskContext) -> anyhow::Result<()> {
    match self {
      Task::String(command) => self.execute(context, command),
      Task::Task(args) => args.run(context),
    }
  }

  fn execute(&self, context: &mut TaskContext, command: &str) -> anyhow::Result<()> {
    assert!(!command.is_empty());

    TaskArgs {
      commands: vec![CommandRunner::CommandRun(command.to_string())],
      ..Default::default()
    }
    .run(context)
  }
}

impl TaskArgs {
  pub fn run(&self, context: &mut TaskContext) -> anyhow::Result<()> {
    assert!(!self.commands.is_empty());

    // Validate parallel execution requirements early
    self.validate_parallel_commands()?;

    let started = Instant::now();
    let tick_interval = Duration::from_millis(80);

    if let Some(shell) = &self.shell {
      context.set_shell(shell);
    }

    if let Some(ignore_errors) = &self.ignore_errors {
      context.set_ignore_errors(*ignore_errors);
    }

    if let Some(verbose) = &self.verbose {
      context.set_verbose(*verbose);
    }

    if !context.is_nested {
      if let Some(vault_location) = &context.task_root.vault_location {
        context.set_secret_vault_location(vault_location.clone());
      }

      if let Some(keys_location) = &context.task_root.keys_location {
        context.set_secret_keys_location(keys_location.clone());
      }

      if let Some(key_name) = &context.task_root.key_name {
        context.set_secret_key_name(key_name.clone());
      }
    }

    if let Some(vault_location) = &self.vault_location {
      context.set_secret_vault_location(vault_location.clone());
    }

    if let Some(keys_location) = &self.keys_location {
      context.set_secret_keys_location(keys_location.clone());
    }

    if let Some(key_name) = &self.key_name {
      context.set_secret_key_name(key_name.clone());
    }

    // Load environment variables from root and task environments and env files.
    if !context.is_nested {
      let config_base_dir = self.config_base_dir(context);
      let root_env = context.task_root.environment.clone();
      let root_env_files = load_env_files_in_dir(&context.task_root.env_file, &config_base_dir)?;
      let root_secret_env = load_secret_env(
        &context.task_root.secrets_path,
        &config_base_dir,
        context.secret_vault_location.as_deref(),
        context.secret_keys_location.as_deref(),
        context.secret_key_name.as_deref(),
      )?;
      context.extend_env_vars(root_env);
      context.extend_env_vars(root_env_files);
      context.extend_env_vars(root_secret_env);
    }

    // Load environment variables from the task environment and env files field
    let defined_env = self.load_env(context)?;
    let additional_env = self.load_env_file(context)?;
    let secret_env = self.load_secret_env(context)?;

    context.extend_env_vars(defined_env);
    context.extend_env_vars(additional_env);
    context.extend_env_vars(secret_env);

    if self.should_skip_from_cache(context)? {
      context.emit_event(&serde_json::json!({
        "event": "task_skipped",
        "task": context.current_task_name.clone().unwrap_or_else(|| "<task>".to_string()),
        "reason": "cache_hit",
      }))?;
      return Ok(());
    }

    let mut rng = rand::thread_rng();
    // Spinners can be found here:
    // https://github.com/sindresorhus/cli-spinners/blob/main/spinners.json
    let pb_style =
      ProgressStyle::with_template("{spinner:.green} [{prefix:.bold.dim}] {wide_msg:.cyan/blue} ")?
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏⦿");

    let depends_on_pb = context.multi.add(ProgressBar::new(self.depends_on.len() as u64));

    if !self.depends_on.is_empty() {
      depends_on_pb.set_style(pb_style.clone());
      depends_on_pb.set_message("Running task dependencies...");
      depends_on_pb.enable_steady_tick(tick_interval);
      for (i, dependency) in self.depends_on.iter().enumerate() {
        thread::sleep(Duration::from_millis(rng.gen_range(40..300)));
        depends_on_pb.set_prefix(format!("{}/{}", i + 1, self.depends_on.len()));
        dependency.run(context)?;
        depends_on_pb.inc(1);
      }

      let message = format!("Dependencies completed in {}.", HumanDuration(started.elapsed()));
      if context.is_nested {
        depends_on_pb.finish_and_clear();
      } else {
        depends_on_pb.finish_with_message(message);
      }
    }

    let precondition_pb = context
      .multi
      .add(ProgressBar::new(self.preconditions.len() as u64));

    if !self.preconditions.is_empty() {
      precondition_pb.set_style(pb_style.clone());
      precondition_pb.set_message("Running task precondition...");
      precondition_pb.enable_steady_tick(tick_interval);
      for (i, precondition) in self.preconditions.iter().enumerate() {
        thread::sleep(Duration::from_millis(rng.gen_range(40..300)));
        precondition_pb.set_prefix(format!("{}/{}", i + 1, self.preconditions.len()));
        precondition.execute(context)?;
        precondition_pb.inc(1);
      }

      let message = format!("Preconditions completed in {}.", HumanDuration(started.elapsed()));
      if context.is_nested {
        precondition_pb.finish_and_clear();
      } else {
        precondition_pb.finish_with_message(message);
      }
    }

    if self.is_parallel() {
      self.execute_commands_parallel(context)?;
    } else {
      let command_pb = context.multi.add(ProgressBar::new(self.commands.len() as u64));
      command_pb.set_style(pb_style);
      command_pb.set_message("Running task command...");
      command_pb.enable_steady_tick(tick_interval);
      for (i, command) in self.commands.iter().enumerate() {
        thread::sleep(Duration::from_millis(rng.gen_range(100..400)));
        command_pb.set_prefix(format!("{}/{}", i + 1, self.commands.len()));
        command.execute(context)?;
        command_pb.inc(1);
      }

      let message = format!("Commands completed in {}.", HumanDuration(started.elapsed()));
      if context.is_nested {
        command_pb.finish_and_clear();
      } else {
        command_pb.finish_with_message(message);
      }
    }

    self.update_cache(context)?;

    Ok(())
  }

  /// Validate if the task can be run in parallel
  fn validate_parallel_commands(&self) -> anyhow::Result<()> {
    if !self.is_parallel() {
      return Ok(());
    }

    for command in &self.commands {
      match command {
        CommandRunner::LocalRun(local_run) if local_run.is_parallel_safe() => continue,
        CommandRunner::LocalRun(_) => {
          return Err(anyhow::anyhow!(
            "Interactive local commands cannot be run in parallel"
          ))
        },
        _ => {
          return Err(anyhow::anyhow!(
            "Parallel execution is only supported for non-interactive local commands"
          ))
        },
      }
    }
    Ok(())
  }

  /// Execute the commands in parallel
  fn execute_commands_parallel(&self, context: &TaskContext) -> anyhow::Result<()> {
    let (tx, rx): (Sender<CommandResult>, Receiver<CommandResult>) = channel();
    let mut handles = vec![];
    let command_count = self.commands.len();
    let max_parallel = self.max_parallel().min(command_count.max(1));
    let fail_fast = self.fail_fast();
    let command_pb = context.multi.add(ProgressBar::new(command_count as u64));
    command_pb.set_style(ProgressStyle::with_template(
      "{spinner:.green} [{prefix:.bold.dim}] {wide_msg:.cyan/blue} ",
    )?);
    command_pb.set_prefix("?/?");
    command_pb.set_message("Running task commands in parallel...");
    command_pb.enable_steady_tick(Duration::from_millis(80));
    let mut failures = Vec::new();

    // Clone all commands upfront to avoid borrowing issues
    let commands: Vec<_> = self.commands.to_vec();

    // Track results in order
    let mut completed = 0;

    let mut iter = commands.into_iter().enumerate();
    let mut running = 0usize;
    let mut stop_scheduling = false;

    while completed < command_count {
      while !stop_scheduling && running < max_parallel {
        let Some((i, command)) = iter.next() else {
          break;
        };

        let tx = tx.clone();
        let context = context.clone();

        let handle = thread::spawn(move || {
          let result = match command.execute(&context) {
            Ok(_) => CommandResult {
              index: i,
              success: true,
              message: format!("Command {} completed successfully", i + 1),
            },
            Err(e) => CommandResult {
              index: i,
              success: false,
              message: format!("Command {} failed: {}", i + 1, e),
            },
          };
          tx.send(result).unwrap();
        });

        handles.push(handle);
        running += 1;
      }

      if running == 0 {
        break;
      }

      match rx.recv() {
        Ok(result) => {
          running -= 1;
          let index = result.index;
          if !result.success && !context.ignore_errors() {
            failures.push(result.message);
            if fail_fast {
              stop_scheduling = true;
            }
          }

          completed += 1;
          command_pb.set_prefix(format!("{}/{}", completed, command_count));
          command_pb.inc(1);

          command_pb.set_message(format!(
            "Running task commands in parallel (completed {})",
            index + 1
          ));
        },
        Err(e) => {
          command_pb.finish_with_message("Error receiving command results");
          return Err(anyhow::anyhow!("Channel error: {}", e));
        },
      }
    }

    // Wait for all threads to complete
    for handle in handles {
      handle.join().unwrap();
    }

    if !failures.is_empty() {
      command_pb.finish_with_message("Some commands failed");

      // Sort failures by command index for clearer error reporting
      failures.sort();
      return Err(anyhow::anyhow!("Failed commands:\n{}", failures.join("\n")));
    }

    let message = "Commands completed in parallel";
    if context.is_nested {
      command_pb.finish_and_clear();
    } else {
      command_pb.finish_with_message(message);
    }

    Ok(())
  }

  fn load_env(&self, context: &TaskContext) -> anyhow::Result<HashMap<String, String>> {
    let mut local_env: HashMap<String, String> = HashMap::new();
    for (key, value) in &self.environment {
      let value = self.get_env_value(context, value)?;
      local_env.insert(key.clone(), value);
    }

    Ok(local_env)
  }

  fn load_env_file(&self, context: &TaskContext) -> anyhow::Result<HashMap<String, String>> {
    load_env_files_in_dir(&self.env_file, &self.config_base_dir(context))
  }

  fn load_secret_env(&self, context: &TaskContext) -> anyhow::Result<HashMap<String, String>> {
    load_secret_env(
      &self.secrets_path,
      &self.config_base_dir(context),
      context.secret_vault_location.as_deref(),
      context.secret_keys_location.as_deref(),
      context.secret_key_name.as_deref(),
    )
  }

  fn get_env_value(&self, context: &TaskContext, value_in: &str) -> anyhow::Result<String> {
    if is_shell_command(value_in)? {
      let verbose = self.verbose();
      let mut cmd = self
        .shell
        .as_ref()
        .map(|shell| shell.proc())
        .unwrap_or_else(|| context.shell().proc());
      let output = run_shell_command!(value_in, cmd, verbose);
      Ok(output)
    } else if super::is_template_command(value_in)? {
      Ok(crate::schema::resolve_template_command_value(value_in, context)?)
    } else {
      Ok(value_in.to_string())
    }
  }

  fn verbose(&self) -> bool {
    self.verbose.unwrap_or(default_verbose())
  }

  pub(crate) fn execution_mode(&self) -> ExecutionMode {
    self
      .execution
      .as_ref()
      .and_then(|execution| execution.mode.clone())
      .or_else(|| {
        self.parallel.map(|parallel| {
          if parallel {
            ExecutionMode::Parallel
          } else {
            ExecutionMode::Sequential
          }
        })
      })
      .unwrap_or(ExecutionMode::Sequential)
  }

  pub(crate) fn is_parallel(&self) -> bool {
    self.execution_mode().is_parallel()
  }

  pub(crate) fn max_parallel(&self) -> usize {
    self
      .execution
      .as_ref()
      .and_then(|execution| execution.max_parallel)
      .unwrap_or(self.commands.len().max(1))
  }

  pub(crate) fn fail_fast(&self) -> bool {
    self
      .execution
      .as_ref()
      .map(|execution| execution.fail_fast)
      .unwrap_or(true)
  }

  fn cache_enabled(&self) -> bool {
    self.cache.as_ref().map(|cache| cache.enabled).unwrap_or(false)
  }

  fn should_skip_from_cache(&self, context: &TaskContext) -> anyhow::Result<bool> {
    if context.force || !self.cache_enabled() || self.outputs.is_empty() {
      return Ok(false);
    }

    let resolved_outputs = self.resolve_output_paths(context)?;
    let outputs_exist = self
      .resolve_output_paths(context)?
      .iter()
      .all(|output| output.exists());
    if !outputs_exist {
      return Ok(false);
    }

    let env_vars = sorted_env_vars(&context.env_vars);
    let inputs = self.resolve_input_paths(context)?;
    let mut env_files = self.resolve_env_file_paths(context);
    env_files.extend(self.resolve_secret_paths(context));
    env_files.sort();
    env_files.dedup();
    let fingerprint = compute_fingerprint(
      &context
        .current_task_name
        .clone()
        .unwrap_or_else(|| "<task>".to_string()),
      &stable_task_debug(self),
      &env_vars,
      &inputs,
      &env_files,
      &resolved_outputs,
    )?;

    let store = context
      .cache_store
      .lock()
      .map_err(|e| anyhow::anyhow!("Failed to lock cache store - {}", e))?;
    let Some(entry) = store.tasks.get(&fingerprint_task_key(context, self)) else {
      return Ok(false);
    };

    Ok(entry.fingerprint == fingerprint)
  }

  fn update_cache(&self, context: &TaskContext) -> anyhow::Result<()> {
    if !self.cache_enabled() || self.outputs.is_empty() {
      return Ok(());
    }

    let env_vars = sorted_env_vars(&context.env_vars);
    let inputs = self.resolve_input_paths(context)?;
    let mut env_files = self.resolve_env_file_paths(context);
    env_files.extend(self.resolve_secret_paths(context));
    env_files.sort();
    env_files.dedup();
    let resolved_outputs = self.resolve_output_paths(context)?;
    let fingerprint = compute_fingerprint(
      &context
        .current_task_name
        .clone()
        .unwrap_or_else(|| "<task>".to_string()),
      &stable_task_debug(self),
      &env_vars,
      &inputs,
      &env_files,
      &resolved_outputs,
    )?;
    let key = fingerprint_task_key(context, self);

    {
      let mut store = context
        .cache_store
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock cache store - {}", e))?;
      store.tasks.insert(
        key,
        CacheEntry {
          fingerprint,
          outputs: resolved_outputs
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
          updated_at: chrono::Utc::now().to_rfc3339(),
        },
      );
      store.save_in_dir(&context.task_root.cache_base_dir())?;
    }

    Ok(())
  }

  pub(crate) fn config_base_dir_from_root(&self, root: &super::TaskRoot) -> std::path::PathBuf {
    root.config_base_dir()
  }

  pub(crate) fn task_base_dir_from_root(&self, root: &super::TaskRoot) -> std::path::PathBuf {
    let config_base_dir = self.config_base_dir_from_root(root);
    let mut work_dirs = self
      .commands
      .iter()
      .filter_map(|command| match command {
        CommandRunner::LocalRun(local_run) => local_run.work_dir.as_ref(),
        _ => None,
      })
      .map(|work_dir| resolve_path(&config_base_dir, work_dir))
      .collect::<Vec<_>>();

    work_dirs.sort();
    work_dirs.dedup();

    if work_dirs.len() == 1 {
      work_dirs.remove(0)
    } else {
      config_base_dir
    }
  }

  fn config_base_dir(&self, context: &TaskContext) -> std::path::PathBuf {
    self.config_base_dir_from_root(&context.task_root)
  }

  fn task_base_dir(&self, context: &TaskContext) -> std::path::PathBuf {
    self.task_base_dir_from_root(&context.task_root)
  }

  fn resolve_input_paths(&self, context: &TaskContext) -> anyhow::Result<Vec<std::path::PathBuf>> {
    expand_patterns_in_dir(&self.task_base_dir(context), &self.inputs)
  }

  fn resolve_output_paths(&self, context: &TaskContext) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let base_dir = self.task_base_dir(context);
    Ok(
      self
        .outputs
        .iter()
        .map(|output| resolve_path(&base_dir, output))
        .collect(),
    )
  }

  fn resolve_env_file_paths(&self, context: &TaskContext) -> Vec<std::path::PathBuf> {
    let config_base_dir = self.config_base_dir(context);
    let mut env_files = context
      .task_root
      .env_file
      .iter()
      .chain(self.env_file.iter())
      .map(|env_file| resolve_path(&config_base_dir, env_file))
      .collect::<Vec<_>>();
    env_files.sort();
    env_files.dedup();
    env_files
  }

  fn resolve_secret_paths(&self, context: &TaskContext) -> Vec<std::path::PathBuf> {
    let config_base_dir = self.config_base_dir(context);
    let vault_location = context
      .secret_vault_location
      .as_deref()
      .map(|path| resolve_path(&config_base_dir, path))
      .unwrap_or_else(|| resolve_path(&config_base_dir, "./.mk/vault"));
    let mut secret_paths = context
      .task_root
      .secrets_path
      .iter()
      .chain(self.secrets_path.iter())
      .map(|secret_path| vault_location.join(secret_path))
      .collect::<Vec<_>>();
    secret_paths.sort();
    secret_paths.dedup();
    secret_paths
  }
}

impl ExecutionMode {
  fn is_parallel(&self) -> bool {
    matches!(self, ExecutionMode::Parallel)
  }
}

fn sorted_env_vars(env_vars: &HashMap<String, String>) -> Vec<(String, String)> {
  let mut pairs: Vec<_> = env_vars
    .iter()
    .map(|(key, value)| (key.clone(), value.clone()))
    .collect();
  pairs.sort();
  pairs
}

fn fingerprint_task_key(context: &TaskContext, task: &TaskArgs) -> String {
  context.current_task_name.clone().unwrap_or_else(|| {
    if !task.description.is_empty() {
      task.description.clone()
    } else {
      format!("{:?}", task.commands)
    }
  })
}

fn stable_task_debug(task: &TaskArgs) -> String {
  let mut labels: Vec<_> = task
    .labels
    .iter()
    .map(|(key, value)| (key.clone(), value.clone()))
    .collect();
  labels.sort();

  let mut environment: Vec<_> = task
    .environment
    .iter()
    .map(|(key, value)| (key.clone(), value.clone()))
    .collect();
  environment.sort();

  let mut secrets_path = task.secrets_path.clone();
  secrets_path.sort();

  format!(
    "commands={:?};preconditions={:?};depends_on={:?};labels={:?};description={:?};environment={:?};env_file={:?};secrets_path={:?};vault_location={:?};keys_location={:?};key_name={:?};shell={:?};execution_mode={:?};max_parallel={:?};fail_fast={};cache_enabled={};inputs={:?};outputs={:?};ignore_errors={:?};verbose={:?}",
    task.commands,
    task.preconditions,
    task.depends_on,
    labels,
    task.description,
    environment,
    task.env_file,
    secrets_path,
    task.vault_location,
    task.keys_location,
    task.key_name,
    task.shell,
    task.execution_mode(),
    task.execution.as_ref().and_then(|execution| execution.max_parallel),
    task.fail_fast(),
    task.cache_enabled(),
    task.inputs,
    task.outputs,
    task.ignore_errors,
    task.verbose
  )
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_task_1() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - command: echo \"Hello, World!\"
            ignore_errors: false
            verbose: false
        depends_on:
          - name: task1
        description: This is a task
        environment:
          FOO: bar
        env_file:
          - test.env
          - test2.env
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
          assert_eq!(local_run.command, "echo \"Hello, World!\"");
          assert_eq!(local_run.work_dir, None);
          assert_eq!(local_run.ignore_errors, Some(false));
          assert_eq!(local_run.verbose, Some(false));
        }

        if let TaskDependency::TaskDependency(args) = &task.depends_on[0] {
          assert_eq!(args.name, "task1");
        }

        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.description, "This is a task");
        assert_eq!(task.environment.len(), 1);
        assert_eq!(task.env_file.len(), 2);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_2() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - command: echo 'Hello, World!'
            ignore_errors: false
            verbose: false
        description: This is a task
        environment:
          FOO: bar
          BAR: foo
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
          assert_eq!(local_run.command, "echo 'Hello, World!'");
          assert_eq!(local_run.work_dir, None);
          assert_eq!(local_run.ignore_errors, Some(false));
          assert_eq!(local_run.verbose, Some(false));
        }

        assert_eq!(task.description, "This is a task");
        assert_eq!(task.depends_on.len(), 0);
        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.env_file.len(), 0);
        assert_eq!(task.environment.len(), 2);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_3() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - command: echo 'Hello, World!'
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
          assert_eq!(local_run.command, "echo 'Hello, World!'");
          assert_eq!(local_run.work_dir, None);
          assert_eq!(local_run.ignore_errors, None);
          assert_eq!(local_run.verbose, None);
        }

        assert_eq!(task.description.len(), 0);
        assert_eq!(task.depends_on.len(), 0);
        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.env_file.len(), 0);
        assert_eq!(task.environment.len(), 0);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_4() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - container_command:
              - echo
              - Hello, World!
            image: docker.io/library/hello-world:latest
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::ContainerRun(container_run) = &task.commands[0] {
          assert_eq!(container_run.container_command.len(), 2);
          assert_eq!(container_run.container_command[0], "echo");
          assert_eq!(container_run.container_command[1], "Hello, World!");
          assert_eq!(container_run.image, "docker.io/library/hello-world:latest");
          assert_eq!(container_run.mounted_paths, Vec::<String>::new());
          assert_eq!(container_run.ignore_errors, None);
          assert_eq!(container_run.verbose, None);
        }

        assert_eq!(task.description.len(), 0);
        assert_eq!(task.depends_on.len(), 0);
        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.env_file.len(), 0);
        assert_eq!(task.environment.len(), 0);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_5() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - container_command:
              - echo
              - Hello, World!
            image: docker.io/library/hello-world:latest
            mounted_paths:
              - /tmp
              - /var/tmp
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::ContainerRun(container_run) = &task.commands[0] {
          assert_eq!(container_run.container_command.len(), 2);
          assert_eq!(container_run.container_command[0], "echo");
          assert_eq!(container_run.container_command[1], "Hello, World!");
          assert_eq!(container_run.image, "docker.io/library/hello-world:latest");
          assert_eq!(container_run.mounted_paths, vec!["/tmp", "/var/tmp"]);
          assert_eq!(container_run.ignore_errors, None);
          assert_eq!(container_run.verbose, None);
        }

        assert_eq!(task.description.len(), 0);
        assert_eq!(task.depends_on.len(), 0);
        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.env_file.len(), 0);
        assert_eq!(task.environment.len(), 0);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_6() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - container_command:
              - echo
              - Hello, World!
            image: docker.io/library/hello-world:latest
            mounted_paths:
              - /tmp
              - /var/tmp
            ignore_errors: true
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::ContainerRun(container_run) = &task.commands[0] {
          assert_eq!(container_run.container_command.len(), 2);
          assert_eq!(container_run.container_command[0], "echo");
          assert_eq!(container_run.container_command[1], "Hello, World!");
          assert_eq!(container_run.image, "docker.io/library/hello-world:latest");
          assert_eq!(container_run.mounted_paths, vec!["/tmp", "/var/tmp"]);
          assert_eq!(container_run.ignore_errors, Some(true));
          assert_eq!(container_run.verbose, None);
        }

        assert_eq!(task.description.len(), 0);
        assert_eq!(task.depends_on.len(), 0);
        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.env_file.len(), 0);
        assert_eq!(task.environment.len(), 0);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_7() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - container_command:
              - echo
              - Hello, World!
            image: docker.io/library/hello-world:latest
            verbose: false
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::ContainerRun(container_run) = &task.commands[0] {
          assert_eq!(container_run.container_command.len(), 2);
          assert_eq!(container_run.container_command[0], "echo");
          assert_eq!(container_run.container_command[1], "Hello, World!");
          assert_eq!(container_run.image, "docker.io/library/hello-world:latest");
          assert_eq!(container_run.mounted_paths, Vec::<String>::new());
          assert_eq!(container_run.ignore_errors, None);
          assert_eq!(container_run.verbose, Some(false));
        }

        assert_eq!(task.description.len(), 0);
        assert_eq!(task.depends_on.len(), 0);
        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.env_file.len(), 0);
        assert_eq!(task.environment.len(), 0);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_8() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - task: task1
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::TaskRun(task_run) = &task.commands[0] {
          assert_eq!(task_run.task, "task1");
          assert_eq!(task_run.ignore_errors, None);
          assert_eq!(task_run.verbose, None);
        }

        assert_eq!(task.description.len(), 0);
        assert_eq!(task.depends_on.len(), 0);
        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.env_file.len(), 0);
        assert_eq!(task.environment.len(), 0);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_9() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - task: task1
            verbose: true
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::TaskRun(task_run) = &task.commands[0] {
          assert_eq!(task_run.task, "task1");
          assert_eq!(task_run.ignore_errors, None);
          assert_eq!(task_run.verbose, Some(true));
        }

        assert_eq!(task.description.len(), 0);
        assert_eq!(task.depends_on.len(), 0);
        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.env_file.len(), 0);
        assert_eq!(task.environment.len(), 0);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_10() -> anyhow::Result<()> {
    {
      let yaml = "
        commands:
          - task: task1
            ignore_errors: true
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        if let CommandRunner::TaskRun(task_run) = &task.commands[0] {
          assert_eq!(task_run.task, "task1");
          assert_eq!(task_run.ignore_errors, Some(true));
          assert_eq!(task_run.verbose, None);
        }

        assert_eq!(task.description.len(), 0);
        assert_eq!(task.depends_on.len(), 0);
        assert_eq!(task.labels.len(), 0);
        assert_eq!(task.env_file.len(), 0);
        assert_eq!(task.environment.len(), 0);
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_11() -> anyhow::Result<()> {
    {
      let yaml = "
        echo 'Hello, World!'
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::String(task) = &task {
        assert_eq!(task, "echo 'Hello, World!'");
      } else {
        panic!("Expected Task::String");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_12() -> anyhow::Result<()> {
    {
      let yaml = "
        'true'
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::String(task) = &task {
        assert_eq!(task, "true");
      } else {
        panic!("Expected Task::String");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_13() -> anyhow::Result<()> {
    {
      let yaml = "
        commands: []
        environment:
          FOO: bar
          BAR: foo
          KEY: 42
          PIS: 3.14
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let Task::Task(task) = &task {
        assert_eq!(task.environment.len(), 4);
        assert_eq!(task.environment.get("FOO").unwrap(), "bar");
        assert_eq!(task.environment.get("BAR").unwrap(), "foo");
        assert_eq!(task.environment.get("KEY").unwrap(), "42");
      } else {
        panic!("Expected Task::Task");
      }

      Ok(())
    }
  }

  #[test]
  fn test_task_14() -> anyhow::Result<()> {
    let yaml = "
      commands: []
      secrets_path:
        - app/common
      vault_location: ./.mk/vault
      keys_location: ./.mk/keys
      key_name: team
      environment:
        SECRET_VALUE: ${{ secrets.app/password }}
    ";

    let task = serde_yaml::from_str::<Task>(yaml)?;

    if let Task::Task(task) = &task {
      assert_eq!(task.secrets_path, vec!["app/common"]);
      assert_eq!(task.vault_location.as_deref(), Some("./.mk/vault"));
      assert_eq!(task.keys_location.as_deref(), Some("./.mk/keys"));
      assert_eq!(task.key_name.as_deref(), Some("team"));
      assert_eq!(
        task.environment.get("SECRET_VALUE").map(String::as_str),
        Some("${{ secrets.app/password }}")
      );
    } else {
      panic!("Expected Task::Task");
    }

    Ok(())
  }

  #[test]
  fn test_parallel_interactive_rejected() -> anyhow::Result<()> {
    let yaml = r#"
          commands:
            - command: "echo hello"
              interactive: true
            - command: "echo world"
          parallel: true
      "#;

    let task = serde_yaml::from_str::<Task>(yaml)?;
    let mut context = TaskContext::empty();

    if let Task::Task(task) = task {
      let result = task.run(&mut context);
      assert!(result.is_err());
      assert!(result
        .unwrap_err()
        .to_string()
        .contains("Interactive local commands cannot be run in parallel"));
    }

    Ok(())
  }

  #[test]
  fn test_parallel_non_interactive_accepted() -> anyhow::Result<()> {
    let yaml = r#"
          commands:
            - command: "echo hello"
              interactive: false
            - command: "echo world"
          parallel: true
      "#;

    let task = serde_yaml::from_str::<Task>(yaml)?;
    let mut context = TaskContext::empty();

    if let Task::Task(task) = task {
      let result = task.run(&mut context);
      assert!(result.is_ok());
    }

    Ok(())
  }
}
