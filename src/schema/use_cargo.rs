use hashbrown::HashMap;
use serde::Deserialize;

use super::{
  CommandRunner,
  LocalRun,
  Task,
  TaskArgs,
};

#[derive(Debug, Deserialize)]
pub struct UseCargoArgs {
  /// The working directory to run the command in
  #[serde(default)]
  pub work_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UseCargo {
  Bool(bool),
  UseCargo(Box<UseCargoArgs>),
}

impl UseCargo {
  pub fn capture(&self) -> anyhow::Result<HashMap<String, Task>> {
    match self {
      UseCargo::Bool(true) => self.capture_tasks(),
      UseCargo::UseCargo(args) => args.capture_tasks(),
      _ => Ok(HashMap::new()),
    }
  }

  fn capture_tasks(&self) -> anyhow::Result<HashMap<String, Task>> {
    UseCargoArgs { work_dir: None }.capture_tasks()
  }
}

impl UseCargoArgs {
  pub fn capture_tasks(&self) -> anyhow::Result<HashMap<String, Task>> {
    let cargo_commands = [
      "add",
      "bench",
      "build",
      "check",
      "clean",
      "clippy",
      "doc",
      "fix",
      "fmt",
      "init",
      "install",
      "miri",
      "new",
      "publish",
      "remove",
      "report",
      "run",
      "search",
      "test",
      "uninstall",
      "update",
    ];

    let hm: HashMap<String, Task> = cargo_commands
      .iter()
      .map(|cmd| {
        let command = format!("cargo {}", cmd);
        let task = Task::Task(Box::new(TaskArgs {
          commands: vec![CommandRunner::LocalRun(LocalRun {
            command,
            shell: None,
            test: None,
            work_dir: self.work_dir.clone(),
            interactive: Some(true),
            ignore_errors: None,
            verbose: None,
          })],
          ..Default::default()
        }));
        (cmd.to_string(), task)
      })
      .collect();
    Ok(hm)
  }
}
