use anyhow::Context;
use indicatif::{
    HumanDuration,
    MultiProgress,
    ProgressBar,
    ProgressStyle,
};
use rand::Rng as _;
use serde::{
    Deserialize,
    Serialize,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{
    Duration,
    Instant,
};
use std::{
    fs,
    thread,
};

use super::{
    Command,
    Precondition,
    TaskDependency,
};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub commands: Vec<Command>,

    #[serde(default)]
    pub preconditions: Vec<Precondition>,

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
        let started = Instant::now();

        let mut environment = self.environment.clone();
        let additional_env = self.load_env_file()?;
        environment.extend(additional_env);

        let mut rng = rand::thread_rng();
        let multi = Arc::new(MultiProgress::new());

        // spinners can be found here:
        // https://github.com/sindresorhus/cli-spinners/blob/main/spinners.json
        let pb_style =
            ProgressStyle::with_template("{spinner:.green} [{prefix:.bold.dim}] {wide_msg:.cyan/blue} ")?
                .tick_chars("⣾⣽⣻⢿⡿⣟⣯⣷");

        let precondition_pb = multi.add(ProgressBar::new(self.preconditions.len() as u64));
        precondition_pb.set_style(pb_style.clone());
        precondition_pb.set_message("Running task precondition...");
        for (i, precondition) in self.preconditions.iter().enumerate() {
            thread::sleep(Duration::from_millis(rng.gen_range(40..300)));
            precondition_pb.set_prefix(format!("{}/{}", i + 1, self.preconditions.len()));
            precondition.execute(Some(&environment))?;
            precondition_pb.inc(1);
        }
        precondition_pb.finish_and_clear();

        let command_pb = multi.add(ProgressBar::new(self.commands.len() as u64));
        command_pb.set_style(pb_style);
        command_pb.set_message("Running task command...");
        for (i, command) in self.commands.iter().enumerate() {
            thread::sleep(Duration::from_millis(rng.gen_range(100..400)));
            command_pb.set_prefix(format!("{}/{}", i + 1, self.commands.len()));
            command.execute(multi.clone(), Some(&environment))?;
            command_pb.inc(1);
        }
        let message = format!("Task completed in {}.", HumanDuration(started.elapsed()));
        command_pb.finish_with_message(message);

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
