use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::Context as _;
use hashbrown::HashMap;
use serde::Deserialize;

use crate::defaults::default_node_package_manager;
use crate::file::ToUtf8 as _;

use super::{
  CommandRunner,
  LocalRun,
  Task,
  TaskArgs,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpmPackage {
  /// The name of the package
  pub name: Option<String>,

  /// The version of the package
  pub version: Option<String>,

  /// The path to the package
  pub scripts: Option<HashMap<String, String>>,

  /// The package manager to use
  pub package_manager: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UseNpmArgs {
  /// The package manager to use
  #[serde(default)]
  pub package_manager: Option<String>,

  /// The working directory to run the command in
  #[serde(default)]
  pub work_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UseNpm {
  Bool(bool),
  UseNpm(Box<UseNpmArgs>),
}

impl UseNpm {
  pub fn capture(&self) -> anyhow::Result<HashMap<String, Task>> {
    match self {
      UseNpm::Bool(true) => self.capture_tasks(),
      UseNpm::UseNpm(args) => args.capture_tasks(),
      _ => Ok(HashMap::new()),
    }
  }

  fn capture_tasks(&self) -> anyhow::Result<HashMap<String, Task>> {
    UseNpmArgs {
      package_manager: None,
      work_dir: None,
    }
    .capture_tasks()
  }
}

impl UseNpmArgs {
  pub fn capture_tasks(&self) -> anyhow::Result<HashMap<String, Task>> {
    let path = self
      .work_dir
      .as_ref()
      .map(|work_dir| PathBuf::from(work_dir).join("package.json"))
      .unwrap_or_else(|| PathBuf::from("package.json"));

    if !path.exists() || !path.is_file() {
      return Err(anyhow::anyhow!("package.json does not exist"));
    }

    let file = File::open(&path).context(format!("Failed to open file - {}", path.to_utf8()?))?;
    let reader = BufReader::new(file);

    let package: NpmPackage = serde_json::from_reader(reader)?;
    let package_manager: &str = &self
      .package_manager
      .clone()
      .unwrap_or_else(default_node_package_manager);

    assert!(!package_manager.is_empty());

    let tasks: HashMap<String, Task> = package
      .scripts
      .unwrap_or_default()
      .into_iter()
      .map(|(k, _)| {
        let command = format!("{package_manager} run {k}");
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
        (k, task)
      })
      .collect();
    Ok(tasks)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_use_npm_1() -> anyhow::Result<()> {
    let json = r#"{
      "name": "test",
      "version": "1.0.0",
      "scripts": {
        "build": "echo 'Building'",
        "test": "echo 'Testing'"
      }
    }"#;
    let package = serde_json::from_str::<NpmPackage>(json)?;
    assert_eq!(package.name, Some("test".to_string()));
    assert_eq!(package.version, Some("1.0.0".to_string()));
    assert_eq!(
      package.scripts,
      Some({
        let mut map = HashMap::new();
        map.insert("build".to_string(), "echo 'Building'".to_string());
        map.insert("test".to_string(), "echo 'Testing'".to_string());
        map
      })
    );
    Ok(())
  }

  #[test]
  fn test_use_npm_2() -> anyhow::Result<()> {
    let yaml = "true";

    let use_npm = serde_yaml::from_str::<UseNpm>(yaml)?;
    if let UseNpm::Bool(value) = use_npm {
      assert!(value);
    } else {
      panic!("Invalid value");
    }

    Ok(())
  }

  #[test]
  fn test_use_npm_3() -> anyhow::Result<()> {
    let yaml = "false";

    let use_npm = serde_yaml::from_str::<UseNpm>(yaml)?;
    if let UseNpm::Bool(value) = use_npm {
      assert!(!value);
    } else {
      panic!("Invalid value");
    }

    Ok(())
  }

  #[test]
  fn test_use_npm_4() -> anyhow::Result<()> {
    let yaml = "
      package_manager: npm
    ";

    let use_npm = serde_yaml::from_str::<UseNpm>(yaml)?;
    if let UseNpm::UseNpm(args) = use_npm {
      assert_eq!(args.package_manager, Some("npm".to_string()));
    } else {
      panic!("Invalid value");
    }

    Ok(())
  }

  #[test]
  fn test_use_npm_5() -> anyhow::Result<()> {
    let yaml = "
      package_manager: yarn
      work_dir: /path/to/dir
    ";

    let use_npm = serde_yaml::from_str::<UseNpm>(yaml)?;
    if let UseNpm::UseNpm(args) = use_npm {
      assert_eq!(args.package_manager, Some("yarn".to_string()));
      assert_eq!(args.work_dir, Some("/path/to/dir".to_string()));
    } else {
      panic!("Invalid value");
    }

    Ok(())
  }
}
