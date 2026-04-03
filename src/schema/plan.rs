use std::collections::HashSet;

use serde::Serialize;

use crate::defaults::default_shell;

use super::{
  CommandRunner,
  Shell,
  Task,
  TaskArgs,
  TaskRoot,
};

#[derive(Debug, Serialize)]
pub struct TaskPlan {
  pub root_task: String,
  pub steps: Vec<PlannedTask>,
}

#[derive(Debug, Serialize)]
pub struct PlannedTask {
  pub name: String,
  pub description: Option<String>,
  pub commands: Vec<PlannedCommand>,
  pub dependencies: Vec<String>,
  pub base_dir: String,
  pub execution_mode: PlannedExecutionMode,
  pub max_parallel: Option<usize>,
  pub skipped_reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannedExecutionMode {
  Sequential,
  Parallel,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlannedCommand {
  CommandRun {
    command: String,
    shell: String,
  },
  LocalRun {
    command: String,
    shell: Option<String>,
    work_dir: Option<String>,
    interactive: bool,
  },
  ContainerRun {
    runtime: String,
    image: String,
    command: Vec<String>,
    mounted_paths: Vec<String>,
  },
  ContainerBuild {
    runtime: String,
    image_name: String,
    context: String,
    containerfile: Option<String>,
    tags: Vec<String>,
    build_args: Vec<String>,
    labels: Vec<String>,
  },
  TaskRun {
    task: String,
  },
}

impl PlannedCommand {
  pub fn summary(&self) -> String {
    match self {
      PlannedCommand::CommandRun { command, .. } => format!("command: {}", command),
      PlannedCommand::LocalRun { command, .. } => format!("local: {}", command),
      PlannedCommand::ContainerRun { image, command, .. } => {
        format!("container_run: {} -> {}", image, command.join(" "))
      },
      PlannedCommand::ContainerBuild {
        image_name, context, ..
      } => format!("container_build: {} ({})", image_name, context),
      PlannedCommand::TaskRun { task } => format!("task: {}", task),
    }
  }
}

impl TaskRoot {
  pub fn plan_task(&self, task_name: &str) -> anyhow::Result<TaskPlan> {
    let mut planner = Planner::default();
    planner.visit_task(self, task_name)?;
    Ok(TaskPlan {
      root_task: task_name.to_string(),
      steps: planner.steps,
    })
  }
}

#[derive(Default)]
struct Planner {
  steps: Vec<PlannedTask>,
  visiting: HashSet<String>,
  visited: HashSet<String>,
}

impl Planner {
  fn visit_task(&mut self, root: &TaskRoot, task_name: &str) -> anyhow::Result<()> {
    if self.visited.contains(task_name) {
      return Ok(());
    }

    if !self.visiting.insert(task_name.to_string()) {
      anyhow::bail!("Circular dependency detected - {}", task_name);
    }

    let task = root
      .tasks
      .get(task_name)
      .ok_or_else(|| anyhow::anyhow!("Task not found - {}", task_name))?;

    let planned_task = match task {
      Task::String(command) => PlannedTask {
        name: task_name.to_string(),
        description: None,
        commands: vec![PlannedCommand::CommandRun {
          command: command.clone(),
          shell: default_shell().cmd(),
        }],
        dependencies: Vec::new(),
        base_dir: root.config_base_dir().to_string_lossy().into_owned(),
        execution_mode: PlannedExecutionMode::Sequential,
        max_parallel: None,
        skipped_reason: None,
      },
      Task::Task(task) => {
        for dependency in &task.depends_on {
          self.visit_task(root, dependency.resolve_name())?;
        }

        PlannedTask {
          name: task_name.to_string(),
          description: if task.description.is_empty() {
            None
          } else {
            Some(task.description.clone())
          },
          commands: task
            .commands
            .iter()
            .map(|command| PlannedCommand::from_task_command(root, task, command))
            .collect(),
          dependencies: task
            .depends_on
            .iter()
            .map(|dependency| dependency.resolve_name().to_string())
            .collect(),
          base_dir: task.task_base_dir_from_root(root).to_string_lossy().into_owned(),
          execution_mode: if task.is_parallel() {
            PlannedExecutionMode::Parallel
          } else {
            PlannedExecutionMode::Sequential
          },
          max_parallel: if task.is_parallel() {
            Some(task.max_parallel())
          } else {
            None
          },
          skipped_reason: None,
        }
      },
    };

    self.visiting.remove(task_name);
    self.visited.insert(task_name.to_string());
    self.steps.push(planned_task);
    Ok(())
  }
}

impl From<&CommandRunner> for PlannedCommand {
  fn from(value: &CommandRunner) -> Self {
    Self::from_task_command(&TaskRoot::default(), &TaskArgs::default(), value)
  }
}

impl PlannedCommand {
  fn from_task_command(root: &TaskRoot, task: &TaskArgs, value: &CommandRunner) -> Self {
    match value {
      CommandRunner::CommandRun(command) => PlannedCommand::CommandRun {
        command: command.clone(),
        shell: effective_shell(task, None).cmd(),
      },
      CommandRunner::LocalRun(local_run) => PlannedCommand::LocalRun {
        command: local_run.command.clone(),
        shell: Some(effective_shell(task, local_run.shell.as_ref()).cmd()),
        work_dir: local_run
          .work_dir
          .as_ref()
          .map(|work_dir| root.resolve_from_config(work_dir).to_string_lossy().into_owned()),
        interactive: local_run.interactive.unwrap_or(false),
      },
      CommandRunner::ContainerRun(container_run) => PlannedCommand::ContainerRun {
        runtime: container_run
          .runtime
          .as_ref()
          .or(root.container_runtime.as_ref())
          .map(|runtime| runtime.name().to_string())
          .unwrap_or_else(|| "auto".to_string()),
        image: container_run.image.clone(),
        command: container_run.container_command.clone(),
        mounted_paths: container_run
          .mounted_paths
          .iter()
          .map(|mounted_path| resolve_plan_mount_spec(root, mounted_path))
          .collect(),
      },
      CommandRunner::ContainerBuild(container_build) => PlannedCommand::ContainerBuild {
        runtime: container_build
          .container_build
          .runtime
          .as_ref()
          .or(root.container_runtime.as_ref())
          .map(|runtime| runtime.name().to_string())
          .unwrap_or_else(|| "auto".to_string()),
        image_name: container_build.container_build.image_name.clone(),
        context: root
          .resolve_from_config(&container_build.container_build.context)
          .to_string_lossy()
          .into_owned(),
        containerfile: container_build
          .container_build
          .containerfile
          .as_ref()
          .map(|containerfile| {
            root
              .resolve_from_config(containerfile)
              .to_string_lossy()
              .into_owned()
          }),
        tags: container_build
          .container_build
          .tags
          .clone()
          .unwrap_or_else(|| vec!["latest".to_string()]),
        build_args: container_build
          .container_build
          .build_args
          .clone()
          .unwrap_or_default(),
        labels: container_build.container_build.labels.clone().unwrap_or_default(),
      },
      CommandRunner::TaskRun(task_run) => PlannedCommand::TaskRun {
        task: task_run.task.clone(),
      },
    }
  }
}

fn effective_shell(task: &TaskArgs, command_shell: Option<&Shell>) -> Shell {
  command_shell
    .cloned()
    .or_else(|| task.shell.clone())
    .unwrap_or_else(default_shell)
}

fn resolve_plan_mount_spec(root: &TaskRoot, mounted_path: &str) -> String {
  let mut parts = mounted_path.splitn(3, ':');
  let host = parts.next().unwrap_or_default();
  let second = parts.next();
  let third = parts.next();

  if let Some(container_path) = second {
    if !should_resolve_bind_host(host, container_path) {
      return mounted_path.to_string();
    }

    let resolved_host = root.resolve_from_config(host);
    match third {
      Some(options) => format!(
        "{}:{}:{}",
        resolved_host.to_string_lossy(),
        container_path,
        options
      ),
      None => format!("{}:{}", resolved_host.to_string_lossy(), container_path),
    }
  } else {
    mounted_path.to_string()
  }
}

fn should_resolve_bind_host(host: &str, container_path: &str) -> bool {
  if host.is_empty() || container_path.is_empty() {
    return false;
  }

  host.starts_with('.')
    || host.starts_with('/')
    || host.contains('/')
    || host == "~"
    || host.starts_with("~/")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_plan_task_resolves_task_shell() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        build:
          shell: bash
          commands:
            - command: echo build
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let plan = task_root.plan_task("build")?;
    let command = &plan.steps[0].commands[0];

    match command {
      PlannedCommand::LocalRun { shell, .. } => {
        assert_eq!(shell.as_deref(), Some("bash"));
      },
      _ => panic!("Expected PlannedCommand::LocalRun"),
    }

    Ok(())
  }
}
