use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use super::{Command, Precondition, TaskDependency};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub commands: Vec<Command>,

    #[serde(default)]
    pub precondition: Vec<Precondition>,

    #[serde(default)]
    pub depends_on: Vec<TaskDependency>,

    #[serde(default)]
    pub labels: HashMap<String, String>,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub environment: HashMap<String, String>,

    #[serde(default)]
    pub env_file: Vec<String>,
}

impl Task {
    pub fn run(&self) -> anyhow::Result<()> {
        let mut environment = self.environment.clone();
        let additional_env = self.load_env_file()?;
        environment.extend(additional_env);

        for preconditions in &self.precondition {
            preconditions.execute(Some(&environment))?;
        }

        for command in &self.commands {
            command.execute(Some(&environment))?;
        }

        Ok(())
    }

    fn load_env_file(&self) -> anyhow::Result<HashMap<String, String>> {
        let mut local_env: HashMap<String, String> = HashMap::new();
        for env_file in &self.env_file {
            let contents = fs::read_to_string(env_file)
                .with_context(|| format!("Failed to read env file: {}", env_file))?;

            for line in contents.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    local_env.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }

        Ok(local_env)
    }
}

mod test {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_task() {
        {
            let yaml = "
        commands:
          - command: echo \"Hello, World!\"
            ignore_errors: false
            verbose: false
        depends_on:
          - name: task1
        description: 'This is a task'
        labels: {}
        environment:
          FOO: bar
        env_file:
          - test.env
      ";

            let task = serde_yaml::from_str::<Task>(yaml).unwrap();

            assert_eq!(task.commands[0].command, "echo \"Hello, World!\"");
            assert_eq!(task.depends_on[0].name, "task1");
            assert_eq!(task.labels.len(), 0);
            assert_eq!(task.description, "This is a task");
            assert_eq!(task.environment.len(), 1);
            assert_eq!(task.env_file.len(), 1);
        }
    }
}
