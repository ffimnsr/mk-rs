use clap::{
    Parser,
    Subcommand,
};
use mk_lib::schema::TaskRoot;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "tasks.yaml")]
    config: String,

    #[arg(help = "The task name to run")]
    task_name: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(aliases = ["r"], about = "Run a specific task")]
    Run { task_name: String },
    #[command(aliases = ["ls"], about = "List all available tasks")]
    List,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    log::trace!("Config: {}", args.config);
    let task_root = TaskRoot::from_file(&args.config)?;

    match args.command {
        Some(Command::Run { task_name }) => {
            let task = task_root
                .tasks
                .get(&task_name)
                .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

            log::trace!("Task: {:?}", task);
            task.run()?;
        },
        Some(Command::List) => {
            for task_name in task_root.tasks.keys() {
                println!("{}", task_name);
            }
        },
        None => {
            if let Some(task_name) = args.task_name {
                let task = task_root
                    .tasks
                    .get(&task_name)
                    .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

                log::trace!("Task: {:?}", task);
                task.run()?;
            } else {
                anyhow::bail!("No subcommand or task name provided");
            }
        },
    }

    Ok(())
}
