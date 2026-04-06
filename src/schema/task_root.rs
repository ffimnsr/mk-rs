use anyhow::Context;
use hashbrown::HashMap;
use mlua::{
  Lua,
  LuaSerdeExt,
};
use serde::Deserialize;

use std::fs::File;
use std::io::{
  BufReader,
  Read as _,
};
use std::path::{
  Path,
  PathBuf,
};

use super::{
  ContainerRuntime,
  Include,
  Task,
  UseCargo,
  UseNpm,
};
use crate::file::ToUtf8 as _;
use crate::utils::{
  deserialize_environment,
  resolve_path,
};

const MK_COMMANDS: [&str; 10] = [
  "run",
  "list",
  "completion",
  "secrets",
  "help",
  "init",
  "update",
  "validate",
  "plan",
  "clean-cache",
];

/// This struct represents the root of the task schema. It contains all the tasks
/// that can be executed.
#[derive(Debug, Default, Deserialize)]
pub struct TaskRoot {
  /// The tasks that can be executed
  pub tasks: HashMap<String, Task>,

  /// The environment variables to set before running any task
  #[serde(default, deserialize_with = "deserialize_environment")]
  pub environment: HashMap<String, String>,

  /// The environment files to load before running any task
  #[serde(default)]
  pub env_file: Vec<String>,

  /// Secret paths to load as dotenv-style environment entries before running any task
  #[serde(default)]
  pub secrets_path: Vec<String>,

  /// The path to the secret vault
  #[serde(default)]
  pub vault_location: Option<String>,

  /// The path to the private keys used for secret decryption
  #[serde(default)]
  pub keys_location: Option<String>,

  /// The key name to use for secret decryption
  #[serde(default)]
  pub key_name: Option<String>,

  /// The GPG key ID or fingerprint to use for secret encryption/decryption via the system gpg binary.
  /// When set, mk delegates all vault crypto operations to `gpg` instead of the built-in PGP engine,
  /// enabling hardware keys (e.g. YubiKey) and passphrase-protected keys.
  #[serde(default)]
  pub gpg_key_id: Option<String>,

  /// This allows mk to use npm scripts as tasks
  #[serde(default)]
  pub use_npm: Option<UseNpm>,

  /// This allows mk to use cargo commands as tasks
  #[serde(default)]
  pub use_cargo: Option<UseCargo>,

  /// Default container runtime to use for container commands
  #[serde(default)]
  pub container_runtime: Option<ContainerRuntime>,

  /// Includes additional files to be merged into the current file
  #[serde(default)]
  pub include: Option<Vec<Include>>,

  /// Extend another root task file
  #[serde(default)]
  pub extends: Option<String>,

  /// Absolute path to the config file used to load this root
  #[serde(skip)]
  pub source_path: Option<PathBuf>,
}

impl TaskRoot {
  pub fn from_file(file: &str) -> anyhow::Result<Self> {
    Self::from_file_with_stack(file, &mut Vec::new())
  }

  fn from_file_with_stack(file: &str, stack: &mut Vec<PathBuf>) -> anyhow::Result<Self> {
    let file_path = normalize_task_file_path(file)?;

    if let Some(index) = stack.iter().position(|path| path == &file_path) {
      let mut cycle = stack[index..]
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
      cycle.push(file_path.to_string_lossy().into_owned());
      anyhow::bail!("Circular extends detected: {}", cycle.join(" -> "));
    }

    stack.push(file_path.clone());
    let result = load_task_root(&file_path, stack);
    stack.pop();
    result
  }

  pub fn from_hashmap(tasks: HashMap<String, Task>) -> Self {
    Self {
      tasks,
      environment: HashMap::new(),
      env_file: Vec::new(),
      secrets_path: Vec::new(),
      vault_location: None,
      keys_location: None,
      key_name: None,
      gpg_key_id: None,
      use_npm: None,
      use_cargo: None,
      container_runtime: None,
      include: None,
      extends: None,
      source_path: None,
    }
  }

  pub fn config_base_dir(&self) -> PathBuf {
    self
      .source_path
      .as_ref()
      .and_then(|path| path.parent().map(Path::to_path_buf))
      .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
  }

  pub fn cache_base_dir(&self) -> PathBuf {
    self.config_base_dir()
  }

  pub fn resolve_from_config(&self, value: &str) -> PathBuf {
    resolve_path(&self.config_base_dir(), value)
  }
}

fn normalize_task_file_path(file: &str) -> anyhow::Result<PathBuf> {
  let file_path = Path::new(file);
  if file_path.is_absolute() {
    Ok(file_path.to_path_buf())
  } else {
    Ok(std::env::current_dir()?.join(file_path))
  }
}

fn load_task_root(file_path: &Path, stack: &mut Vec<PathBuf>) -> anyhow::Result<TaskRoot> {
  let file_extension = file_path
    .extension()
    .and_then(|ext| ext.to_str())
    .context("Failed to get file extension")?;

  let mut root = match file_extension {
    "yaml" | "yml" => load_yaml_file(file_path, stack),
    "lua" => load_lua_file(file_path, stack),
    "json" => load_json_file(file_path, stack),
    "toml" => load_toml_file(file_path, stack),
    "json5" => anyhow::bail!("JSON5 files are not supported yet. Use YAML, TOML, JSON, or Lua instead."),
    "makefile" | "mk" => anyhow::bail!("Makefiles are not supported. Use a tasks.yaml file instead."),
    _ => anyhow::bail!(
      "Unsupported config file extension '{}'. Supported formats: yaml, yml, toml, json, lua.",
      file_extension
    ),
  }?;

  if root.include.is_some() {
    anyhow::bail!("`include` is no longer supported. Use `extends` instead.");
  }

  root.source_path = Some(file_path.to_path_buf());
  process_task_sources(&mut root)?;

  Ok(root)
}

fn load_yaml_file(file: &Path, stack: &mut Vec<PathBuf>) -> anyhow::Result<TaskRoot> {
  let file_handle = File::open(file).with_context(|| {
    format!(
      "Failed to open file - {}",
      file.to_utf8().unwrap_or("<non-utf8-path>")
    )
  })?;
  let reader = BufReader::new(file_handle);

  // Deserialize the YAML file into a serde_yaml::Value to be able to merge
  // anchors and aliases
  let mut value: serde_yaml::Value = serde_yaml::from_reader(reader)?;
  value.apply_merge()?;

  // Deserialize the serde_yaml::Value into a TaskRoot
  let root: TaskRoot = serde_yaml::from_value(value)?;
  apply_extends(file, stack, root)
}

fn load_toml_file(file: &Path, stack: &mut Vec<PathBuf>) -> anyhow::Result<TaskRoot> {
  let mut file_handle = File::open(file).with_context(|| {
    format!(
      "Failed to open file - {}",
      file.to_utf8().unwrap_or("<non-utf8-path>")
    )
  })?;
  let mut contents = String::new();
  file_handle.read_to_string(&mut contents)?;

  // Deserialize the TOML file into a TaskRoot
  let root: TaskRoot = toml::from_str(&contents)?;
  apply_extends(file, stack, root)
}

fn load_json_file(file: &Path, stack: &mut Vec<PathBuf>) -> anyhow::Result<TaskRoot> {
  let file_handle = File::open(file).with_context(|| {
    format!(
      "Failed to open file - {}",
      file.to_utf8().unwrap_or("<non-utf8-path>")
    )
  })?;
  let reader = BufReader::new(file_handle);

  // Deserialize the JSON file into a TaskRoot
  let root: TaskRoot = serde_json::from_reader(reader)?;
  apply_extends(file, stack, root)
}

fn load_lua_file(file: &Path, stack: &mut Vec<PathBuf>) -> anyhow::Result<TaskRoot> {
  let mut file_handle = File::open(file).with_context(|| {
    format!(
      "Failed to open file - {}",
      file.to_utf8().unwrap_or("<non-utf8-path>")
    )
  })?;
  let mut contents = String::new();
  file_handle.read_to_string(&mut contents)?;

  // Deserialize the Lua value into a TaskRoot
  let root: TaskRoot = get_lua_table(&contents)?;
  apply_extends(file, stack, root)
}

fn process_task_sources(root: &mut TaskRoot) -> anyhow::Result<()> {
  root.tasks = rename_tasks(
    std::mem::take(&mut root.tasks),
    "task",
    &MK_COMMANDS,
    &HashMap::new(),
  );

  if let Some(npm) = &root.use_npm {
    let npm_tasks = npm.capture_in_dir(&root.config_base_dir())?;
    let renamed_npm_tasks = rename_tasks(npm_tasks, "npm", &MK_COMMANDS, &root.tasks);
    root.tasks.extend(renamed_npm_tasks);
  }

  if let Some(cargo) = &root.use_cargo {
    let cargo_tasks = cargo.capture_in_dir(&root.config_base_dir())?;
    let renamed_cargo_tasks = rename_tasks(cargo_tasks, "cargo", &MK_COMMANDS, &root.tasks);
    root.tasks.extend(renamed_cargo_tasks);
  }

  Ok(())
}

fn apply_extends(file: &Path, stack: &mut Vec<PathBuf>, mut root: TaskRoot) -> anyhow::Result<TaskRoot> {
  let Some(parent) = root.extends.clone() else {
    return Ok(root);
  };

  let parent_path = file.parent().unwrap_or_else(|| Path::new(".")).join(parent);
  let mut base = TaskRoot::from_file_with_stack(parent_path.to_string_lossy().as_ref(), stack)?;

  base.tasks.extend(root.tasks.drain());
  base.environment.extend(root.environment.drain());
  base.env_file.extend(root.env_file);
  base.secrets_path.extend(root.secrets_path);
  base.vault_location = root.vault_location.or(base.vault_location);
  base.keys_location = root.keys_location.or(base.keys_location);
  base.key_name = root.key_name.or(base.key_name);
  base.gpg_key_id = root.gpg_key_id.or(base.gpg_key_id);
  base.use_npm = root.use_npm.or(base.use_npm);
  base.use_cargo = root.use_cargo.or(base.use_cargo);
  base.container_runtime = root.container_runtime.or(base.container_runtime);
  base.include = root.include.or(base.include);
  base.extends = None;
  base.source_path = root.source_path.or(base.source_path);

  Ok(base)
}

fn get_lua_table(contents: &str) -> anyhow::Result<TaskRoot> {
  // Create a new Lua instance
  let lua = Lua::new();

  // Load the Lua file and evaluate it
  let value = lua.load(contents).eval()?;

  // Deserialize the Lua value into a TaskRoot
  let root = lua.from_value(value)?;

  Ok(root)
}

fn rename_tasks(
  tasks: HashMap<String, Task>,
  prefix: &str,
  mk_commands: &[&str],
  existing_tasks: &HashMap<String, Task>,
) -> HashMap<String, Task> {
  let mut new_tasks = HashMap::new();
  for (task_name, task) in tasks.into_iter() {
    let new_task_name =
      if mk_commands.contains(&task_name.as_str()) || existing_tasks.contains_key(&task_name) {
        format!("{}_{}", prefix, task_name)
      } else {
        task_name
      };

    new_tasks.insert(new_task_name, task);
  }
  new_tasks
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::schema::{
    CommandRunner,
    TaskDependency,
  };
  use assert_fs::TempDir;

  #[test]
  fn test_task_root_1() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        task1:
          commands:
            - command: echo \"Hello, World 1!\"
              ignore_errors: false
              verbose: false
          depends_on:
            - name: task2
          description: 'This is a task'
          labels: {}
          environment:
            FOO: bar
          env_file:
            - test.env
        task2:
          commands:
            - command: echo \"Hello, World 2!\"
              ignore_errors: false
              verbose: false
          depends_on:
            - name: task1
          description: 'This is a task'
          labels: {}
          environment: {}
        task3:
          commands:
            - command: echo \"Hello, World 3!\"
              ignore_errors: false
              verbose: false
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;

    assert_eq!(task_root.tasks.len(), 3);

    if let Task::Task(task) = &task_root.tasks["task1"] {
      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo \"Hello, World 1!\"");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.ignore_errors, Some(false));
        assert_eq!(local_run.verbose, Some(false));
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      if let TaskDependency::TaskDependency(args) = &task.depends_on[0] {
        assert_eq!(args.name, "task2");
      } else {
        panic!("Expected TaskDependency::TaskDependency");
      }
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "This is a task");
      assert_eq!(task.environment.len(), 1);
      assert_eq!(task.env_file.len(), 1);
    } else {
      panic!("Expected Task::Task");
    }

    if let Task::Task(task) = &task_root.tasks["task2"] {
      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo \"Hello, World 2!\"");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.ignore_errors, Some(false));
        assert_eq!(local_run.verbose, Some(false));
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      if let TaskDependency::TaskDependency(args) = &task.depends_on[0] {
        assert_eq!(args.name, "task1");
      } else {
        panic!("Expected TaskDependency::TaskDependency");
      }
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "This is a task");
      assert_eq!(task.environment.len(), 0);
      assert_eq!(task.env_file.len(), 0);
    } else {
      panic!("Expected Task::Task");
    }

    if let Task::Task(task) = &task_root.tasks["task3"] {
      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo \"Hello, World 3!\"");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.ignore_errors, Some(false));
        assert_eq!(local_run.verbose, Some(false));
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      assert_eq!(task.depends_on.len(), 0);
      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description.len(), 0);
      assert_eq!(task.environment.len(), 0);
      assert_eq!(task.env_file.len(), 0);
    } else {
      panic!("Expected Task::Task");
    }

    Ok(())
  }

  #[test]
  fn test_task_root_2() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        task1:
          commands:
            - command: echo \"Hello, World 1!\"
        task2:
          commands:
            - echo \"Hello, World 2!\"
        task3: echo \"Hello, World 3!\"
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;

    assert_eq!(task_root.tasks.len(), 3);

    if let Task::Task(task) = &task_root.tasks["task1"] {
      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "echo \"Hello, World 1!\"");
        assert_eq!(local_run.work_dir, None);
        assert_eq!(local_run.ignore_errors, None);
        assert_eq!(local_run.verbose, None);
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }

      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "");
      assert_eq!(task.environment.len(), 0);
      assert_eq!(task.env_file.len(), 0);
    } else {
      panic!("Expected Task::Task");
    }

    if let Task::Task(task) = &task_root.tasks["task2"] {
      if let CommandRunner::CommandRun(command) = &task.commands[0] {
        assert_eq!(command, "echo \"Hello, World 2!\"");
      } else {
        panic!("Expected CommandRunner::CommandRun");
      }

      assert_eq!(task.labels.len(), 0);
      assert_eq!(task.description, "");
      assert_eq!(task.environment.len(), 0);
      assert_eq!(task.env_file.len(), 0);
    } else {
      panic!("Expected Task::Task");
    }

    if let Task::String(command) = &task_root.tasks["task3"] {
      assert_eq!(command, "echo \"Hello, World 3!\"");
    } else {
      panic!("Expected Task::String");
    }

    Ok(())
  }

  #[test]
  fn test_task_root_secrets_config() -> anyhow::Result<()> {
    let yaml = "
      vault_location: ./.mk/vault
      keys_location: ./.mk/keys
      key_name: team
      secrets_path:
        - app/common
      tasks:
        demo:
          commands:
            - command: echo ready
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;

    assert_eq!(task_root.secrets_path, vec!["app/common"]);
    assert_eq!(task_root.vault_location.as_deref(), Some("./.mk/vault"));
    assert_eq!(task_root.keys_location.as_deref(), Some("./.mk/keys"));
    assert_eq!(task_root.key_name.as_deref(), Some("team"));
    assert_eq!(task_root.gpg_key_id, None);

    Ok(())
  }

  #[test]
  fn test_task_root_gpg_key_id_deserialized() -> anyhow::Result<()> {
    let yaml = "
      gpg_key_id: 0xABCD1234EFGH5678
      tasks:
        demo:
          commands:
            - command: echo ready
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    assert_eq!(task_root.gpg_key_id.as_deref(), Some("0xABCD1234EFGH5678"));

    Ok(())
  }

  #[test]
  fn test_task_root_gpg_key_id_absent_defaults_to_none() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        demo:
          commands:
            - command: echo ready
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;
    assert_eq!(task_root.gpg_key_id, None);

    Ok(())
  }

  #[test]
  fn test_task_root_apply_extends_gpg_key_id() -> anyhow::Result<()> {
    // Parent has a gpg_key_id; child does not → parent value propagates.
    let dir = TempDir::new().unwrap();
    let parent_path = dir.path().join("parent.yaml");
    let child_path = dir.path().join("child.yaml");

    std::fs::write(&parent_path, "gpg_key_id: PARENT_KEY\ntasks:\n  dummy: echo ok\n")?;
    std::fs::write(
      &child_path,
      "extends: parent.yaml\ntasks:\n  child_task: echo child\n",
    )?;

    let root = TaskRoot::from_file(child_path.to_str().unwrap())?;
    assert_eq!(root.gpg_key_id.as_deref(), Some("PARENT_KEY"));

    Ok(())
  }

  #[test]
  fn test_task_root_apply_extends_child_gpg_key_id_overrides() -> anyhow::Result<()> {
    // Child has its own gpg_key_id → it overrides the parent.
    let dir = TempDir::new().unwrap();
    let parent_path = dir.path().join("parent.yaml");
    let child_path = dir.path().join("child.yaml");

    std::fs::write(&parent_path, "gpg_key_id: PARENT_KEY\ntasks:\n  dummy: echo ok\n")?;
    std::fs::write(
      &child_path,
      "extends: parent.yaml\ngpg_key_id: CHILD_KEY\ntasks:\n  child_task: echo child\n",
    )?;

    let root = TaskRoot::from_file(child_path.to_str().unwrap())?;
    assert_eq!(root.gpg_key_id.as_deref(), Some("CHILD_KEY"));

    Ok(())
  }

  #[test]
  fn test_task_root_3() -> anyhow::Result<()> {
    let yaml = "
      tasks:
        task1: echo \"Hello, World 1!\"
        task2: echo \"Hello, World 2!\"
        task3: echo \"Hello, World 3!\"
    ";

    let task_root = serde_yaml::from_str::<TaskRoot>(yaml)?;

    assert_eq!(task_root.tasks.len(), 3);

    if let Task::String(command) = &task_root.tasks["task1"] {
      assert_eq!(command, "echo \"Hello, World 1!\"");
    } else {
      panic!("Expected Task::String");
    }

    if let Task::String(command) = &task_root.tasks["task2"] {
      assert_eq!(command, "echo \"Hello, World 2!\"");
    } else {
      panic!("Expected Task::String");
    }

    if let Task::String(command) = &task_root.tasks["task3"] {
      assert_eq!(command, "echo \"Hello, World 3!\"");
    } else {
      panic!("Expected Task::String");
    }

    Ok(())
  }

  #[test]
  fn test_task_root_4() -> anyhow::Result<()> {
    let lua = "
      {
        tasks = {
          task1 = 'echo \"Hello, World 1!\"',
          task2 = 'echo \"Hello, World 2!\"',
          task3 = 'echo \"Hello, World 3!\"',
        }
      }
    ";

    let task_root = get_lua_table(lua)?;

    assert_eq!(task_root.tasks.len(), 3);

    if let Task::String(command) = &task_root.tasks["task1"] {
      assert_eq!(command, "echo \"Hello, World 1!\"");
    } else {
      panic!("Expected Task::String");
    }

    if let Task::String(command) = &task_root.tasks["task2"] {
      assert_eq!(command, "echo \"Hello, World 2!\"");
    } else {
      panic!("Expected Task::String");
    }

    if let Task::String(command) = &task_root.tasks["task3"] {
      assert_eq!(command, "echo \"Hello, World 3!\"");
    } else {
      panic!("Expected Task::String");
    }

    Ok(())
  }

  #[test]
  fn test_task_root_5_from_file_loads_use_cargo() -> anyhow::Result<()> {
    use assert_fs::TempDir;
    use std::fs;

    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("tasks.yaml");
    fs::write(
      &config_path,
      "
      tasks:
        build:
          commands:
            - command: echo build
      use_cargo:
        work_dir: crates/app
      ",
    )?;

    let task_root = TaskRoot::from_file(config_path.to_str().unwrap())?;

    assert!(task_root.tasks.contains_key("test"));

    if let Task::Task(task) = &task_root.tasks["test"] {
      if let CommandRunner::LocalRun(local_run) = &task.commands[0] {
        assert_eq!(local_run.command, "cargo test");
        assert_eq!(
          local_run.work_dir,
          Some(
            temp_dir
              .path()
              .join("crates")
              .join("app")
              .to_string_lossy()
              .into_owned()
          )
        );
      } else {
        panic!("Expected CommandRunner::LocalRun");
      }
    } else {
      panic!("Expected Task::Task");
    }

    Ok(())
  }

  #[test]
  fn test_task_root_6_from_file_rejects_include() -> anyhow::Result<()> {
    use assert_fs::TempDir;
    use std::fs;

    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("tasks.yaml");
    fs::write(
      &config_path,
      "
      include:
        - shared.yaml
      tasks:
        hello:
          commands:
            - command: echo hello
      ",
    )?;

    let error = TaskRoot::from_file(config_path.to_str().unwrap()).unwrap_err();
    assert!(error
      .to_string()
      .contains("`include` is no longer supported. Use `extends` instead."));
    Ok(())
  }

  #[test]
  fn test_task_root_7_from_file_rejects_extends_cycle() -> anyhow::Result<()> {
    use assert_fs::TempDir;
    use std::fs;

    let temp_dir = TempDir::new()?;
    let a_path = temp_dir.path().join("a.yaml");
    let b_path = temp_dir.path().join("b.yaml");

    fs::write(
      &a_path,
      "
        extends: b.yaml
        tasks:
          a:
            commands:
              - command: echo a
        ",
    )?;
    fs::write(
      &b_path,
      "
        extends: a.yaml
        tasks:
          b:
            commands:
              - command: echo b
        ",
    )?;

    let error = TaskRoot::from_file(a_path.to_str().unwrap()).unwrap_err();
    assert!(error.to_string().contains("Circular extends detected:"));
    Ok(())
  }
}
