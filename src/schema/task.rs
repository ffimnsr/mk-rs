use anyhow::Context;
use hashbrown::HashMap;
use indicatif::{
  HumanDuration,
  ProgressBar,
  ProgressStyle,
};
use rand::Rng as _;
use serde::Deserialize;

use std::io::BufRead as _;
use std::sync::mpsc::{
  channel,
  Receiver,
  Sender,
};
use std::time::{
  Duration,
  Instant,
};
use std::{
  fs,
  thread,
};

use super::{
  is_shell_command,
  CommandRunner,
  Precondition,
  Shell,
  TaskContext,
  TaskDependency,
};
use crate::defaults::default_verbose;
use crate::run_shell_command;
use crate::utils::deserialize_environment;

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

  /// The shell to use when running the task
  #[serde(default)]
  pub shell: Option<Shell>,

  /// Run the commands in parallel
  /// It should only work if the task are local_run commands
  #[serde(default)]
  pub parallel: Option<bool>,

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

    // Load environment variables from the task environment and env files field
    let defined_env = self.load_env(context)?;
    let additional_env = self.load_env_file()?;

    context.extend_env_vars(defined_env);
    context.extend_env_vars(additional_env);

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

    if self.parallel.unwrap_or(false) {
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

    Ok(())
  }

  /// Validate if the task can be run in parallel
  fn validate_parallel_commands(&self) -> anyhow::Result<()> {
    if !self.parallel.unwrap_or(false) {
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

    // Clone all commands upfront to avoid borrowing issues
    let commands: Vec<_> = self.commands.to_vec();

    // Track results in order
    let mut completed = 0;

    for (i, command) in commands.into_iter().enumerate() {
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
    }

    // Set up progress bar
    let command_pb = context.multi.add(ProgressBar::new(command_count as u64));
    command_pb.set_style(ProgressStyle::with_template(
      "{spinner:.green} [{prefix:.bold.dim}] {wide_msg:.cyan/blue} ",
    )?);
    command_pb.set_prefix("?/?");
    command_pb.set_message("Running task commands in parallel...");
    command_pb.enable_steady_tick(Duration::from_millis(80));

    // Process results as they come in
    let mut failures = Vec::new();

    while completed < command_count {
      match rx.recv() {
        Ok(result) => {
          let index = result.index;
          if !result.success && !context.ignore_errors() {
            failures.push(result.message);
          }

          completed += 1;
          command_pb.set_prefix(format!("{}/{}", completed, command_count));
          command_pb.inc(1);

          // Update progress message with latest completed command
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

  fn load_env_file(&self) -> anyhow::Result<HashMap<String, String>> {
    let mut local_env: HashMap<String, String> = HashMap::new();
    for env_file in &self.env_file {
      let contents =
        fs::read_to_string(env_file).with_context(|| format!("Failed to read env file - {}", env_file))?;

      for line in contents.lines() {
        if let Some((key, value)) = line.split_once('=') {
          local_env.insert(key.trim().to_string(), value.trim().to_string());
        }
      }
    }

    Ok(local_env)
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
    } else {
      Ok(value_in.to_string())
    }
  }

  fn verbose(&self) -> bool {
    self.verbose.unwrap_or(default_verbose())
  }
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
