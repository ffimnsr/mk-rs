use anyhow::Context;
use indicatif::{
  HumanDuration,
  MultiProgress,
  ProgressBar,
  ProgressStyle,
};
use rand::Rng as _;
use serde::{
  Deserialize,
  Serialize,
};
use std::collections::HashMap;
use std::sync::Arc;
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
  ExecutionStack,
  Precondition,
  TaskDependency,
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
}

/// This struct represents a task that can be executed. A task can contain multiple
/// commands that are executed sequentially. A task can also have preconditions that
/// must be met before the task can be executed.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Task {
  pub commands: Vec<CommandRunner>,

  #[serde(default)]
  pub preconditions: Vec<Precondition>,

  #[serde(default)]
  pub depends_on: Vec<TaskDependency>,

  #[serde(default)]
  pub labels: HashMap<String, String>,

  #[serde(default)]
  pub description: String,

  #[serde(default)]
  pub environment: HashMap<String, String>,

  #[serde(default)]
  pub env_file: Vec<String>,
}

impl Task {
  pub fn run(&self, context: &mut TaskContext) -> anyhow::Result<()> {
    let started = Instant::now();

    let mut current_env = context.env_vars.clone();

    // Load environment variables from the task environment and env files field
    let defined_env = self.environment.clone();
    let additional_env = self.load_env_file()?;

    current_env.extend(defined_env);
    current_env.extend(additional_env);

    context.env_vars = current_env;

    let mut rng = rand::thread_rng();
    // Spinners can be found here:
    // https://github.com/sindresorhus/cli-spinners/blob/main/spinners.json
    let pb_style =
      ProgressStyle::with_template("{spinner:.green} [{prefix:.bold.dim}] {wide_msg:.cyan/blue} ")?
        .tick_chars("⣾⣽⣻⢿⡿⣟⣯⣷");

    let depends_on_pb = context.multi.add(ProgressBar::new(self.depends_on.len() as u64));

    if !self.depends_on.is_empty() {
      depends_on_pb.set_style(pb_style.clone());
      depends_on_pb.set_message("Running task dependencies...");
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
        fs::read_to_string(env_file).with_context(|| format!("Failed to read env file: {}", env_file))?;

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

      if let CommandRunner::LocalRun {
        command,
        work_dir,
        shell,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(command, "echo \"Hello, World!\"");
        assert_eq!(work_dir, &None);
        assert_eq!(shell, "sh");
        assert_eq!(ignore_errors, &false);
        assert_eq!(verbose, &false);
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

      if let CommandRunner::LocalRun {
        command,
        work_dir,
        shell,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(command, "echo 'Hello, World!'");
        assert_eq!(*work_dir, None);
        assert_eq!(shell, "sh");
        assert!(!*ignore_errors);
        assert!(!*verbose);
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

      if let CommandRunner::LocalRun {
        command,
        work_dir,
        shell,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(command, "echo 'Hello, World!'");
        assert_eq!(*work_dir, None);
        assert_eq!(shell, "sh");
        assert!(!*ignore_errors);
        assert!(!*verbose);
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

      if let CommandRunner::ContainerRun {
        container_command,
        image,
        mounted_paths,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(container_command.len(), 2);
        assert_eq!(container_command[0], "echo");
        assert_eq!(container_command[1], "Hello, World!");
        assert_eq!(image, "docker.io/library/hello-world:latest");
        assert_eq!(*mounted_paths, Vec::<String>::new());
        assert!(!*ignore_errors);
        assert!(!*verbose);
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

      if let CommandRunner::ContainerRun {
        container_command,
        image,
        mounted_paths,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(container_command.len(), 2);
        assert_eq!(container_command[0], "echo");
        assert_eq!(container_command[1], "Hello, World!");
        assert_eq!(image, "docker.io/library/hello-world:latest");
        assert_eq!(*mounted_paths, vec!["/tmp", "/var/tmp"]);
        assert!(!*ignore_errors);
        assert!(!*verbose);
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

      if let CommandRunner::ContainerRun {
        container_command,
        image,
        mounted_paths,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(container_command.len(), 2);
        assert_eq!(container_command[0], "echo");
        assert_eq!(container_command[1], "Hello, World!");
        assert_eq!(image, "docker.io/library/hello-world:latest");
        assert_eq!(*mounted_paths, vec!["/tmp", "/var/tmp"]);
        assert!(*ignore_errors);
        assert!(!*verbose);
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
            verbose: true
      ";

      let task = serde_yaml::from_str::<Task>(yaml)?;

      if let CommandRunner::ContainerRun {
        container_command,
        image,
        mounted_paths,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(container_command.len(), 2);
        assert_eq!(container_command[0], "echo");
        assert_eq!(container_command[1], "Hello, World!");
        assert_eq!(image, "docker.io/library/hello-world:latest");
        assert_eq!(*mounted_paths, Vec::<String>::new());
        assert!(!*ignore_errors);
        assert!(*verbose);
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

      if let CommandRunner::TaskRun {
        task,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(task, "task1");
        assert!(!*ignore_errors);
        assert!(!*verbose);
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

      if let CommandRunner::TaskRun {
        task,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(task, "task1");
        assert!(!*ignore_errors);
        assert!(*verbose);
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

      if let CommandRunner::TaskRun {
        task,
        ignore_errors,
        verbose,
      } = &task.commands[0]
      {
        assert_eq!(task, "task1");
        assert!(*ignore_errors);
        assert!(!*verbose);
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
