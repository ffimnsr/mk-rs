use serde::Deserialize;
use std::process::Command as ProcessCommand;

#[derive(Debug, Default, Deserialize, Clone, PartialEq, Eq)]
pub struct ShellArgs {
  /// The shell command to run
  pub command: String,

  /// The flags to pass to the shell command
  pub args: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Shell {
  String(String),
  Shell(Box<ShellArgs>),
}

impl Default for Shell {
  fn default() -> Self {
    Shell::String("sh".to_string())
  }
}

impl Shell {
  pub fn new() -> anyhow::Result<Self> {
    Ok(Shell::default())
  }

  pub fn new_with_flags(command: &str, args: Vec<String>) -> anyhow::Result<Self> {
    let shell_def = ShellArgs {
      command: command.to_string(),
      args: Some(args),
    };
    Ok(Shell::Shell(Box::new(shell_def)))
  }

  pub fn from_shell(shell: &Shell) -> Self {
    match shell {
      Shell::String(command) => Shell::String(command.to_string()),
      Shell::Shell(args) => Shell::Shell(args.clone()),
    }
  }

  pub fn cmd(&self) -> String {
    match self {
      Shell::String(command) => ShellArgs {
        command: command.to_string(),
        args: None,
      }
      .cmd(),
      Shell::Shell(args) => args.cmd(),
    }
  }

  pub fn args(&self) -> Vec<String> {
    match self {
      Shell::String(command) => ShellArgs {
        command: command.to_string(),
        args: None,
      }
      .shell_args(),
      Shell::Shell(args) => args.shell_args(),
    }
  }

  pub fn proc(&self) -> ProcessCommand {
    let shell = self.cmd();
    let args = self.args();

    let mut cmd = ProcessCommand::new(&shell);
    for arg in args {
      cmd.arg(arg);
    }

    cmd
  }
}

impl From<Shell> for ProcessCommand {
  fn from(shell: Shell) -> Self {
    shell.proc()
  }
}

impl ShellArgs {
  pub fn cmd(&self) -> String {
    self.command.clone()
  }

  pub fn shell_args(&self) -> Vec<String> {
    let command = self.command.clone();
    let args = self.args.clone().unwrap_or_default();
    let posix_shell = ["sh", "bash", "zsh", "fish"];

    // If the shell is not a POSIX shell, we don't need to add the `-c` flag
    // to the command. We can just return the arguments as is.
    if !posix_shell.contains(&command.as_str()) {
      return args;
    }

    // If the shell is a POSIX shell, we need to add the `-c` flag
    // to the command. If it's already present, we don't need to add it.
    if args.iter().any(|arg| arg == "-c") {
      return args;
    }

    // If the `-c` flag is not present, we need to add it
    let mut args = args;
    args.push("-c".to_string());
    args
  }
}
