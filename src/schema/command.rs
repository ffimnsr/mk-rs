use anyhow::{
  Context as _,
  Ok,
};
use serde::{
  Deserialize,
  Serialize,
};
use std::io::{
  BufRead as _,
  BufReader,
};
use std::process::{
  Command as ProcessCommand,
  Stdio,
};
use std::{
  env,
  thread,
};
use which::which;

use crate::defaults::{default_shell, default_true};
use super::TaskContext;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CommandRunner {
  LocalRun {
    /// The command to run
    command: String,

    /// The shell to use to run the command
    #[serde(default = "default_shell")]
    shell: String,

    /// The working directory to run the command in
    #[serde(default)]
    work_dir: Option<String>,

    /// Ignore errors if the command fails
    #[serde(default)]
    ignore_errors: bool,

    /// Show verbose output
    #[serde(default = "default_true")]
    verbose: bool,
  },
  TaskRun {
    /// The name of the task to run
    task: String,

    #[serde(default)]
    ignore_errors: bool,

    #[serde(default = "default_true")]
    verbose: bool,
  },
  ContainerRun {
    /// The command to run in the container
    container_command: Vec<String>,

    /// The container image to use
    image: String,

    /// The mounted paths to bind mount into the container
    #[serde(default)]
    mounted_paths: Vec<String>,

    /// Ignore errors if the command fails
    #[serde(default)]
    ignore_errors: bool,

    /// Show verbose output
    #[serde(default = "default_true")]
    verbose: bool,
  },
}

impl CommandRunner {
  pub fn execute(&self, context: &mut TaskContext) -> anyhow::Result<()> {
    match self {
      CommandRunner::LocalRun {
        command,
        shell,
        work_dir,
        ignore_errors,
        verbose,
      } => self.execute_local_run(context, command, shell, work_dir, *ignore_errors, *verbose),
      CommandRunner::TaskRun {
        task,
        ignore_errors,
        verbose,
      } => self.execute_task_run(context, task, *ignore_errors, *verbose),
      CommandRunner::ContainerRun {
        container_command,
        image,
        mounted_paths,
        ignore_errors,
        verbose,
      } => self.execute_container_run(
        context,
        container_command,
        image,
        mounted_paths,
        *ignore_errors,
        *verbose,
      ),
    }
  }

  fn execute_local_run(
    &self,
    context: &TaskContext,
    command: &str,
    shell: &str,
    work_dir: &Option<String>,
    ignore_errors: bool,
    verbose: bool,
  ) -> anyhow::Result<()> {
    let stdout = if verbose { Stdio::piped() } else { Stdio::null() };
    let stderr = if verbose { Stdio::piped() } else { Stdio::null() };

    let mut cmd = ProcessCommand::new(shell);
    cmd.arg("-c").arg(command).stdout(stdout).stderr(stderr);

    if let Some(work_dir) = work_dir {
      cmd.current_dir(work_dir);
    }

    // Inject environment variables
    for (key, value) in context.env_vars.iter() {
      cmd.env(key, value);
    }

    let mut cmd = cmd.spawn()?;

    if verbose {
      let stdout = cmd.stdout.take().with_context(|| "Failed to open stdout")?;
      let stderr = cmd.stderr.take().with_context(|| "Failed to open stderr")?;

      let multi_clone = context.multi.clone();
      thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
          let _ = multi_clone.println(line);
        }
      });

      let multi_clone = context.multi.clone();
      thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
          let _ = multi_clone.println(line);
        }
      });
    }

    let status = cmd.wait()?;
    if !status.success() && !ignore_errors {
      anyhow::bail!("Command failed: {}", command);
    }

    Ok(())
  }

  fn execute_task_run(
    &self,
    context: &TaskContext,
    task_name: &str,
    ignore_errors: bool,
    verbose: bool,
  ) -> anyhow::Result<()> {
    let task = context
      .task_root
      .tasks
      .get(task_name)
      .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

    log::trace!("Task: {:?}", task);

    {
      let mut stack = context
        .execution_stack
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock execution stack: {}", e))?;

      if stack.contains(task_name) {
        anyhow::bail!("Circular dependency detected: {}", task_name);
      }

      stack.insert(task_name.into());
    }

    let mut context = TaskContext::from_context_with_args(context, ignore_errors, verbose);
    task.run(&mut context)?;

    Ok(())
  }

  fn execute_container_run(
    &self,
    context: &TaskContext,
    command: &[String],
    image: &str,
    mounted_paths: &[String],
    ignore_errors: bool,
    verbose: bool,
  ) -> anyhow::Result<()> {
    let stdout = if verbose { Stdio::piped() } else { Stdio::null() };
    let stderr = if verbose { Stdio::piped() } else { Stdio::null() };

    let container_runtime = which("docker")
      .or_else(|_| which("podman"))
      .with_context(|| "Failed to find docker or podman")?;

    let mut cmd = ProcessCommand::new(container_runtime);
    cmd.arg("run").arg("--rm").arg("-i").stdout(stdout).stderr(stderr);

    let current_dir = env::current_dir()?;
    cmd.arg("-v").arg(format!("{}:/workdir:z", current_dir.display()));
    cmd.arg("-w").arg("/workdir");

    for mounted_path in mounted_paths {
      cmd.arg("-v").arg(mounted_path);
    }

    // Inject environment variables in both container and command
    for (key, value) in context.env_vars.iter() {
      cmd.env(key, value);
      cmd.arg("-e").arg(format!("{}={}", key, value));
    }

    cmd.arg(image).args(command);

    log::trace!("Running command: {:?}", cmd);

    let mut cmd = cmd.spawn()?;
    if verbose {
      let stdout = cmd.stdout.take().with_context(|| "Failed to open stdout")?;
      let stderr = cmd.stderr.take().with_context(|| "Failed to open stderr")?;

      let multi_clone = context.multi.clone();
      thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
          let _ = multi_clone.println(line);
        }
      });

      let multi_clone = context.multi.clone();
      thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
          let _ = multi_clone.println(line);
        }
      });
    }

    let status = cmd.wait()?;
    if !status.success() && !ignore_errors {
      anyhow::bail!("Command failed: {}", command.join(" "));
    }

    Ok(())
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_command_1() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        ignore_errors: false
        verbose: false
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;

      if let CommandRunner::LocalRun {
        command,
        shell,
        work_dir,
        ignore_errors,
        verbose,
      } = command
      {
        assert_eq!(command, "echo \"Hello, World!\"");
        assert_eq!(shell, "sh");
        assert_eq!(work_dir, None);
        assert!(!ignore_errors);
        assert!(!verbose);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }

  #[test]
  fn test_command_2() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;

      if let CommandRunner::LocalRun {
        command,
        shell,
        work_dir,
        ignore_errors,
        verbose,
      } = command
      {
        assert_eq!(command, "echo \"Hello, World!\"");
        assert_eq!(shell, "sh");
        assert_eq!(work_dir, None);
        assert!(!ignore_errors);
        assert!(verbose);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }

  #[test]
  fn test_command_3() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        ignore_errors: true
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;
      if let CommandRunner::LocalRun {
        command,
        shell,
        work_dir,
        ignore_errors,
        verbose,
      } = command
      {
        assert_eq!(command, "echo \"Hello, World!\"");
        assert_eq!(shell, "sh");
        assert_eq!(work_dir, None);
        assert!(ignore_errors);
        assert!(verbose);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }

  #[test]
  fn test_command_4() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        verbose: false
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;
      if let CommandRunner::LocalRun {
        command,
        shell,
        work_dir,
        ignore_errors,
        verbose,
      } = command
      {
        assert_eq!(command, "echo \"Hello, World!\"");
        assert_eq!(shell, "sh");
        assert_eq!(work_dir, None);
        assert!(!ignore_errors);
        assert!(!verbose);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }

  #[test]
  fn test_command_5() -> anyhow::Result<()> {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        work_dir: /tmp
      ";
      let command = serde_yaml::from_str::<CommandRunner>(yaml)?;
      if let CommandRunner::LocalRun {
        command,
        shell,
        work_dir,
        ignore_errors,
        verbose,
      } = command
      {
        assert_eq!(command, "echo \"Hello, World!\"");
        assert_eq!(shell, "sh");
        assert_eq!(work_dir, Some("/tmp".into()));
        assert!(!ignore_errors);
        assert!(verbose);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      Ok(())
    }
  }
}
