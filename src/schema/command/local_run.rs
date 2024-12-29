use std::io::{
  BufRead as _,
  BufReader,
};
use std::process::{
  Command as ProcessCommand,
  Stdio,
};
use std::thread;

use anyhow::Context as _;
use serde::Deserialize;

use crate::defaults::{
  default_shell,
  default_true,
};
use crate::schema::TaskContext;

#[derive(Debug, Deserialize)]
pub struct LocalRun {
  /// The command to run
  pub command: String,

  /// The shell to use to run the command
  #[serde(default = "default_shell")]
  pub shell: String,

  /// The working directory to run the command in
  #[serde(default)]
  pub work_dir: Option<String>,

  /// Ignore errors if the command fails
  #[serde(default)]
  pub ignore_errors: bool,

  /// Show verbose output
  #[serde(default = "default_true")]
  pub verbose: bool,
}

impl LocalRun {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    let stdout = if self.verbose {
      Stdio::piped()
    } else {
      Stdio::null()
    };
    let stderr = if self.verbose {
      Stdio::piped()
    } else {
      Stdio::null()
    };

    let mut cmd = ProcessCommand::new(&self.shell);
    cmd.arg("-c").arg(&self.command).stdout(stdout).stderr(stderr);

    if let Some(work_dir) = &self.work_dir.clone() {
      cmd.current_dir(work_dir);
    }

    // Inject environment variables
    for (key, value) in context.env_vars.iter() {
      cmd.env(key, value);
    }

    let mut cmd = cmd.spawn()?;

    if self.verbose {
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
    if !status.success() && !self.ignore_errors {
      anyhow::bail!("Command failed - {}", self.command);
    }

    Ok(())
  }
}
