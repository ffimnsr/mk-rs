use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;

use super::{
  contains_output_reference,
  extract_output_references,
  CommandRunner,
  ContainerRuntime,
  Include,
  Task,
  TaskRoot,
  UseCargo,
  UseNpm,
};

#[derive(Debug, Clone, Serialize)]
pub struct ValidationIssue {
  pub severity: ValidationSeverity,
  pub task: Option<String>,
  pub field: Option<String>,
  pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
  Error,
  Warning,
}

#[derive(Debug, Default, Serialize)]
pub struct ValidationReport {
  pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
  pub fn push_error(&mut self, task: Option<&str>, field: Option<&str>, message: impl Into<String>) {
    self.issues.push(ValidationIssue {
      severity: ValidationSeverity::Error,
      task: task.map(str::to_string),
      field: field.map(str::to_string),
      message: message.into(),
    });
  }

  pub fn push_warning(&mut self, task: Option<&str>, field: Option<&str>, message: impl Into<String>) {
    self.issues.push(ValidationIssue {
      severity: ValidationSeverity::Warning,
      task: task.map(str::to_string),
      field: field.map(str::to_string),
      message: message.into(),
    });
  }

  pub fn has_errors(&self) -> bool {
    self
      .issues
      .iter()
      .any(|issue| issue.severity == ValidationSeverity::Error)
  }
}

impl TaskRoot {
  pub fn validate(&self) -> ValidationReport {
    let mut report = ValidationReport::default();

    self.validate_root(&mut report);

    for (task_name, task) in &self.tasks {
      self.validate_task(task_name, task, &mut report);
    }

    self.validate_cycles(&mut report);

    report
  }

  fn validate_root(&self, report: &mut ValidationReport) {
    if let Some(use_npm) = &self.use_npm {
      self.validate_use_npm(use_npm, report);
    }

    if let Some(use_cargo) = &self.use_cargo {
      self.validate_use_cargo(use_cargo, report);
    }

    if let Some(includes) = &self.include {
      self.validate_includes(includes, report);
    }

    self.validate_runtime(
      None,
      Some("container_runtime"),
      self.container_runtime.as_ref(),
      report,
    );
  }

  fn validate_task(&self, task_name: &str, task: &Task, report: &mut ValidationReport) {
    match task {
      Task::String(command) => {
        if command.trim().is_empty() {
          report.push_error(Some(task_name), Some("commands"), "Command must not be empty");
        }
      },
      Task::Task(task) => {
        if task.commands.is_empty() {
          report.push_error(
            Some(task_name),
            Some("commands"),
            "Task must define at least one command",
          );
        }

        for dependency in &task.depends_on {
          let dependency_name = dependency.resolve_name();
          if dependency_name.is_empty() {
            report.push_error(
              Some(task_name),
              Some("depends_on"),
              "Dependency name must not be empty",
            );
          } else if dependency_name == task_name {
            report.push_error(
              Some(task_name),
              Some("depends_on"),
              "Task cannot depend on itself",
            );
          } else if !self.tasks.contains_key(dependency_name) {
            report.push_error(
              Some(task_name),
              Some("depends_on"),
              format!("Missing dependency: {}", dependency_name),
            );
          }
        }

        if task.is_parallel() {
          for command in &task.commands {
            match command {
              CommandRunner::LocalRun(local_run) if local_run.is_parallel_safe() => {},
              CommandRunner::LocalRun(_) => report.push_error(
                Some(task_name),
                Some("parallel"),
                "Parallel execution only supports non-interactive local commands",
              ),
              _ => report.push_error(
                Some(task_name),
                Some("parallel"),
                "Parallel execution only supports non-interactive local commands",
              ),
            }
          }

          if task
            .environment
            .values()
            .any(|value| contains_output_reference(value))
            || task.commands.iter().any(command_uses_task_outputs)
          {
            report.push_error(
              Some(task_name),
              Some("execution.mode"),
              "Parallel execution does not support saved command outputs",
            );
          }
        }

        if let Some(execution) = &task.execution {
          if let Some(max_parallel) = execution.max_parallel {
            if max_parallel == 0 {
              report.push_error(
                Some(task_name),
                Some("execution.max_parallel"),
                "execution.max_parallel must be greater than zero",
              );
            }
          }
        }

        if task.cache.as_ref().map(|cache| cache.enabled).unwrap_or(false) && task.outputs.is_empty() {
          report.push_warning(
            Some(task_name),
            Some("outputs"),
            "Task cache is enabled without declared outputs; cache hits will not be possible",
          );
        }

        for command in &task.commands {
          self.validate_command(task_name, command, report);
        }

        self.validate_command_outputs(task_name, task, report);
      },
    }
  }

  fn validate_command(&self, task_name: &str, command: &CommandRunner, report: &mut ValidationReport) {
    match command {
      CommandRunner::CommandRun(command) => {
        if command.trim().is_empty() {
          report.push_error(Some(task_name), Some("command"), "Command must not be empty");
        }
        if contains_output_reference(command) {
          report.push_error(
            Some(task_name),
            Some("command"),
            "Saved command outputs are only supported by local `command:` entries",
          );
        }
      },
      CommandRunner::LocalRun(local_run) => {
        if local_run.command.trim().is_empty() {
          report.push_error(Some(task_name), Some("command"), "Command must not be empty");
        }
        if let Some(save_output_as) = &local_run.save_output_as {
          if save_output_as.trim().is_empty() {
            report.push_error(
              Some(task_name),
              Some("save_output_as"),
              "save_output_as must not be empty",
            );
          }
        }
      },
      CommandRunner::ContainerRun(container_run) => {
        if container_run.image.trim().is_empty() {
          report.push_error(
            Some(task_name),
            Some("image"),
            "Container image must not be empty",
          );
        }
        if container_run.container_command.is_empty() {
          report.push_error(
            Some(task_name),
            Some("container_command"),
            "Container command must not be empty",
          );
        }
        self.validate_runtime(
          Some(task_name),
          Some("runtime"),
          container_run.runtime.as_ref(),
          report,
        );
      },
      CommandRunner::ContainerBuild(container_build) => {
        if container_build.container_build.image_name.trim().is_empty() {
          report.push_error(
            Some(task_name),
            Some("container_build.image_name"),
            "Container image_name must not be empty",
          );
        }
        if container_build.container_build.context.trim().is_empty() {
          report.push_error(
            Some(task_name),
            Some("container_build.context"),
            "Container build context must not be empty",
          );
        }
        if container_build.container_build.containerfile.is_none()
          && !has_default_containerfile(&self.resolve_from_config(&container_build.container_build.context))
        {
          report.push_warning(
            Some(task_name),
            Some("container_build.containerfile"),
            "No explicit containerfile set and no Dockerfile or Containerfile was found in the build context",
          );
        }
        self.validate_runtime(
          Some(task_name),
          Some("container_build.runtime"),
          container_build.container_build.runtime.as_ref(),
          report,
        );
      },
      CommandRunner::TaskRun(task_run) => {
        if task_run.task.trim().is_empty() {
          report.push_error(Some(task_name), Some("task"), "Task name must not be empty");
        } else if !self.tasks.contains_key(&task_run.task) {
          report.push_error(
            Some(task_name),
            Some("task"),
            format!("Referenced task does not exist: {}", task_run.task),
          );
        }
      },
    }
  }

  fn validate_command_outputs(&self, task_name: &str, task: &super::TaskArgs, report: &mut ValidationReport) {
    let declared_outputs = task
      .commands
      .iter()
      .filter_map(|command| match command {
        CommandRunner::LocalRun(local_run) => local_run.save_output_as.as_ref(),
        _ => None,
      })
      .map(|name| name.trim().to_string())
      .filter(|name| !name.is_empty())
      .collect::<HashSet<_>>();

    for value in task.environment.values() {
      for output_name in extract_output_references(value) {
        if !declared_outputs.contains(&output_name) {
          report.push_error(
            Some(task_name),
            Some("environment"),
            format!("Unknown task output reference: {}", output_name),
          );
        }
      }
    }

    let mut produced_outputs = HashSet::new();
    for command in &task.commands {
      match command {
        CommandRunner::LocalRun(local_run) => {
          for output_name in extract_output_references(&local_run.command) {
            if !produced_outputs.contains(&output_name) {
              report.push_error(
                Some(task_name),
                Some("command"),
                format!(
                  "Output reference must come from an earlier command: {}",
                  output_name
                ),
              );
            }
          }

          if let Some(test) = &local_run.test {
            for output_name in extract_output_references(test) {
              if !produced_outputs.contains(&output_name) {
                report.push_error(
                  Some(task_name),
                  Some("test"),
                  format!(
                    "Output reference must come from an earlier command: {}",
                    output_name
                  ),
                );
              }
            }
          }

          if let Some(save_output_as) = &local_run.save_output_as {
            let save_output_as = save_output_as.trim().to_string();
            if !save_output_as.is_empty() && !produced_outputs.insert(save_output_as.clone()) {
              report.push_error(
                Some(task_name),
                Some("save_output_as"),
                format!("Duplicate saved output name: {}", save_output_as),
              );
            }
          }
        },
        CommandRunner::ContainerRun(_) | CommandRunner::ContainerBuild(_) | CommandRunner::TaskRun(_) => {},
        CommandRunner::CommandRun(command) => {
          for output_name in extract_output_references(command) {
            if !produced_outputs.contains(&output_name) {
              report.push_error(
                Some(task_name),
                Some("command"),
                format!(
                  "Output reference must come from an earlier command: {}",
                  output_name
                ),
              );
            }
          }
        },
      }
    }
  }

  fn validate_use_npm(&self, use_npm: &UseNpm, report: &mut ValidationReport) {
    let work_dir = match use_npm {
      UseNpm::Bool(true) => None,
      UseNpm::UseNpm(args) => args.work_dir.as_deref(),
      _ => return,
    };

    let package_json = work_dir
      .map(|path| self.resolve_from_config(path).join("package.json"))
      .unwrap_or_else(|| self.resolve_from_config("package.json"));

    if !package_json.is_file() {
      report.push_error(
        None,
        Some("use_npm"),
        format!("package.json does not exist: {}", package_json.to_string_lossy()),
      );
    }
  }

  fn validate_use_cargo(&self, use_cargo: &UseCargo, report: &mut ValidationReport) {
    let work_dir = match use_cargo {
      UseCargo::Bool(true) => None,
      UseCargo::UseCargo(args) => args.work_dir.as_deref(),
      _ => return,
    };

    if let Some(work_dir) = work_dir {
      let path = self.resolve_from_config(work_dir);
      if !path.is_dir() {
        report.push_error(
          None,
          Some("use_cargo.work_dir"),
          format!("Cargo work_dir does not exist: {}", path.to_string_lossy()),
        );
      }
    }
  }

  fn validate_runtime(
    &self,
    task: Option<&str>,
    field: Option<&str>,
    runtime: Option<&ContainerRuntime>,
    report: &mut ValidationReport,
  ) {
    if let Some(runtime) = runtime {
      if ContainerRuntime::resolve(Some(runtime)).is_err() {
        report.push_error(
          task,
          field,
          format!("Requested container runtime is unavailable: {}", runtime.name()),
        );
      }
    }
  }

  fn validate_includes(&self, includes: &[Include], report: &mut ValidationReport) {
    for include in includes {
      let name = include.name();

      if name.trim().is_empty() {
        report.push_error(None, Some("include"), "Include name must not be empty");
        continue;
      }

      let overwrite_suffix = if include.overwrite() {
        " (overwrite=true)"
      } else {
        ""
      };
      report.push_error(
        None,
        Some("include"),
        format!(
          "`include` is no longer supported. Replace it with `extends`: {}{}",
          name, overwrite_suffix
        ),
      );
    }
  }

  fn validate_cycles(&self, report: &mut ValidationReport) {
    let mut visited = HashSet::new();
    let mut visiting = Vec::new();

    for task_name in self.tasks.keys() {
      self.detect_cycle(task_name, &mut visiting, &mut visited, report);
    }
  }

  fn detect_cycle(
    &self,
    task_name: &str,
    visiting: &mut Vec<String>,
    visited: &mut HashSet<String>,
    report: &mut ValidationReport,
  ) {
    if visited.contains(task_name) {
      return;
    }

    if let Some(index) = visiting.iter().position(|name| name == task_name) {
      let mut cycle = visiting[index..].to_vec();
      cycle.push(task_name.to_string());
      report.push_error(
        Some(task_name),
        Some("depends_on"),
        format!("Circular dependency detected: {}", cycle.join(" -> ")),
      );
      return;
    }

    visiting.push(task_name.to_string());

    if let Some(Task::Task(task)) = self.tasks.get(task_name) {
      for dependency in &task.depends_on {
        self.detect_cycle(dependency.resolve_name(), visiting, visited, report);
      }

      for command in &task.commands {
        if let CommandRunner::TaskRun(task_run) = command {
          self.detect_cycle(&task_run.task, visiting, visited, report);
        }
      }
    }

    visiting.pop();
    visited.insert(task_name.to_string());
  }
}

fn command_uses_task_outputs(command: &CommandRunner) -> bool {
  match command {
    CommandRunner::LocalRun(local_run) => {
      local_run.save_output_as.is_some()
        || contains_output_reference(&local_run.command)
        || local_run
          .test
          .as_ref()
          .is_some_and(|test| contains_output_reference(test))
    },
    CommandRunner::CommandRun(command) => contains_output_reference(command),
    CommandRunner::ContainerRun(_) | CommandRunner::ContainerBuild(_) | CommandRunner::TaskRun(_) => false,
  }
}

fn has_default_containerfile(context_path: &Path) -> bool {
  context_path.join("Dockerfile").is_file() || context_path.join("Containerfile").is_file()
}
