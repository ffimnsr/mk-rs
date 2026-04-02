use hashbrown::HashMap;
use serde::Deserialize;

use crate::utils::resolve_path;

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
    self.capture_in_dir(std::path::Path::new("."))
  }

  pub fn capture_in_dir(&self, base_dir: &std::path::Path) -> anyhow::Result<HashMap<String, Task>> {
    match self {
      UseCargo::Bool(true) => self.capture_tasks_in_dir(base_dir),
      UseCargo::UseCargo(args) => args.capture_tasks_in_dir(base_dir),
      _ => Ok(HashMap::new()),
    }
  }

  fn capture_tasks_in_dir(&self, base_dir: &std::path::Path) -> anyhow::Result<HashMap<String, Task>> {
    UseCargoArgs { work_dir: None }.capture_tasks_in_dir(base_dir)
  }
}

impl UseCargoArgs {
  pub fn capture_tasks(&self) -> anyhow::Result<HashMap<String, Task>> {
    self.capture_tasks_in_dir(std::path::Path::new("."))
  }

  pub fn capture_tasks_in_dir(&self, base_dir: &std::path::Path) -> anyhow::Result<HashMap<String, Task>> {
    let resolved_work_dir = self
      .work_dir
      .as_ref()
      .map(|work_dir| resolve_path(base_dir, work_dir));
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
            work_dir: resolved_work_dir
              .as_ref()
              .map(|work_dir| work_dir.to_string_lossy().into_owned()),
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
