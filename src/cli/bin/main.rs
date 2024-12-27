use clap::Parser;
use mk_lib::schema::TaskRoot;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
  #[arg(short, long, default_value = "tasks.yaml")]
  config: String,

  task_name: String,
  task_args: Vec<String>,
}

fn main() -> anyhow::Result<()> {
  let args = Args::parse();

  log::trace!("Config: {}", args.config);
  let task_root = TaskRoot::from_file(&args.config)?;

  let task = task_root.tasks.get(&args.task_name).ok_or_else(|| anyhow::anyhow!("Task not found"))?;
  println!("Task: {:?}", task);

  for command in &task.commands {
    command.execute()?;
  }
  Ok(())
}
