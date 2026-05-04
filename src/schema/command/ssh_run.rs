use std::io::{BufRead as _, BufReader};
use std::process::{Child, Command as ProcessCommand, ExitStatus, Stdio};
use std::thread;

use anyhow::Context as _;
use indicatif::ProgressDrawTarget;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::defaults::{default_ignore_errors, default_verbose};
use crate::handle_output;
use crate::schema::{get_output_handler, interpolate_template_string, Shell, TaskContext};

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub struct SshRunArgs {
  /// SSH target host name or alias
  pub host: String,

  /// The command to run on the remote host
  pub command: String,

  /// The user to connect as
  #[serde(default)]
  pub user: Option<String>,

  /// The TCP port to connect to
  #[serde(default)]
  pub port: Option<u16>,

  /// Identity file to pass to ssh
  #[serde(default)]
  pub identity_file: Option<String>,

  /// Extra `ssh -o` options
  #[serde(default)]
  pub options: Vec<String>,

  /// Remote shell used to evaluate the command
  #[serde(default)]
  pub shell: Option<Shell>,

  /// The test to run before running command
  /// If the test fails, the command will not run
  #[serde(default)]
  pub test: Option<String>,

  /// Remote working directory to switch to before command execution
  #[serde(default)]
  pub work_dir: Option<String>,

  /// Interactive mode
  #[serde(default)]
  pub interactive: Option<bool>,

  /// Ignore errors if the command fails
  #[serde(default)]
  pub ignore_errors: Option<bool>,

  /// Save the command stdout to a task-scoped output name
  #[serde(default)]
  pub save_output_as: Option<String>,

  /// Save the command stderr to a task-scoped output name
  #[serde(default)]
  pub save_stderr_as: Option<String>,

  /// Save the command exit code to a task-scoped output name
  #[serde(default)]
  pub save_exit_code_as: Option<String>,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub struct SshRun {
  /// SSH-backed remote command execution
  pub ssh_run: SshRunArgs,

  /// Show verbose output
  #[serde(default)]
  pub verbose: Option<bool>,
}

impl SshRun {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.ssh_run.host.is_empty());
    assert!(!self.ssh_run.command.is_empty());

    let interactive = self.interactive_enabled();
    let ignore_errors = self.ignore_errors(context);
    let capture_stdout = self.ssh_run.save_output_as.is_some();
    let capture_stderr = self.ssh_run.save_stderr_as.is_some();
    let capture_exit_code = self.ssh_run.save_exit_code_as.is_some();
    let verbose = interactive || self.verbose(context);

    if self.test(context).is_err() {
      return Ok(());
    }

    let command = interpolate_template_string(&self.ssh_run.command, context)?;
    let remote_command = self.remote_command(context, &command)?;
    let result = self
      .spawn_command(
        context,
        &remote_command,
        capture_stdout,
        capture_stderr,
        verbose,
        interactive,
      )?
      .wait_for_completion()?;

    self.finish_execution(
      context,
      &command,
      result,
      ExecutionOptions {
        capture_exit_code,
        ignore_errors,
      },
    )
  }

  fn spawn_command(
    &self,
    context: &TaskContext,
    remote_command: &str,
    capture_stdout: bool,
    capture_stderr: bool,
    verbose: bool,
    interactive: bool,
  ) -> anyhow::Result<SpawnedSshCommand> {
    let mut cmd = self.base_ssh_command(context, interactive)?;
    cmd.arg(remote_command);

    if capture_stdout {
      cmd.stdout(Stdio::piped());
      if interactive {
        context.multi.set_draw_target(ProgressDrawTarget::hidden());
        cmd.stdin(Stdio::inherit());
        if capture_stderr {
          cmd.stderr(Stdio::piped());
        } else {
          cmd.stderr(Stdio::inherit());
        }
      } else if capture_stderr {
        cmd.stderr(Stdio::piped());
      } else {
        cmd.stderr(get_output_handler(verbose));
      }
    } else if capture_stderr {
      cmd.stderr(Stdio::piped());
      if interactive {
        context.multi.set_draw_target(ProgressDrawTarget::hidden());
        cmd.stdin(Stdio::inherit()).stdout(Stdio::inherit());
      } else {
        cmd.stdout(get_output_handler(verbose));
      }
    } else if verbose {
      if interactive {
        context.multi.set_draw_target(ProgressDrawTarget::hidden());
        cmd
          .stdin(Stdio::inherit())
          .stdout(Stdio::inherit())
          .stderr(Stdio::inherit());
      } else {
        let stdout = get_output_handler(verbose);
        let stderr = get_output_handler(verbose);
        cmd.stdout(stdout).stderr(stderr);
      }
    }

    for (key, value) in context.env_vars.iter() {
      cmd.env(key, value);
    }

    let mut child = cmd.spawn()?;
    let stdout_handle = if capture_stdout {
      let stdout = child.stdout.take().context("Failed to open stdout")?;
      let multi = context.multi.clone();
      Some(thread::spawn(move || -> anyhow::Result<String> {
        let reader = BufReader::new(stdout);
        let mut output = String::new();
        for line in reader.lines() {
          let line = line?;
          if verbose {
            let _ = multi.println(line.clone());
          }
          output.push_str(&line);
          output.push('\n');
        }
        Ok(output.trim_end_matches(['\r', '\n']).to_string())
      }))
    } else {
      None
    };

    let stderr_handle = if capture_stderr {
      let stderr = child.stderr.take().context("Failed to open stderr")?;
      let multi = context.multi.clone();
      Some(thread::spawn(move || -> anyhow::Result<String> {
        let reader = BufReader::new(stderr);
        let mut output = String::new();
        for line in reader.lines() {
          let line = line?;
          if verbose {
            let _ = multi.println(line.clone());
          }
          output.push_str(&line);
          output.push('\n');
        }
        Ok(output.trim_end_matches(['\r', '\n']).to_string())
      }))
    } else {
      None
    };

    if verbose && !interactive && !capture_stdout {
      handle_output!(child.stdout, context);
    }
    if verbose && !interactive && !capture_stderr {
      handle_output!(child.stderr, context);
    }

    Ok(SpawnedSshCommand {
      child,
      stdout_handle,
      stderr_handle,
    })
  }

  fn finish_execution(
    &self,
    context: &TaskContext,
    command: &str,
    result: ExecutionResult,
    options: ExecutionOptions,
  ) -> anyhow::Result<()> {
    if options.capture_exit_code {
      if let Some(exit_code_name) = &self.ssh_run.save_exit_code_as {
        let code = result.status.code().unwrap_or(-1).to_string();
        context.insert_task_output(exit_code_name.clone(), code)?;
      }
    }

    if !result.status.success() && !options.ignore_errors {
      anyhow::bail!("SSH command failed - {}", command);
    }

    if result.status.success() {
      if let (Some(output_name), Some(output_value)) = (&self.ssh_run.save_output_as, result.captured_stdout)
      {
        context.insert_task_output(output_name.clone(), output_value)?;
      }
    }

    if result.status.success() || options.ignore_errors {
      if let (Some(stderr_name), Some(stderr_value)) = (&self.ssh_run.save_stderr_as, result.captured_stderr)
      {
        context.insert_task_output(stderr_name.clone(), stderr_value)?;
      }
    }

    Ok(())
  }

  pub fn interactive_enabled(&self) -> bool {
    self.ssh_run.interactive.unwrap_or(false)
  }

  fn test(&self, context: &TaskContext) -> anyhow::Result<()> {
    let Some(test) = &self.ssh_run.test else {
      return Ok(());
    };

    let test = interpolate_template_string(test, context)?;
    let remote_command = self.remote_command(context, &test)?;
    let verbose = self.verbose(context);

    let stdout = get_output_handler(verbose);
    let stderr = get_output_handler(verbose);

    let mut cmd = self.base_ssh_command(context, false)?;
    cmd.arg(&remote_command).stdout(stdout).stderr(stderr);

    let mut child = cmd.spawn()?;
    if verbose {
      handle_output!(child.stdout, context);
      handle_output!(child.stderr, context);
    }

    let status = child.wait()?;
    if !status.success() {
      anyhow::bail!("SSH command test failed - {}", test);
    }

    Ok(())
  }

  fn base_ssh_command(&self, context: &TaskContext, interactive: bool) -> anyhow::Result<ProcessCommand> {
    let mut cmd = ProcessCommand::new("ssh");

    if interactive {
      cmd.arg("-tt");
    } else {
      cmd.arg("-T");
    }

    if let Some(port) = self.ssh_run.port {
      cmd.arg("-p").arg(port.to_string());
    }

    if let Some(identity_file) = &self.ssh_run.identity_file {
      let identity_file = interpolate_template_string(identity_file, context)?;
      cmd.arg("-i").arg(identity_file);
    }

    for option in &self.ssh_run.options {
      let option = interpolate_template_string(option, context)?;
      cmd.arg("-o").arg(option);
    }

    cmd.arg(self.destination());
    Ok(cmd)
  }

  fn destination(&self) -> String {
    match &self.ssh_run.user {
      Some(user) => format!("{}@{}", user, self.ssh_run.host),
      None => self.ssh_run.host.clone(),
    }
  }

  fn remote_command(&self, context: &TaskContext, command: &str) -> anyhow::Result<String> {
    let script = match &self.ssh_run.work_dir {
      Some(work_dir) => {
        let work_dir = interpolate_template_string(work_dir, context)?;
        format!("cd {} && {}", quote_posix(&work_dir), command)
      },
      None => command.to_string(),
    };

    let shell = self
      .ssh_run
      .shell
      .clone()
      .unwrap_or_else(|| Shell::String("sh".to_string()));

    let mut parts = vec![shell.cmd()];
    parts.extend(shell.args());
    parts.push(script);

    Ok(
      parts
        .iter()
        .map(|part| quote_posix(part))
        .collect::<Vec<_>>()
        .join(" "),
    )
  }

  fn ignore_errors(&self, context: &TaskContext) -> bool {
    self
      .ssh_run
      .ignore_errors
      .or(context.ignore_errors)
      .unwrap_or(default_ignore_errors())
  }

  fn verbose(&self, context: &TaskContext) -> bool {
    self.verbose.or(context.verbose).unwrap_or(default_verbose())
  }
}

struct SpawnedSshCommand {
  child: Child,
  stdout_handle: Option<thread::JoinHandle<anyhow::Result<String>>>,
  stderr_handle: Option<thread::JoinHandle<anyhow::Result<String>>>,
}

impl SpawnedSshCommand {
  fn wait_for_completion(mut self) -> anyhow::Result<ExecutionResult> {
    Ok(ExecutionResult {
      status: self.child.wait()?,
      captured_stdout: self.join_stdout_handle()?,
      captured_stderr: self.join_stderr_handle()?,
    })
  }

  fn join_stdout_handle(&mut self) -> anyhow::Result<Option<String>> {
    self
      .stdout_handle
      .take()
      .map(|handle| {
        handle
          .join()
          .map_err(|_| anyhow::anyhow!("Failed to join stdout capture thread"))?
      })
      .transpose()
  }

  fn join_stderr_handle(&mut self) -> anyhow::Result<Option<String>> {
    self
      .stderr_handle
      .take()
      .map(|handle| {
        handle
          .join()
          .map_err(|_| anyhow::anyhow!("Failed to join stderr capture thread"))?
      })
      .transpose()
  }
}

struct ExecutionOptions {
  capture_exit_code: bool,
  ignore_errors: bool,
}

struct ExecutionResult {
  status: ExitStatus,
  captured_stdout: Option<String>,
  captured_stderr: Option<String>,
}

fn quote_posix(value: &str) -> String {
  if value.is_empty() {
    return "''".to_string();
  }

  let escaped = value.replace('\'', "'\"'\"'");
  format!("'{}'", escaped)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_ssh_run_1() -> anyhow::Result<()> {
    let yaml = "
      ssh_run:
        host: buildbox
        command: echo 'Hello, World!'
    ";
    let ssh_run = serde_yaml::from_str::<SshRun>(yaml)?;

    assert_eq!(ssh_run.ssh_run.host, "buildbox");
    assert_eq!(ssh_run.ssh_run.command, "echo 'Hello, World!'");
    assert_eq!(ssh_run.ssh_run.user, None);
    assert_eq!(ssh_run.verbose, None);

    Ok(())
  }

  #[test]
  fn test_remote_command_wraps_shell_and_work_dir() -> anyhow::Result<()> {
    let yaml = "
      ssh_run:
        host: buildbox
        command: cargo test
        work_dir: /tmp/repo
        shell:
          command: bash
          args:
            - -l
    ";
    let ssh_run = serde_yaml::from_str::<SshRun>(yaml)?;
    let context = TaskContext::empty();
    let command = ssh_run.remote_command(&context, "cargo test")?;

    assert!(command.starts_with("'bash' '-l' '-c' "));
    assert!(command.contains("/tmp/repo"));
    assert!(command.contains("cargo test"));

    Ok(())
  }

  #[test]
  fn test_quote_posix_escapes_single_quote() {
    assert_eq!(quote_posix("a'b"), "'a'\"'\"'b'");
  }
}
