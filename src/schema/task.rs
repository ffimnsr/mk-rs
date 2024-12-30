use anyhow::Context;
use indicatif::{
  HumanDuration,
  ProgressBar,
  ProgressStyle,
};
use rand::Rng as _;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{
  Duration,
  Instant,
};
use std::{
  fs,
  thread,
};

use super::{
  CommandRunner,
  Precondition,
  TaskContext,
  TaskDependency,
};
use crate::defaults::default_true;

/// This struct represents a task that can be executed. A task can contain multiple
/// commands that are executed sequentially. A task can also have preconditions that
/// must be met before the task can be executed.
#[derive(Debug, Default, Deserialize)]
pub struct Task {
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
  #[serde(default)]
  pub environment: HashMap<String, String>,

  /// The environment files to load before running the task
  #[serde(default)]
  pub env_file: Vec<String>,

  /// Ignore errors if the task fails
  #[serde(default)]
  pub ignore_errors: bool,

  /// Show verbose output
  #[serde(default = "default_true")]
  pub verbose: bool,
}

impl Task {
  pub fn run(&self, context: &mut TaskContext) -> anyhow::Result<()> {
    assert!(!self.commands.is_empty());

    let started = Instant::now();
    let tick_interval = Duration::from_millis(80);

    // Load environment variables from the task environment and env files field
    let defined_env = self.environment.clone();
    let additional_env = self.load_env_file()?;

    context.extend_env_vars(defined_env);
    context.extend_env_vars(additional_env);
    context.set_ignore_errors(self.ignore_errors);
    context.set_verbose(self.verbose);

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

    Ok(())
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

      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo \"Hello, World!\"");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.shell, "sh");
        assert!(!local_run.ignore_errors);
        assert!(!local_run.verbose);
      }

      assert_eq!(task.depends_on[0].name, "task1");
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "This is a task");
      assert_eq!(task.environment.len(), 1);
      assert_eq!(task.env_file.len(), 2);

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

      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo 'Hello, World!'");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.shell, "sh");
        assert!(!local_run.ignore_errors);
        assert!(!local_run.verbose);
      }

      assert_eq!(task.description, "This is a task");
      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.environment.len(), 2);

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

      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo 'Hello, World!'");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.shell, "sh");
        assert!(!local_run.ignore_errors);
        assert!(local_run.verbose);
      }

      assert_eq!(task.description.len(), 0);
      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.environment.len(), 0);

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

      if let CommandRunner::ContainerRun(container_run) = &task.commands[0] {
        assert_eq!(container_run.container_command.len(), 2);
        assert_eq!(container_run.container_command[0], "echo");
        assert_eq!(container_run.container_command[1], "Hello, World!");
        assert_eq!(container_run.image, "docker.io/library/hello-world:latest");
        assert_eq!(container_run.mounted_paths, Vec::<String>::new());
        assert!(!container_run.ignore_errors);
        assert!(container_run.verbose);
      }

      assert_eq!(task.description.len(), 0);
      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.environment.len(), 0);

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

      if let CommandRunner::ContainerRun(container_run) = &task.commands[0] {
        assert_eq!(container_run.container_command.len(), 2);
        assert_eq!(container_run.container_command[0], "echo");
        assert_eq!(container_run.container_command[1], "Hello, World!");
        assert_eq!(container_run.image, "docker.io/library/hello-world:latest");
        assert_eq!(container_run.mounted_paths, vec!["/tmp", "/var/tmp"]);
        assert!(!container_run.ignore_errors);
        assert!(container_run.verbose);
      }

      assert_eq!(task.description.len(), 0);
      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.environment.len(), 0);

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

      if let CommandRunner::ContainerRun(container_run) = &task.commands[0] {
        assert_eq!(container_run.container_command.len(), 2);
        assert_eq!(container_run.container_command[0], "echo");
        assert_eq!(container_run.container_command[1], "Hello, World!");
        assert_eq!(container_run.image, "docker.io/library/hello-world:latest");
        assert_eq!(container_run.mounted_paths, vec!["/tmp", "/var/tmp"]);
        assert!(container_run.ignore_errors);
        assert!(container_run.verbose);
      }

      assert_eq!(task.description.len(), 0);
      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.environment.len(), 0);

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

      if let CommandRunner::ContainerRun(container_run) = &task.commands[0] {
        assert_eq!(container_run.container_command.len(), 2);
        assert_eq!(container_run.container_command[0], "echo");
        assert_eq!(container_run.container_command[1], "Hello, World!");
        assert_eq!(container_run.image, "docker.io/library/hello-world:latest");
        assert_eq!(container_run.mounted_paths, Vec::<String>::new());
        assert!(!container_run.ignore_errors);
        assert!(!container_run.verbose);
      }

      assert_eq!(task.description.len(), 0);
      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.environment.len(), 0);

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

      if let CommandRunner::TaskRun(task_run) = &task.commands[0] {
        assert_eq!(task_run.task, "task1");
        assert!(!task_run.ignore_errors);
        assert!(task_run.verbose);
      }

      assert_eq!(task.description.len(), 0);
      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.environment.len(), 0);

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

      if let CommandRunner::TaskRun(task_run) = &task.commands[0] {
        assert_eq!(task_run.task, "task1");
        assert!(!task_run.ignore_errors);
        assert!(task_run.verbose);
      }

      assert_eq!(task.description.len(), 0);
      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.environment.len(), 0);

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

      if let CommandRunner::TaskRun(task_run) = &task.commands[0] {
        assert_eq!(task_run.task, "task1");
        assert!(task_run.ignore_errors);
        assert!(task_run.verbose);
      }

      assert_eq!(task.description.len(), 0);
      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.env_file.len(), 0);
      assert_eq!(task.environment.len(), 0);

      Ok(())
    }
  }
}
