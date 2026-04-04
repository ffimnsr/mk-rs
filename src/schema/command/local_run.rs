use std::io::{
  BufRead as _,
  BufReader,
  IsTerminal as _,
};
use std::process::{
  Child,
  ExitStatus,
  Stdio,
};
use std::thread;
#[cfg(unix)]
use std::time::Duration;

use anyhow::Context as _;
#[cfg(unix)]
use console::Term;
use indicatif::ProgressDrawTarget;
use serde::Deserialize;

#[cfg(unix)]
use std::os::fd::AsRawFd as _;
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;

use crate::defaults::{
  default_ignore_errors,
  default_verbose,
};
use crate::handle_output;
#[cfg(unix)]
use crate::schema::ExecutionInterrupted;
use crate::schema::{
  get_output_handler,
  interpolate_template_string,
  Shell,
  TaskContext,
};

#[derive(Debug, Deserialize, Clone)]
pub struct LocalRun {
  /// The command to run
  pub command: String,

  /// The shell to use to run the command
  #[serde(default)]
  pub shell: Option<Shell>,

  /// The test to run before running command
  /// If the test fails, the command will not run
  #[serde(default)]
  pub test: Option<String>,

  /// The working directory to run the command in
  #[serde(default)]
  pub work_dir: Option<String>,

  /// Interactive mode
  /// If true, the command will be interactive accepting user input
  #[serde(default)]
  pub interactive: Option<bool>,

  /// Allow pressing `R` to manually stop and restart a non-interactive command.
  #[serde(default)]
  pub retrigger: Option<bool>,

  /// Ignore errors if the command fails
  #[serde(default)]
  pub ignore_errors: Option<bool>,

  /// Save the command stdout to a task-scoped output name
  #[serde(default)]
  pub save_output_as: Option<String>,

  /// Show verbose output
  #[serde(default)]
  pub verbose: Option<bool>,
}

impl LocalRun {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.command.is_empty());

    let command = interpolate_template_string(&self.command, context)?;
    let interactive = self.interactive_enabled();
    let retrigger = self.retrigger_enabled();
    if interactive && retrigger {
      anyhow::bail!("retrigger is only supported for non-interactive local commands");
    }
    let ignore_errors = self.ignore_errors(context);
    let capture_output = self.save_output_as.is_some();
    // If interactive mode is enabled, we don't need to redirect the output
    // to the parent process. This is because the command will be run in the
    // foreground and the user will be able to see the output.
    let verbose = interactive || self.verbose(context);

    // Skip the command if the test fails
    if self.test(context).is_err() {
      return Ok(());
    }

    if retrigger {
      return self.execute_with_retrigger(context, &command, ignore_errors, capture_output, verbose);
    }

    let (status, captured_stdout) = self
      .spawn_command(context, &command, capture_output, verbose, interactive)?
      .wait_for_completion()?;
    self.finish_execution(context, &command, status, captured_stdout, ignore_errors)
  }

  fn spawn_command(
    &self,
    context: &TaskContext,
    command: &str,
    capture_output: bool,
    verbose: bool,
    interactive: bool,
  ) -> anyhow::Result<SpawnedLocalCommand> {
    let mut cmd = self
      .shell
      .as_ref()
      .map(|shell| shell.proc())
      .unwrap_or_else(|| context.shell().proc());

    cmd.arg(command);

    if capture_output {
      cmd.stdout(Stdio::piped());
      if interactive {
        context.multi.set_draw_target(ProgressDrawTarget::hidden());
        cmd.stdin(Stdio::inherit()).stderr(Stdio::inherit());
      } else {
        cmd.stderr(get_output_handler(verbose));
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

    if let Some(work_dir) = self.resolved_work_dir(context) {
      cmd.current_dir(work_dir);
    }

    #[cfg(unix)]
    if self.retrigger_enabled() && !interactive {
      unsafe {
        cmd.pre_exec(|| {
          if libc::setpgid(0, 0) != 0 {
            return Err(std::io::Error::last_os_error());
          }
          Ok(())
        });
      }
    }

    // Inject environment variables
    for (key, value) in context.env_vars.iter() {
      cmd.env(key, value);
    }

    let mut child = cmd.spawn()?;
    let stdout_handle = if capture_output {
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

    if verbose && !interactive && !capture_output {
      handle_output!(child.stdout, context);
      handle_output!(child.stderr, context);
    } else if verbose && !interactive && capture_output {
      handle_output!(child.stderr, context);
    }

    Ok(SpawnedLocalCommand { child, stdout_handle })
  }

  fn finish_execution(
    &self,
    context: &TaskContext,
    command: &str,
    status: ExitStatus,
    captured_stdout: Option<String>,
    ignore_errors: bool,
  ) -> anyhow::Result<()> {
    if !status.success() && !ignore_errors {
      anyhow::bail!("Command failed - {}", command);
    }

    if status.success() {
      if let (Some(output_name), Some(output_value)) = (&self.save_output_as, captured_stdout) {
        context.insert_task_output(output_name.clone(), output_value)?;
      }
    }

    Ok(())
  }

  fn execute_with_retrigger(
    &self,
    context: &TaskContext,
    command: &str,
    ignore_errors: bool,
    capture_output: bool,
    verbose: bool,
  ) -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() || context.json_events {
      return self.execute_without_retrigger(
        context,
        command,
        ignore_errors,
        capture_output,
        verbose,
        "Manual retrigger requires an attached terminal and is disabled for `--json-events`.",
      );
    }

    #[cfg(not(unix))]
    {
      return self.execute_without_retrigger(
        context,
        command,
        ignore_errors,
        capture_output,
        verbose,
        "Manual retrigger is currently supported on Unix terminals only.",
      );
    }

    #[cfg(unix)]
    {
      let _raw_mode = RawModeGuard::acquire()?;
      let term = Term::stderr();
      let _ = term.write_line("Press R or r to restart the running command.");
      drain_retrigger_input()?;

      loop {
        let spawned = self.spawn_command(context, command, capture_output, verbose, false)?;
        match spawned.wait_for_completion_or_retrigger() {
          Ok(CommandOutcome::Completed {
            status,
            captured_stdout,
          }) => {
            return self.finish_execution(context, command, status, captured_stdout, ignore_errors);
          },
          Ok(CommandOutcome::RestartRequested) => {
            let _ = term.write_line("Restarting command...");
          },
          Ok(CommandOutcome::Interrupted) => {
            return Err(ExecutionInterrupted.into());
          },
          Err(error) => return Err(error),
        }
      }
    }
  }

  fn execute_without_retrigger(
    &self,
    context: &TaskContext,
    command: &str,
    ignore_errors: bool,
    capture_output: bool,
    verbose: bool,
    reason: &str,
  ) -> anyhow::Result<()> {
    if !context.json_events {
      let _ = context.multi.println(reason);
    }
    let (status, captured_stdout) = self
      .spawn_command(context, command, capture_output, verbose, false)?
      .wait_for_completion()?;
    self.finish_execution(context, command, status, captured_stdout, ignore_errors)
  }

  /// Check if the local run task is parallel safe
  /// If the task is interactive or retriggerable, it is not parallel safe
  pub fn is_parallel_safe(&self) -> bool {
    !self.interactive_enabled() && !self.retrigger_enabled()
  }

  pub fn interactive_enabled(&self) -> bool {
    self.interactive.unwrap_or(false)
  }

  pub fn retrigger_enabled(&self) -> bool {
    self.retrigger.unwrap_or(false)
  }

  fn test(&self, context: &TaskContext) -> anyhow::Result<()> {
    let verbose = self.verbose(context);

    let stdout = get_output_handler(verbose);
    let stderr = get_output_handler(verbose);

    if let Some(test) = &self.test {
      let test = interpolate_template_string(test, context)?;
      let mut cmd = self
        .shell
        .as_ref()
        .map(|shell| shell.proc())
        .unwrap_or_else(|| context.shell().proc());
      cmd.arg(&test).stdout(stdout).stderr(stderr);

      if let Some(work_dir) = self.resolved_work_dir(context) {
        cmd.current_dir(work_dir);
      }

      let mut cmd = cmd.spawn()?;
      if verbose {
        handle_output!(cmd.stdout, context);
        handle_output!(cmd.stderr, context);
      }

      let status = cmd.wait()?;

      log::trace!("Test status: {:?}", status.success());
      if !status.success() {
        anyhow::bail!("Command test failed - {}", test);
      }
    }

    Ok(())
  }

  fn ignore_errors(&self, context: &TaskContext) -> bool {
    self
      .ignore_errors
      .or(context.ignore_errors)
      .unwrap_or(default_ignore_errors())
  }

  fn verbose(&self, context: &TaskContext) -> bool {
    self.verbose.or(context.verbose).unwrap_or(default_verbose())
  }

  pub fn resolved_work_dir(&self, context: &TaskContext) -> Option<std::path::PathBuf> {
    self
      .work_dir
      .as_ref()
      .map(|work_dir| context.resolve_from_config(work_dir))
  }
}

struct SpawnedLocalCommand {
  child: Child,
  stdout_handle: Option<thread::JoinHandle<anyhow::Result<String>>>,
}

impl SpawnedLocalCommand {
  fn wait_for_completion(mut self) -> anyhow::Result<(ExitStatus, Option<String>)> {
    let status = self.child.wait()?;
    let captured_stdout = self.join_stdout_handle()?;
    Ok((status, captured_stdout))
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

  #[cfg(unix)]
  fn wait_for_completion_or_retrigger(mut self) -> anyhow::Result<CommandOutcome> {
    loop {
      if let Some(status) = self.child.try_wait()? {
        let captured_stdout = self.join_stdout_handle()?;
        return Ok(CommandOutcome::Completed {
          status,
          captured_stdout,
        });
      }

      match read_control_byte(Duration::from_millis(100))? {
        Some(b'R' | b'r') => {
          self.kill_for_restart()?;
          let _ = self.child.wait()?;
          let _ = self.join_stdout_handle()?;
          drain_retrigger_input()?;
          return Ok(CommandOutcome::RestartRequested);
        },
        Some(3) => {
          self.kill_for_restart()?;
          let _ = self.child.wait()?;
          let _ = self.join_stdout_handle()?;
          drain_retrigger_input()?;
          return Ok(CommandOutcome::Interrupted);
        },
        _ => {},
      }
    }
  }

  #[cfg(unix)]
  fn kill_for_restart(&mut self) -> anyhow::Result<()> {
    let pid = self.child.id() as i32;
    let kill_result = unsafe { libc::killpg(pid, libc::SIGKILL) };
    if kill_result == 0 {
      return Ok(());
    }

    let error = std::io::Error::last_os_error();
    let raw_error = error.raw_os_error();
    if raw_error == Some(libc::ESRCH) || raw_error == Some(libc::EPERM) {
      match self.child.kill() {
        Ok(()) => return Ok(()),
        Err(child_error) if child_error.kind() == std::io::ErrorKind::InvalidInput => return Ok(()),
        Err(child_error) => return Err(child_error.into()),
      }
    }

    Err(error.into())
  }
}

#[cfg(unix)]
enum CommandOutcome {
  Completed {
    status: ExitStatus,
    captured_stdout: Option<String>,
  },
  RestartRequested,
  Interrupted,
}

#[cfg(unix)]
struct RawModeGuard {
  fd: std::os::fd::RawFd,
  original: libc::termios,
}

#[cfg(unix)]
impl RawModeGuard {
  fn acquire() -> anyhow::Result<Self> {
    let fd = std::io::stdin().as_raw_fd();
    let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
    let get_attr_result = unsafe { libc::tcgetattr(fd, original.as_mut_ptr()) };
    if get_attr_result != 0 {
      return Err(std::io::Error::last_os_error().into());
    }

    let original = unsafe { original.assume_init() };
    let mut raw = original;
    raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
    raw.c_cc[libc::VMIN] = 0;
    raw.c_cc[libc::VTIME] = 0;

    let set_attr_result = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) };
    if set_attr_result != 0 {
      return Err(std::io::Error::last_os_error().into());
    }

    Ok(Self { fd, original })
  }
}

#[cfg(unix)]
impl Drop for RawModeGuard {
  fn drop(&mut self) {
    let _ = unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.original) };
  }
}

#[cfg(unix)]
fn read_control_byte(timeout: Duration) -> anyhow::Result<Option<u8>> {
  let fd = std::io::stdin().as_raw_fd();
  let timeout_ms = timeout.as_millis().min(libc::c_int::MAX as u128) as libc::c_int;
  let mut poll_fd = libc::pollfd {
    fd,
    events: libc::POLLIN,
    revents: 0,
  };

  let poll_result = unsafe { libc::poll(&mut poll_fd, 1, timeout_ms) };
  if poll_result < 0 {
    return Err(std::io::Error::last_os_error().into());
  }
  if poll_result == 0 || poll_fd.revents & libc::POLLIN == 0 {
    return Ok(None);
  }

  let mut byte = [0_u8; 1];
  let read_result = unsafe { libc::read(fd, byte.as_mut_ptr().cast(), 1) };
  if read_result < 0 {
    return Err(std::io::Error::last_os_error().into());
  }
  if read_result == 0 {
    return Ok(None);
  }

  Ok(Some(byte[0]))
}

#[cfg(unix)]
fn drain_retrigger_input() -> anyhow::Result<()> {
  while read_control_byte(Duration::ZERO)?.is_some() {}
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_local_run_1() -> anyhow::Result<()> {
    {
      let yaml = "
        command: echo 'Hello, World!'
        ignore_errors: false
        verbose: false
      ";
      let local_run = serde_yaml::from_str::<LocalRun>(yaml)?;

      assert_eq!(local_run.command, "echo 'Hello, World!'");
      assert_eq!(local_run.work_dir, None);
      assert_eq!(local_run.ignore_errors, Some(false));
      assert_eq!(local_run.verbose, Some(false));
      assert_eq!(local_run.retrigger, None);
      assert_eq!(local_run.save_output_as, None);

      Ok(())
    }
  }

  #[test]
  fn test_local_run_2() -> anyhow::Result<()> {
    {
      let yaml = "
        command: echo 'Hello, World!'
        test: test $(uname) = 'Linux'
        ignore_errors: false
        verbose: false
      ";
      let local_run = serde_yaml::from_str::<LocalRun>(yaml)?;

      assert_eq!(local_run.command, "echo 'Hello, World!'");
      assert_eq!(local_run.test, Some("test $(uname) = 'Linux'".to_string()));
      assert_eq!(local_run.work_dir, None);
      assert_eq!(local_run.ignore_errors, Some(false));
      assert_eq!(local_run.verbose, Some(false));
      assert_eq!(local_run.save_output_as, None);

      Ok(())
    }
  }

  #[test]
  fn test_local_run_3() -> anyhow::Result<()> {
    {
      let yaml = "
        command: echo 'Hello, World!'
        test: test $(uname) = 'Linux'
        shell: bash
        ignore_errors: false
        verbose: false
        interactive: true
      ";
      let local_run = serde_yaml::from_str::<LocalRun>(yaml)?;

      assert_eq!(local_run.command, "echo 'Hello, World!'");
      assert_eq!(local_run.test, Some("test $(uname) = 'Linux'".to_string()));
      assert_eq!(local_run.shell, Some(Shell::String("bash".to_string())));
      assert_eq!(local_run.work_dir, None);
      assert_eq!(local_run.ignore_errors, Some(false));
      assert_eq!(local_run.verbose, Some(false));
      assert_eq!(local_run.interactive, Some(true));
      assert_eq!(local_run.retrigger, None);
      assert_eq!(local_run.save_output_as, None);

      Ok(())
    }
  }

  #[test]
  fn test_local_run_4() -> anyhow::Result<()> {
    let yaml = "
      command: go run .
      retrigger: true
    ";
    let local_run = serde_yaml::from_str::<LocalRun>(yaml)?;

    assert_eq!(local_run.command, "go run .");
    assert_eq!(local_run.retrigger, Some(true));
    assert!(!local_run.interactive_enabled());
    assert!(!local_run.is_parallel_safe());

    Ok(())
  }

  #[test]
  fn test_local_run_5_rejects_interactive_retrigger_combo_at_execution() {
    let yaml = "
      command: cat
      interactive: true
      retrigger: true
    ";
    let local_run = serde_yaml::from_str::<LocalRun>(yaml).expect("valid local run");
    let context = TaskContext::empty();

    let error = local_run
      .execute(&context)
      .expect_err("expected execution to fail");
    assert!(error
      .to_string()
      .contains("retrigger is only supported for non-interactive local commands"));
  }
}
