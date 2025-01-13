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
use std::path::Path;

use super::{
  Include,
  Task,
  UseCargo,
  UseNpm,
};

const MK_COMMANDS: [&str; 5] = ["run", "list", "completion", "secrets", "help"];

macro_rules! process_tasks {
  ($root:expr, $mk_commands:expr) => {
    // Rename tasks that have the same name as mk commands
    $root.tasks = rename_tasks($root.tasks, "task", &$mk_commands, &HashMap::new());

    if let Some(npm) = &$root.use_npm {
      let npm_tasks = npm.capture()?;

      // Rename tasks that have the same name as mk commands and existing tasks
      let renamed_npm_tasks = rename_tasks(npm_tasks, "npm", &$mk_commands, &$root.tasks);

      $root.tasks.extend(renamed_npm_tasks);
    }
  };
}

/// This struct represents the root of the task schema. It contains all the tasks
/// that can be executed.
#[derive(Debug, Default, Deserialize)]
pub struct TaskRoot {
  /// The tasks that can be executed
  pub tasks: HashMap<String, Task>,

  /// This allows mk to use npm scripts as tasks
  #[serde(default)]
  pub use_npm: Option<UseNpm>,

  /// This allows mk to use cargo commands as tasks
  #[serde(default)]
  pub use_cargo: Option<UseCargo>,

  /// Includes additional files to be merged into the current file
  #[serde(default)]
  pub include: Option<Vec<Include>>,
}

impl TaskRoot {
  pub fn from_file(file: &str) -> anyhow::Result<Self> {
    let file_path = Path::new(file);
    let file_extension = file_path
      .extension()
      .and_then(|ext| ext.to_str())
      .context("Failed to get file extension")?;

    match file_extension {
      "yaml" | "yml" => load_yaml_file(file),
      "lua" => load_lua_file(file),
      "json" => load_json_file(file),
      "toml" => load_toml_file(file),
      "json5" => anyhow::bail!("JSON5 files are not supported yet"),
      "makefile" | "mk" => anyhow::bail!("Makefiles are not supported yet"),
      _ => anyhow::bail!("Unsupported file extension - {}", file_extension),
    }
  }

  pub fn from_hashmap(tasks: HashMap<String, Task>) -> Self {
    Self {
      tasks,
      use_npm: None,
      use_cargo: None,
      include: None,
    }
  }
}

fn load_yaml_file(file: &str) -> anyhow::Result<TaskRoot> {
  let file = File::open(file).with_context(|| format!("Failed to open file - {}", file))?;
  let reader = BufReader::new(file);

  // Deserialize the YAML file into a serde_yaml::Value to be able to merge
  // anchors and aliases
  let mut value: serde_yaml::Value = serde_yaml::from_reader(reader)?;
  value.apply_merge()?;

  // Deserialize the serde_yaml::Value into a TaskRoot
  let mut root: TaskRoot = serde_yaml::from_value(value)?;

  process_tasks!(root, MK_COMMANDS);

  Ok(root)
}

fn load_toml_file(file: &str) -> anyhow::Result<TaskRoot> {
  let mut file = File::open(file).with_context(|| format!("Failed to open file - {}", file))?;
  let mut contents = String::new();
  file.read_to_string(&mut contents)?;

  // Deserialize the TOML file into a TaskRoot
  let mut root: TaskRoot = toml::from_str(&contents)?;

  process_tasks!(root, MK_COMMANDS);

  Ok(root)
}

fn load_json_file(file: &str) -> anyhow::Result<TaskRoot> {
  let file = File::open(file).with_context(|| format!("Failed to open file - {}", file))?;
  let reader = BufReader::new(file);

  // Deserialize the JSON file into a TaskRoot
  let mut root: TaskRoot = serde_json::from_reader(reader)?;

  process_tasks!(root, MK_COMMANDS);

  Ok(root)
}

fn load_lua_file(file: &str) -> anyhow::Result<TaskRoot> {
  let mut file = File::open(file).with_context(|| format!("Failed to open file - {}", file))?;
  let mut contents = String::new();
  file.read_to_string(&mut contents)?;

  // Deserialize the Lua value into a TaskRoot
  let mut root: TaskRoot = get_lua_table(&contents)?;

  process_tasks!(root, MK_COMMANDS);

  Ok(root)
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
        assert_eq!(local_run.shell, "sh");
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
        assert_eq!(local_run.shell, "sh");
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
        assert_eq!(local_run.shell, "sh");
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
        assert_eq!(local_run.shell, "sh");
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
}
