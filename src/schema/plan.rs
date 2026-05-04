use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use crate::defaults::default_shell;

use super::{
  interpolate_matrix_template_string, CommandRunner, MatrixSelector, Shell, Task, TaskArgs, TaskRoot,
};

#[derive(Debug, Serialize)]
pub struct TaskPlan {
  pub root_task: String,
  pub steps: Vec<PlannedTask>,
}

#[derive(Debug, Serialize)]
pub struct PlannedTask {
  pub name: String,
  pub matrix: Option<BTreeMap<String, String>>,
  pub description: Option<String>,
  pub commands: Vec<PlannedCommand>,
  pub dependencies: Vec<String>,
  pub base_dir: String,
  pub execution_mode: PlannedExecutionMode,
  pub max_parallel: Option<usize>,
  pub skipped_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannedExecutionMode {
  Sequential,
  Parallel,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlannedCommand {
  MakeRun {
    makefile: String,
    target: String,
  },
  CommandRun {
    command: String,
    shell: String,
  },
  LocalRun {
    command: String,
    shell: Option<String>,
    work_dir: Option<String>,
    interactive: bool,
    retrigger: bool,
  },
  SshRun {
    host: String,
    user: Option<String>,
    port: Option<u16>,
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
  JsonExtract {
    extract_json_from: String,
    json_path: String,
    save_as: String,
  },
  WriteOutput {
    write_output: String,
    to_file: String,
    create_parents: bool,
  },
}

impl PlannedCommand {
  pub fn summary(&self) -> String {
    match self {
      PlannedCommand::MakeRun { makefile, target } => {
        format!("make: make -f {} {}", makefile, target)
      },
      PlannedCommand::CommandRun { command, .. } => format!("command: {}", command),
      PlannedCommand::LocalRun { command, .. } => format!("local: {}", command),
      PlannedCommand::SshRun { host, command, .. } => format!("ssh:{} -> {}", host, command),
      PlannedCommand::ContainerRun { image, command, .. } => {
        format!("container_run: {} -> {}", image, command.join(" "))
      },
      PlannedCommand::ContainerBuild {
        image_name, context, ..
      } => format!("container_build: {} ({})", image_name, context),
      PlannedCommand::TaskRun { task } => format!("task: {}", task),
      PlannedCommand::JsonExtract {
        extract_json_from,
        json_path,
        save_as,
      } => format!("json_extract: {}.{} -> {}", extract_json_from, json_path, save_as),
      PlannedCommand::WriteOutput {
        write_output,
        to_file,
        ..
      } => {
        format!("write_output: {} -> {}", write_output, to_file)
      },
    }
  }
}

impl TaskRoot {
  pub fn plan_task(&self, task_name: &str) -> anyhow::Result<TaskPlan> {
    self.plan_task_with_selectors(task_name, &[])
  }

  pub fn plan_task_with_selectors(
    &self,
    task_name: &str,
    selectors: &[MatrixSelector],
  ) -> anyhow::Result<TaskPlan> {
    let mut planner = Planner::new(task_name, selectors);
    planner.visit_task(self, task_name)?;
    Ok(TaskPlan {
      root_task: task_name.to_string(),
      steps: planner.steps,
    })
  }
}

struct Planner {
  steps: Vec<PlannedTask>,
  visiting: HashSet<String>,
  visited: HashSet<String>,
  root_task_name: String,
  root_matrix_selectors: Vec<MatrixSelector>,
}

impl Planner {
  fn new(task_name: &str, selectors: &[MatrixSelector]) -> Self {
    Self {
      steps: Vec::new(),
      visiting: HashSet::new(),
      visited: HashSet::new(),
      root_task_name: task_name.to_string(),
      root_matrix_selectors: selectors.to_vec(),
    }
  }

  fn visit_task(&mut self, root: &TaskRoot, task_name: &str) -> anyhow::Result<()> {
    if self.visited.contains(task_name) {
      return Ok(());
    }

    if !self.visiting.insert(task_name.to_string()) {
      anyhow::bail!("Circular dependency detected - {}", task_name);
    }

    let task = root.tasks.get(task_name).ok_or_else(|| {
      anyhow::anyhow!(
        "Task '{}' not found. Run 'mk list' to see available tasks.",
        task_name
      )
    })?;

    match task {
      Task::String(command) => {
        self.steps.push(PlannedTask {
          name: task_name.to_string(),
          matrix: None,
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
        });
      },
      Task::Task(task) => {
        for dependency in &task.depends_on {
          self.visit_task(root, dependency.resolve_name())?;
        }

        let variants = if task_name == self.root_task_name {
          task.select_matrix_variants(task_name, &self.root_matrix_selectors)?
        } else {
          task.expand_matrix_variants(task_name)?
        };
        let dependencies = task
          .depends_on
          .iter()
          .map(|dependency| dependency.resolve_name().to_string())
          .collect::<Vec<_>>();
        let base_dir = task.task_base_dir_from_root(root).to_string_lossy().into_owned();
        let execution_mode = if task.is_parallel() {
          PlannedExecutionMode::Parallel
        } else {
          PlannedExecutionMode::Sequential
        };
        let max_parallel = if task.is_parallel() {
          Some(task.max_parallel())
        } else {
          None
        };

        let planned_variants = variants
          .into_iter()
          .map(|variant| {
            let matrix = variant.values.clone();
            let commands = if root.is_makefile_config() {
              vec![PlannedCommand::MakeRun {
                makefile: root
                  .source_path
                  .as_ref()
                  .map(|path| path.to_string_lossy().into_owned())
                  .unwrap_or_else(|| String::from("Makefile")),
                target: task_name.to_string(),
              }]
            } else {
              task
                .commands
                .iter()
                .map(|command| PlannedCommand::from_task_command(root, task, command, &variant.values))
                .collect()
            };

            PlannedTask {
              name: variant.name,
              matrix: if matrix.is_empty() {
                None
              } else {
                Some(matrix.clone())
              },
              description: if task.description.is_empty() {
                None
              } else {
                Some(interpolate_matrix_template_string(&task.description, &matrix))
              },
              commands,
              dependencies: dependencies.clone(),
              base_dir: base_dir.clone(),
              execution_mode,
              max_parallel,
              skipped_reason: None,
            }
          })
          .collect::<Vec<_>>();

        self.steps.extend(planned_variants);
      },
    }

    self.visiting.remove(task_name);
    self.visited.insert(task_name.to_string());
    Ok(())
  }
}

impl From<&CommandRunner> for PlannedCommand {
  fn from(value: &CommandRunner) -> Self {
    Self::from_task_command(
      &TaskRoot::default(),
      &TaskArgs::default(),
      value,
      &BTreeMap::new(),
    )
  }
}

impl PlannedCommand {
  fn from_task_command(
    root: &TaskRoot,
    task: &TaskArgs,
    value: &CommandRunner,
    matrix: &BTreeMap<String, String>,
  ) -> Self {
    match value {
      CommandRunner::CommandRun(command) => PlannedCommand::CommandRun {
        command: interpolate_matrix_template_string(command, matrix),
        shell: effective_shell(task, None).cmd(),
      },
      CommandRunner::LocalRun(local_run) => PlannedCommand::LocalRun {
        command: interpolate_matrix_template_string(&local_run.command, matrix),
        shell: Some(effective_shell(task, local_run.shell.as_ref()).cmd()),
        work_dir: local_run
          .work_dir
          .as_ref()
          .map(|work_dir| interpolate_matrix_template_string(work_dir, matrix))
          .map(|work_dir| root.resolve_from_config(&work_dir).to_string_lossy().into_owned()),
        interactive: local_run.interactive_enabled(),
        retrigger: local_run.retrigger_enabled(),
      },
      CommandRunner::SshRun(ssh_run) => PlannedCommand::SshRun {
        host: interpolate_matrix_template_string(&ssh_run.ssh_run.host, matrix),
        user: ssh_run
          .ssh_run
          .user
          .as_ref()
          .map(|user| interpolate_matrix_template_string(user, matrix)),
        port: ssh_run.ssh_run.port,
        command: interpolate_matrix_template_string(&ssh_run.ssh_run.command, matrix),
        shell: ssh_run.ssh_run.shell.as_ref().map(|shell| shell.cmd()),
        work_dir: ssh_run
          .ssh_run
          .work_dir
          .as_ref()
          .map(|work_dir| interpolate_matrix_template_string(work_dir, matrix)),
        interactive: ssh_run.interactive_enabled(),
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
      CommandRunner::JsonExtract(json_extract) => PlannedCommand::JsonExtract {
        extract_json_from: interpolate_matrix_template_string(&json_extract.extract_json_from, matrix),
        json_path: interpolate_matrix_template_string(&json_extract.json_path, matrix),
        save_as: interpolate_matrix_template_string(&json_extract.save_as, matrix),
      },
      CommandRunner::WriteOutput(write_output) => PlannedCommand::WriteOutput {
        write_output: interpolate_matrix_template_string(&write_output.write_output, matrix),
        to_file: interpolate_matrix_template_string(&write_output.to_file, matrix),
        create_parents: write_output.create_parents.unwrap_or(false),
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

  #[test]
  fn test_plan_task_includes_retrigger() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        dev:
          commands:
            - command: go run .
              retrigger: true
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let plan = task_root.plan_task("dev")?;
    let command = &plan.steps[0].commands[0];

    match command {
      PlannedCommand::LocalRun { retrigger, .. } => {
        assert!(*retrigger);
      },
      _ => panic!("Expected PlannedCommand::LocalRun"),
    }

    Ok(())
  }

  #[test]
  fn test_plan_task_expands_matrix_variants() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        build:
          matrix:
            os:
              - linux
              - macos
            arch:
              - x86_64
          commands:
            - command: echo ${{ matrix.os }}-${{ matrix.arch }}
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let plan = task_root.plan_task("build")?;

    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].name, "build[arch=x86_64,os=linux]");
    assert_eq!(plan.steps[1].name, "build[arch=x86_64,os=macos]");

    match &plan.steps[0].commands[0] {
      PlannedCommand::LocalRun { command, .. } => assert_eq!(command, "echo linux-x86_64"),
      _ => panic!("Expected PlannedCommand::LocalRun"),
    }

    assert_eq!(
      plan.steps[0].matrix.as_ref().and_then(|matrix| matrix.get("os")),
      Some(&"linux".to_string())
    );
    Ok(())
  }

  #[test]
  fn test_plan_task_with_selectors_filters_root_variants() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        build:
          matrix:
            os:
              - linux
              - macos
            arch:
              - x86_64
          commands:
            - command: echo ${{ matrix.os }}-${{ matrix.arch }}
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let plan = task_root.plan_task_with_selectors(
      "build",
      &[MatrixSelector {
        key: "os".to_string(),
        value: "linux".to_string(),
      }],
    )?;

    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].name, "build[arch=x86_64,os=linux]");
    Ok(())
  }
}
