use schemars::JsonSchema;
use serde::Deserialize;
use std::process::Command as ProcessCommand;

#[derive(Debug, Default, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
/// Shell command with optional flags.
pub struct ShellArgs {
  /// The shell command to run
  pub command: String,

  /// The flags to pass to the shell command
  pub args: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
/// The shell to use. Either a string name (e.g. "bash") or an object with `command` and optional `args`.
pub enum Shell {
  String(String),
  Shell(Box<ShellArgs>),
}

impl Default for Shell {
  fn default() -> Self {
    Shell::String(default_shell_command().to_string())
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
    let Some(eval_flag) = shell_eval_flag(&command) else {
      return args;
    };

    if args.iter().any(|arg| arg.eq_ignore_ascii_case(eval_flag)) {
      return args;
    }

    let mut args = args;
    args.push(eval_flag.to_string());
    args
  }
}

fn default_shell_command() -> &'static str {
  if cfg!(windows) {
    "cmd"
  } else {
    "sh"
  }
}

fn shell_eval_flag(command: &str) -> Option<&'static str> {
  let shell = command
    .rsplit(['/', '\\'])
    .next()
    .unwrap_or(command)
    .to_ascii_lowercase();

  match shell.as_str() {
    "sh" | "bash" | "zsh" | "fish" => Some("-c"),
    "cmd" | "cmd.exe" => Some("/C"),
    "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe" => Some("-Command"),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::{default_shell_command, Shell, ShellArgs};

  #[test]
  fn shell_default_matches_platform() {
    let shell = Shell::default();
    assert_eq!(shell.cmd(), default_shell_command().to_string());
  }

  #[test]
  fn posix_shell_adds_dash_c() {
    let args = ShellArgs {
      command: "bash".to_string(),
      args: None,
    };

    assert_eq!(args.shell_args(), vec!["-c".to_string()]);
  }

  #[test]
  fn cmd_shell_adds_slash_c() {
    let args = ShellArgs {
      command: "cmd.exe".to_string(),
      args: None,
    };

    assert_eq!(args.shell_args(), vec!["/C".to_string()]);
  }

  #[test]
  fn powershell_adds_command_flag() {
    let args = ShellArgs {
      command: "pwsh".to_string(),
      args: Some(vec!["-NoProfile".to_string()]),
    };

    assert_eq!(
      args.shell_args(),
      vec!["-NoProfile".to_string(), "-Command".to_string()]
    );
  }

  #[test]
  fn existing_eval_flag_is_preserved() {
    let args = ShellArgs {
      command: "cmd".to_string(),
      args: Some(vec!["/C".to_string()]),
    };

    assert_eq!(args.shell_args(), vec!["/C".to_string()]);
  }
}
