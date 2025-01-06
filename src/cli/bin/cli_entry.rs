use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{
  Arc,
  Mutex,
};

use anyhow::Ok;
use clap::{
  CommandFactory,
  Parser,
  Subcommand,
};
use clap_complete::Shell;
use console::style;
use mk_lib::schema::{
  ExecutionStack,
  Task,
  TaskContext,
  TaskRoot,
};
use mk_lib::version::get_version_digits;
use once_cell::sync::Lazy;
use prettytable::format::consts;
use prettytable::{
  row,
  Table,
};

static VERSION: Lazy<String> = Lazy::new(get_version_digits);

/// The CLI arguments
#[derive(Debug, Parser)]
#[command(
  version = VERSION.as_str(),
  about,
  long_about = None
)]
struct Args {
  #[arg(short, long, help = "Config file to source", default_value = "tasks.yaml")]
  config: String,

  // Waiting for the dynamic completion to be implemented
  // Tracking can be found here:
  // - https://github.com/clap-rs/clap/issues/3166
  // - https://github.com/clap-rs/clap/issues/1232
  //
  // Usually, this would call `mk list --plain` or `mk list --json` to capture
  // the available tasks and use them as completions.
  #[arg(help = "The task name to run", value_hint = clap::ValueHint::Other)]
  task_name: Option<String>,

  #[command(subcommand)]
  command: Option<Command>,
}

/// The available subcommands
#[derive(Debug, Subcommand)]
enum Command {
  #[command(aliases = ["r"], about = "Run specific tasks")]
  Run {
    #[arg(help = "The task name to run", value_hint = clap::ValueHint::Other)]
    task_name: String,
  },
  #[command(aliases = ["ls"], about = "List all available tasks")]
  List {
    #[arg(short, long, help = "Show list that does not include headers")]
    plain: bool,

    #[arg(short, long, help = "Show list in JSON format", conflicts_with = "plain")]
    json: bool,
  },
  #[command(about = "Generate shell completions")]
  Completions {
    #[arg(help = "The shell to generate completions for")]
    shell: String,
  },
}

/// The CLI entry
pub struct CliEntry {
  args: Args,
  task_root: Arc<TaskRoot>,
  execution_stack: ExecutionStack,
}

impl CliEntry {
  /// Create a new CLI entry
  pub fn new() -> anyhow::Result<Self> {
    let args = Args::parse();
    log::trace!("Config: {}", args.config);

    assert!(!args.config.is_empty());

    let task_root = Arc::new(TaskRoot::from_file(&args.config)?);
    let execution_stack = Arc::new(Mutex::new(HashSet::new()));
    Ok(Self {
      args,
      task_root,
      execution_stack,
    })
  }

  /// Run the CLI entry
  pub fn run(&self) -> anyhow::Result<()> {
    match &self.args.command {
      Some(Command::Run { task_name }) => {
        self.run_task(task_name)?;
      },
      Some(Command::List { plain, json }) => {
        self.print_available_tasks(*plain, *json)?;
      },
      Some(Command::Completions { shell }) => {
        self.write_completions(shell)?;
      },
      None => {
        if let Some(task_name) = &self.args.task_name {
          self.run_task(task_name)?;
        } else {
          anyhow::bail!("No subcommand or task name provided. Use `--help` flag for more information.");
        }
      },
    }

    Ok(())
  }

  /// Run the specified tasks
  fn run_task(&self, task_name: &str) -> anyhow::Result<()> {
    assert!(!task_name.is_empty());

    let task = self
      .task_root
      .tasks
      .get(task_name)
      .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

    log::trace!("Task: {:?}", task);

    // Scope the lock to the task execution
    {
      let mut stack = self
        .execution_stack
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock execution stack - {}", e))?;

      stack.insert(task_name.to_string());
    }

    let mut context = TaskContext::new(self.task_root.clone(), self.execution_stack.clone());
    task.run(&mut context)?;

    // Don't carry over the execution stack to the next task
    {
      let mut stack = self
        .execution_stack
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock execution stack - {}", e))?;

      stack.clear();
    }

    Ok(())
  }

  /// Print all available tasks
  fn print_available_tasks(&self, plain: bool, json: bool) -> anyhow::Result<()> {
    if json {
      let tasks: Vec<_> = self
        .task_root
        .tasks
        .iter()
        .map(|(name, task)| {
          if let Task::Task(task) = task {
            serde_json::json!({
              "name": name,
              "description": task.description,
            })
          } else {
            serde_json::json!({
              "name": name,
              "description": "No description provided",
            })
          }
        })
        .collect();
      println!("{}", serde_json::to_string_pretty(&tasks)?);
    } else {
      let mut table = Table::new();
      if !plain {
        table.set_titles(row![Fbb->"Task", Fbb->"Description"]);

        let msg = style("Available tasks:").bold().cyan();
        println!();
        println!("{msg}");
        println!();
      }
      table.set_format(*consts::FORMAT_CLEAN);

      for (task_name, task) in &self.task_root.tasks {
        if let Task::Task(task) = task {
          table.add_row(row![b->&task_name, Fg->&task.description]);
        } else {
          table.add_row(row![b->&task_name, Fg->"No description provided"]);
        }
      }

      table.printstd();
    }

    Ok(())
  }

  fn write_completions(&self, shell: &str) -> anyhow::Result<()> {
    let shell = Shell::from_str(shell).map_err(|e| anyhow::anyhow!("Invalid shell - {}", e))?;

    let mut app = Args::command();
    clap_complete::generate(shell, &mut app, "mk", &mut std::io::stdout().lock());

    Ok(())
  }
}
