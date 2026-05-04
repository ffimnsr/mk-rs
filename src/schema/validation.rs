use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;

use super::{
  contains_output_reference, extract_output_references, CommandRunner, ContainerRuntime, Include, Task,
  TaskRoot, UseCargo, UseNpm,
};
use crate::file::ToUtf8 as _;
use crate::schema::Precondition;
use crate::secrets::{merge_optional_secret_settings, SecretBackend, SecretSettings};

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

  pub fn sort_issues(&mut self) {
    self.issues.sort_by(|left, right| {
      severity_rank(&left.severity)
        .cmp(&severity_rank(&right.severity))
        .then_with(|| {
          left
            .task
            .as_deref()
            .unwrap_or("")
            .cmp(right.task.as_deref().unwrap_or(""))
        })
        .then_with(|| {
          left
            .field
            .as_deref()
            .unwrap_or("")
            .cmp(right.field.as_deref().unwrap_or(""))
        })
        .then_with(|| left.message.cmp(&right.message))
    });
  }
}

fn severity_rank(severity: &ValidationSeverity) -> u8 {
  match severity {
    ValidationSeverity::Error => 0,
    ValidationSeverity::Warning => 1,
  }
}

impl TaskRoot {
  pub fn validate(&self) -> ValidationReport {
    let mut report = ValidationReport::default();

    if self.is_makefile_config() {
      self.validate_makefile_config(&mut report);
      report.sort_issues();
      return report;
    }

    self.validate_loaded_config(&mut report);
    report.sort_issues();

    report
  }

  fn validate_loaded_config(&self, report: &mut ValidationReport) {
    self.validate_root(report);

    for (task_name, task) in &self.tasks {
      self.validate_task(task_name, task, report);
    }

    self.validate_cycles(report);
  }

  fn validate_makefile_config(&self, report: &mut ValidationReport) {
    let Some(source_path) = self.source_path.as_deref() else {
      report.push_error(None, Some("config"), "Makefile config path is missing");
      return;
    };

    if !source_path.exists() {
      report.push_error(
        None,
        Some("config"),
        format!(
          "Makefile config not found: {}",
          source_path.to_utf8().unwrap_or("<non-utf8-path>")
        ),
      );
      return;
    }

    if let Err(error) = crate::make::locate_make_binary() {
      report.push_error(None, Some("make"), error.to_string());
      return;
    }

    match crate::make::load_makefile_task_root(source_path) {
      Ok(imported_root) => {
        report.push_warning(
          None,
          Some("config"),
          "Make-backed config uses delegated GNU Make support. Label filters, watch, and forwarded args remain unsupported.",
        );
        imported_root.validate_loaded_config(report);
      },
      Err(error) => {
        report.push_error(None, Some("config"), error.to_string());
      },
    }
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

    self.validate_legacy_secret_settings_usage(None, &self.validation_legacy_secret_settings(), report);

    self.validate_secret_setting_combinations(
      None,
      self.raw_secrets.as_ref().or(self.secrets.as_ref()),
      &self.validation_legacy_secret_settings(),
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
        if task.commands.is_empty() && !self.is_makefile_config() {
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
              CommandRunner::LocalRun(local_run) if local_run.interactive_enabled() => report.push_error(
                Some(task_name),
                Some("parallel"),
                "Parallel execution only supports non-interactive local commands",
              ),
              CommandRunner::LocalRun(_) => report.push_error(
                Some(task_name),
                Some("parallel"),
                "Parallel execution does not support local commands with `retrigger: true`",
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

        if task.cache.as_ref().map(|cache| cache.enabled).unwrap_or(false)
          && !task.depends_on.is_empty()
          && task.inputs.is_empty()
        {
          report.push_warning(
            Some(task_name),
            Some("inputs"),
            "Cached task depends_on other tasks but declares no inputs; dependency side effects may bypass cache invalidation",
          );
        }

        if task.cache.as_ref().map(|cache| cache.enabled).unwrap_or(false)
          && task.inputs.is_empty()
          && task_uses_dynamic_runtime_inputs(task)
        {
          report.push_warning(
            Some(task_name),
            Some("inputs"),
            "Cached task contains shell-derived or runtime-derived command inputs but declares no inputs; cache invalidation may miss external changes",
          );
        }

        for command in &task.commands {
          self.validate_command(task_name, command, report);
        }

        self.validate_legacy_secret_settings_usage(
          Some(task_name),
          &task.validation_legacy_secret_settings(),
          report,
        );

        self.validate_secret_setting_combinations(
          Some(task_name),
          task.raw_secrets.as_ref().or(task.secrets.as_ref()),
          &task.validation_legacy_secret_settings(),
          report,
        );

        self.validate_command_outputs(task_name, task, report);
        self.validate_labels(task_name, task, report);
      },
    }
  }

  fn validate_labels(&self, task_name: &str, task: &super::TaskArgs, report: &mut ValidationReport) {
    for (key, value) in &task.labels {
      if key.trim().is_empty() {
        report.push_warning(Some(task_name), Some("labels"), "Label key must not be empty");
      } else if key.starts_with("mk.") {
        report.push_warning(
          Some(task_name),
          Some("labels"),
          format!("Label key '{}' uses reserved 'mk.' prefix", key),
        );
      }

      if value.trim().is_empty() {
        report.push_warning(
          Some(task_name),
          Some("labels"),
          format!("Label '{}' has an empty value", key),
        );
      }
    }
  }

  fn validate_secret_setting_combinations(
    &self,
    task_name: Option<&str>,
    secrets_block: Option<&SecretSettings>,
    legacy: &SecretSettings,
    report: &mut ValidationReport,
  ) {
    self.validate_legacy_secret_conflicts(task_name, secrets_block, legacy, report);

    let effective = merge_optional_secret_settings(Some(legacy.clone()), secrets_block.cloned());
    let Some(effective) = effective.filter(|settings| !settings.is_empty()) else {
      return;
    };

    let backend = effective.resolved_backend();
    let explicit_key_name = secrets_block
      .and_then(|settings| settings.key_name.as_ref())
      .or(legacy.key_name.as_ref());
    let explicit_keys_location = secrets_block
      .and_then(|settings| settings.keys_location.as_ref())
      .or(legacy.keys_location.as_ref());

    match backend {
      SecretBackend::Gpg => {
        if effective.gpg_key_id.is_none() {
          report.push_error(
            task_name,
            Some("secrets.gpg_key_id"),
            "GPG backend requires gpg_key_id",
          );
        }

        if explicit_key_name.is_some() || explicit_keys_location.is_some() {
          report.push_error(
            task_name,
            Some("secrets"),
            "GPG backend cannot be combined with PGP-only settings: key_name, keys_location",
          );
        }
      },
      SecretBackend::BuiltInPgp => {
        if effective.key_name.is_none() {
          report.push_error(
            task_name,
            Some("secrets.key_name"),
            "PGP backend requires key_name",
          );
        }

        if effective.keys_location.is_none() && !pgp_default_keys_location_applies() {
          report.push_error(
            task_name,
            Some("secrets.keys_location"),
            "PGP backend requires keys_location when no default applies",
          );
        }
      },
    }
  }

  fn validate_legacy_secret_conflicts(
    &self,
    task_name: Option<&str>,
    secrets_block: Option<&SecretSettings>,
    legacy: &SecretSettings,
    report: &mut ValidationReport,
  ) {
    let Some(secrets_block) = secrets_block else {
      return;
    };

    validate_secret_field_conflict(
      task_name,
      "vault_location",
      legacy.vault_location.as_ref(),
      secrets_block.vault_location.as_ref(),
      report,
    );
    validate_secret_field_conflict(
      task_name,
      "keys_location",
      legacy.keys_location.as_ref(),
      secrets_block.keys_location.as_ref(),
      report,
    );
    validate_secret_field_conflict(
      task_name,
      "key_name",
      legacy.key_name.as_ref(),
      secrets_block.key_name.as_ref(),
      report,
    );
    validate_secret_field_conflict(
      task_name,
      "gpg_key_id",
      legacy.gpg_key_id.as_ref(),
      secrets_block.gpg_key_id.as_ref(),
      report,
    );
    validate_secret_field_conflict(
      task_name,
      "secrets_path",
      legacy.secrets_path.as_ref(),
      secrets_block.secrets_path.as_ref(),
      report,
    );
  }

  fn validate_legacy_secret_settings_usage(
    &self,
    task_name: Option<&str>,
    legacy: &SecretSettings,
    report: &mut ValidationReport,
  ) {
    if legacy.is_empty() {
      return;
    }

    report.push_warning(
      task_name,
      Some("secrets"),
      "Legacy secret fields are deprecated; prefer the `secrets` block",
    );
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
        if local_run.interactive_enabled() && local_run.retrigger_enabled() {
          report.push_error(
            Some(task_name),
            Some("retrigger"),
            "retrigger is only supported for non-interactive local commands",
          );
        }
      },
      CommandRunner::SshRun(ssh_run) => {
        if ssh_run.ssh_run.host.trim().is_empty() {
          report.push_error(
            Some(task_name),
            Some("ssh_run.host"),
            "SSH host must not be empty",
          );
        }
        if ssh_run.ssh_run.command.trim().is_empty() {
          report.push_error(
            Some(task_name),
            Some("ssh_run.command"),
            "SSH command must not be empty",
          );
        }
        for (field, name_opt) in [
          ("ssh_run.save_output_as", ssh_run.ssh_run.save_output_as.as_ref()),
          ("ssh_run.save_stderr_as", ssh_run.ssh_run.save_stderr_as.as_ref()),
          (
            "ssh_run.save_exit_code_as",
            ssh_run.ssh_run.save_exit_code_as.as_ref(),
          ),
        ] {
          if let Some(name) = name_opt {
            if name.trim().is_empty() {
              report.push_error(Some(task_name), Some(field), format!("{field} must not be empty"));
            }
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
      CommandRunner::JsonExtract(json_extract) => {
        if json_extract.extract_json_from.trim().is_empty() {
          report.push_error(
            Some(task_name),
            Some("extract_json_from"),
            "extract_json_from must not be empty",
          );
        }
        if json_extract.json_path.trim().is_empty() {
          report.push_error(Some(task_name), Some("json_path"), "json_path must not be empty");
        }
        if json_extract.save_as.trim().is_empty() {
          report.push_error(Some(task_name), Some("save_as"), "save_as must not be empty");
        }
      },
      CommandRunner::WriteOutput(write_output) => {
        if write_output.write_output.trim().is_empty() {
          report.push_error(
            Some(task_name),
            Some("write_output"),
            "write_output must not be empty",
          );
        }
        if write_output.to_file.trim().is_empty() {
          report.push_error(Some(task_name), Some("to_file"), "to_file must not be empty");
        }
      },
    }
  }

  fn validate_command_outputs(&self, task_name: &str, task: &super::TaskArgs, report: &mut ValidationReport) {
    let declared_outputs = task
      .commands
      .iter()
      .flat_map(|command| match command {
        CommandRunner::LocalRun(local_run) => {
          let mut names = vec![];
          if let Some(n) = &local_run.save_output_as {
            names.push(n.trim().to_string());
          }
          if let Some(n) = &local_run.save_stderr_as {
            names.push(n.trim().to_string());
          }
          if let Some(n) = &local_run.save_exit_code_as {
            names.push(n.trim().to_string());
          }
          names
        },
        CommandRunner::SshRun(ssh_run) => {
          let mut names = vec![];
          if let Some(n) = &ssh_run.ssh_run.save_output_as {
            names.push(n.trim().to_string());
          }
          if let Some(n) = &ssh_run.ssh_run.save_stderr_as {
            names.push(n.trim().to_string());
          }
          if let Some(n) = &ssh_run.ssh_run.save_exit_code_as {
            names.push(n.trim().to_string());
          }
          names
        },
        CommandRunner::JsonExtract(je) => vec![je.save_as.trim().to_string()],
        _ => vec![],
      })
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

          for (field, name_opt) in [
            ("save_output_as", local_run.save_output_as.as_ref()),
            ("save_stderr_as", local_run.save_stderr_as.as_ref()),
            ("save_exit_code_as", local_run.save_exit_code_as.as_ref()),
          ] {
            if let Some(output_name) = name_opt {
              let output_name = output_name.trim().to_string();
              if !output_name.is_empty() && !produced_outputs.insert(output_name.clone()) {
                report.push_error(
                  Some(task_name),
                  Some(field),
                  format!("Duplicate saved output name: {}", output_name),
                );
              }
            }
          }
        },
        CommandRunner::JsonExtract(json_extract) => {
          // The source output must have been produced by an earlier command
          if !json_extract.extract_json_from.trim().is_empty()
            && !produced_outputs.contains(json_extract.extract_json_from.trim())
          {
            report.push_error(
              Some(task_name),
              Some("extract_json_from"),
              format!(
                "Output reference must come from an earlier command: {}",
                json_extract.extract_json_from.trim()
              ),
            );
          }
          let save_as = json_extract.save_as.trim().to_string();
          if !save_as.is_empty() && !produced_outputs.insert(save_as.clone()) {
            report.push_error(
              Some(task_name),
              Some("save_as"),
              format!("Duplicate saved output name: {}", save_as),
            );
          }
        },
        CommandRunner::WriteOutput(write_output) => {
          // The source output must have been produced by an earlier command
          if !write_output.write_output.trim().is_empty()
            && !produced_outputs.contains(write_output.write_output.trim())
          {
            report.push_error(
              Some(task_name),
              Some("write_output"),
              format!(
                "Output reference must come from an earlier command: {}",
                write_output.write_output.trim()
              ),
            );
          }
        },
        CommandRunner::SshRun(ssh_run) => {
          for output_name in extract_output_references(&ssh_run.ssh_run.command) {
            if !produced_outputs.contains(&output_name) {
              report.push_error(
                Some(task_name),
                Some("ssh_run.command"),
                format!(
                  "Output reference must come from an earlier command: {}",
                  output_name
                ),
              );
            }
          }

          if let Some(test) = &ssh_run.ssh_run.test {
            for output_name in extract_output_references(test) {
              if !produced_outputs.contains(&output_name) {
                report.push_error(
                  Some(task_name),
                  Some("ssh_run.test"),
                  format!(
                    "Output reference must come from an earlier command: {}",
                    output_name
                  ),
                );
              }
            }
          }

          for (field, name_opt) in [
            ("ssh_run.save_output_as", ssh_run.ssh_run.save_output_as.as_ref()),
            ("ssh_run.save_stderr_as", ssh_run.ssh_run.save_stderr_as.as_ref()),
            (
              "ssh_run.save_exit_code_as",
              ssh_run.ssh_run.save_exit_code_as.as_ref(),
            ),
          ] {
            if let Some(output_name) = name_opt {
              let output_name = output_name.trim().to_string();
              if !output_name.is_empty() && !produced_outputs.insert(output_name.clone()) {
                report.push_error(
                  Some(task_name),
                  Some(field),
                  format!("Duplicate saved output name: {}", output_name),
                );
              }
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

fn validate_secret_field_conflict<T: PartialEq>(
  task_name: Option<&str>,
  field_name: &str,
  legacy: Option<&T>,
  secrets_block: Option<&T>,
  report: &mut ValidationReport,
) {
  if legacy.is_some() && secrets_block.is_some() && legacy != secrets_block {
    report.push_error(
      task_name,
      Some("secrets"),
      format!(
        "Legacy secret field '{}' conflicts with `secrets.{}`",
        field_name, field_name
      ),
    );
  }
}

fn pgp_default_keys_location_applies() -> bool {
  true
}

fn command_uses_task_outputs(command: &CommandRunner) -> bool {
  match command {
    CommandRunner::LocalRun(local_run) => {
      local_run.save_output_as.is_some()
        || local_run.save_stderr_as.is_some()
        || local_run.save_exit_code_as.is_some()
        || contains_output_reference(&local_run.command)
        || local_run
          .test
          .as_ref()
          .is_some_and(|test| contains_output_reference(test))
    },
    CommandRunner::SshRun(ssh_run) => {
      ssh_run.ssh_run.save_output_as.is_some()
        || ssh_run.ssh_run.save_stderr_as.is_some()
        || ssh_run.ssh_run.save_exit_code_as.is_some()
        || contains_output_reference(&ssh_run.ssh_run.command)
        || ssh_run
          .ssh_run
          .test
          .as_ref()
          .is_some_and(|test| contains_output_reference(test))
    },
    CommandRunner::JsonExtract(_) | CommandRunner::WriteOutput(_) => true,
    CommandRunner::CommandRun(command) => contains_output_reference(command),
    CommandRunner::ContainerRun(_) | CommandRunner::ContainerBuild(_) | CommandRunner::TaskRun(_) => false,
  }
}

fn task_uses_dynamic_runtime_inputs(task: &super::TaskArgs) -> bool {
  task.commands.iter().any(command_uses_dynamic_runtime_inputs)
    || task
      .preconditions
      .iter()
      .any(precondition_uses_dynamic_runtime_inputs)
}

fn command_uses_dynamic_runtime_inputs(command: &CommandRunner) -> bool {
  match command {
    CommandRunner::JsonExtract(_) | CommandRunner::WriteOutput(_) => false,
    CommandRunner::CommandRun(command) => contains_dynamic_runtime_fragment(command),
    CommandRunner::LocalRun(local_run) => {
      contains_dynamic_runtime_fragment(&local_run.command)
        || local_run
          .test
          .as_ref()
          .is_some_and(|test| contains_dynamic_runtime_fragment(test))
    },
    CommandRunner::SshRun(ssh_run) => {
      contains_dynamic_runtime_fragment(&ssh_run.ssh_run.host)
        || contains_dynamic_runtime_fragment(&ssh_run.ssh_run.command)
        || ssh_run
          .ssh_run
          .test
          .as_ref()
          .is_some_and(|test| contains_dynamic_runtime_fragment(test))
        || ssh_run
          .ssh_run
          .work_dir
          .as_ref()
          .is_some_and(|work_dir| contains_dynamic_runtime_fragment(work_dir))
        || ssh_run
          .ssh_run
          .identity_file
          .as_ref()
          .is_some_and(|identity_file| contains_dynamic_runtime_fragment(identity_file))
        || ssh_run
          .ssh_run
          .options
          .iter()
          .any(|value| contains_dynamic_runtime_fragment(value))
    },
    CommandRunner::ContainerRun(container_run) => {
      contains_dynamic_runtime_fragment(&container_run.image)
        || container_run
          .container_command
          .iter()
          .any(|value| contains_dynamic_runtime_fragment(value))
        || container_run
          .mounted_paths
          .iter()
          .any(|value| contains_dynamic_runtime_fragment(value))
    },
    CommandRunner::ContainerBuild(container_build) => {
      contains_dynamic_runtime_fragment(&container_build.container_build.image_name)
        || contains_dynamic_runtime_fragment(&container_build.container_build.context)
        || container_build
          .container_build
          .containerfile
          .as_ref()
          .is_some_and(|value| contains_dynamic_runtime_fragment(value))
        || container_build
          .container_build
          .tags
          .as_ref()
          .is_some_and(|values| {
            values
              .iter()
              .any(|value| contains_dynamic_runtime_fragment(value))
          })
        || container_build
          .container_build
          .build_args
          .as_ref()
          .is_some_and(|values| {
            values
              .iter()
              .any(|value| contains_dynamic_runtime_fragment(value))
          })
        || container_build
          .container_build
          .labels
          .as_ref()
          .is_some_and(|values| {
            values
              .iter()
              .any(|value| contains_dynamic_runtime_fragment(value))
          })
    },
    CommandRunner::TaskRun(_) => false,
  }
}

fn precondition_uses_dynamic_runtime_inputs(precondition: &Precondition) -> bool {
  contains_dynamic_runtime_fragment(&precondition.command)
}

fn contains_dynamic_runtime_fragment(value: &str) -> bool {
  value.contains("$(")
    || value.contains('`')
    || value.contains("MK_NOW")
    || value.contains("MK_GIT_REVISION")
    || value.contains("MK_GIT_REMOTE_ORIGIN")
}

fn has_default_containerfile(context_path: &Path) -> bool {
  context_path.join("Dockerfile").is_file() || context_path.join("Containerfile").is_file()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn has_error(report: &ValidationReport, field: &str, message: &str) -> bool {
    report.issues.iter().any(|issue| {
      issue.severity == ValidationSeverity::Error
        && issue.field.as_deref() == Some(field)
        && issue.message == message
    })
  }

  #[test]
  fn test_validate_retrigger_requires_non_interactive_local_run() -> anyhow::Result<()> {
    let yaml = r#"
      tasks:
        dev:
          commands:
            - command: "go run ."
              interactive: true
              retrigger: true
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(report.issues.iter().any(|issue| {
      issue.field.as_deref() == Some("retrigger")
        && issue.message == "retrigger is only supported for non-interactive local commands"
    }));

    Ok(())
  }

  #[test]
  fn test_validate_rejects_gpg_backend_without_gpg_key_id() -> anyhow::Result<()> {
    let yaml = r#"
      secrets:
        backend: gpg
      tasks:
        demo:
          commands:
            - command: echo ready
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(has_error(
      &report,
      "secrets.gpg_key_id",
      "GPG backend requires gpg_key_id"
    ));
    Ok(())
  }

  #[test]
  fn test_validate_rejects_pgp_backend_without_key_name() -> anyhow::Result<()> {
    let yaml = r#"
      secrets:
        backend: built_in_pgp
        keys_location: ./.mk/keys
      tasks:
        demo:
          commands:
            - command: echo ready
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(has_error(
      &report,
      "secrets.key_name",
      "PGP backend requires key_name"
    ));
    Ok(())
  }

  #[test]
  fn test_validate_allows_pgp_backend_without_keys_location_when_default_applies() -> anyhow::Result<()> {
    let yaml = r#"
      secrets:
        backend: built_in_pgp
        key_name: team
      tasks:
        demo:
          commands:
            - command: echo ready
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(!report
      .issues
      .iter()
      .any(|issue| issue.field.as_deref() == Some("secrets.keys_location")));
    Ok(())
  }

  #[test]
  fn test_validate_rejects_gpg_backend_with_pgp_only_settings() -> anyhow::Result<()> {
    let yaml = r#"
      tasks:
        demo:
          secrets:
            backend: gpg
            gpg_key_id: TEAMKEY
            key_name: team
            keys_location: ./.mk/keys
          commands:
            - command: echo ready
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(has_error(
      &report,
      "secrets",
      "GPG backend cannot be combined with PGP-only settings: key_name, keys_location"
    ));
    Ok(())
  }

  #[test]
  fn test_validate_rejects_conflicting_legacy_and_secrets_block_values() -> anyhow::Result<()> {
    let yaml = r#"
      vault_location: ./.mk/legacy-vault
      secrets:
        vault_location: ./.mk/new-vault
      tasks:
        demo:
          commands:
            - command: echo ready
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(has_error(
      &report,
      "secrets",
      "Legacy secret field 'vault_location' conflicts with `secrets.vault_location`"
    ));
    Ok(())
  }

  fn has_warning(report: &ValidationReport, field: &str, message: &str) -> bool {
    report.issues.iter().any(|issue| {
      issue.severity == ValidationSeverity::Warning
        && issue.field.as_deref() == Some(field)
        && issue.message == message
    })
  }

  #[test]
  fn test_validate_warns_on_empty_label_key() -> anyhow::Result<()> {
    let yaml = r#"
      tasks:
        demo:
          commands:
            - command: echo ready
          labels:
            "": present
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(has_warning(&report, "labels", "Label key must not be empty"));
    Ok(())
  }

  #[test]
  fn test_validate_warns_on_empty_label_value() -> anyhow::Result<()> {
    let yaml = r#"
      tasks:
        demo:
          commands:
            - command: echo ready
          labels:
            area: ""
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(has_warning(&report, "labels", "Label 'area' has an empty value"));
    Ok(())
  }

  #[test]
  fn test_validate_warns_on_reserved_mk_prefix() -> anyhow::Result<()> {
    let yaml = r#"
      tasks:
        demo:
          commands:
            - command: echo ready
          labels:
            mk.internal: reserved
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(has_warning(
      &report,
      "labels",
      "Label key 'mk.internal' uses reserved 'mk.' prefix"
    ));
    Ok(())
  }

  #[test]
  fn test_validate_allows_valid_labels() -> anyhow::Result<()> {
    let yaml = r#"
      tasks:
        demo:
          commands:
            - command: echo ready
          labels:
            area: ci
            kind: test
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(!report
      .issues
      .iter()
      .any(|issue| issue.field.as_deref() == Some("labels")));
    Ok(())
  }

  #[test]
  fn test_validate_warns_for_cached_dynamic_command_without_inputs() -> anyhow::Result<()> {
    let yaml = r#"
      tasks:
        build:
          outputs:
            - output.txt
          cache:
            enabled: true
          commands:
            - command: echo $(git rev-parse HEAD) > output.txt
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(has_warning(
      &report,
      "inputs",
      "Cached task contains shell-derived or runtime-derived command inputs but declares no inputs; cache invalidation may miss external changes"
    ));
    Ok(())
  }

  #[test]
  fn test_validate_does_not_warn_for_cached_dynamic_command_with_inputs() -> anyhow::Result<()> {
    let yaml = r#"
      tasks:
        build:
          inputs:
            - .git/HEAD
          outputs:
            - output.txt
          cache:
            enabled: true
          commands:
            - command: echo $(git rev-parse HEAD) > output.txt
    "#;

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    let report = task_root.validate();

    assert!(!has_warning(
      &report,
      "inputs",
      "Cached task contains shell-derived or runtime-derived command inputs but declares no inputs; cache invalidation may miss external changes"
    ));
    Ok(())
  }
}
