use std::collections::HashSet;
use std::sync::{
  Arc,
  Mutex,
};

use clap::{
  Parser,
  Subcommand,
};
use console::style;
use lazy_static::lazy_static;
use mk_lib::schema::{
  ExecutionStack,
  TaskContext,
  TaskRoot,
};
use mk_lib::version::get_version_digits;
use prettytable::format::consts;
use prettytable::{
  row,
  Table,
};

lazy_static! {
  static ref VERSION: String = get_version_digits();
}

#[derive(Debug, Parser)]
#[command(
  version = VERSION.as_str(),
  about,
  long_about = None
)]
struct Args {
  #[arg(short, long, help = "Config file to source", default_value = "tasks.yaml")]
  config: String,

  #[arg(help = "The task names to run")]
  task_names: Vec<String>,

  #[command(subcommand)]
  command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
  #[command(aliases = ["r"], about = "Run specific tasks")]
  Run { task_names: Vec<String> },

  #[command(aliases = ["ls"], about = "List all available tasks")]
  List,
}

pub struct CliEntry {
  args: Args,
  task_root: Arc<TaskRoot>,
  execution_stack: ExecutionStack,
}

impl CliEntry {
  pub fn new() -> anyhow::Result<Self> {
    let args = Args::parse();
    log::trace!("Config: {}", args.config);

    let task_root = Arc::new(TaskRoot::from_file(&args.config)?);
    let execution_stack = Arc::new(Mutex::new(HashSet::new()));
    Ok(Self {
      args,
      task_root,
      execution_stack,
    })
  }

  pub fn run(&self) -> anyhow::Result<()> {
    match &self.args.command {
      Some(Command::Run { task_names }) => {
        self.run_tasks(task_names)?;
      },
      Some(Command::List) => {
        self.print_available_tasks();
      },
      None => {
        if !self.args.task_names.is_empty() {
          self.run_tasks(&self.args.task_names)?;
        } else {
          anyhow::bail!("No subcommand or task name provided. Use `--help` flag for more information.");
        }
      },
    }

    Ok(())
  }

  fn run_tasks(&self, task_names: &[String]) -> anyhow::Result<()> {
    for task_name in task_names {
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
        stack.insert(task_name.clone());
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
    }

    Ok(())
  }

  fn print_available_tasks(&self) {
    let mut table = Table::new();
    table.set_titles(row![Fbb->"Task", Fbb->"Description"]);
    table.set_format(*consts::FORMAT_CLEAN);

    for (task_name, task) in &self.task_root.tasks {
      table.add_row(row![b->&task_name, Fg->&task.description]);
    }

    let msg = style("Available tasks:").bold().cyan();
    println!();
    println!("{msg}");
    println!();
    table.printstd();
  }
}
