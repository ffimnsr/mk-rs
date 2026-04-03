use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use crate::secrets::Secrets;
use anyhow::Ok;
use clap::{
  crate_authors,
  CommandFactory,
  Parser,
  Subcommand,
};
use clap_complete::Shell;
use console::style;
use mk_lib::file::ToUtf8 as _;
use mk_lib::schema::{
  run_task_by_name,
  Task,
  TaskContext,
  TaskPlan,
  TaskRoot,
};
use mk_lib::version::get_version_digits;
use once_cell::sync::Lazy;
use prettytable::format::consts;
use prettytable::{
  row,
  Table,
};
use reqwest::blocking::Client;

static VERSION: Lazy<String> = Lazy::new(get_version_digits);

/// The CLI arguments
#[derive(Debug, Parser)]
#[command(
  version = VERSION.as_str(),
  about,
  long_about = "mk is a powerful and flexible task runner designed to help you automate and manage your tasks efficiently. It supports running commands both locally and inside containers, making it versatile for various environments and use cases. Running tasks in containers is a first-class citizen, ensuring seamless integration with containerized workflows.",
  arg_required_else_help = true,
  author = crate_authors!("\n"),
  propagate_version = true,
)]
struct Args {
  #[arg(
    short,
    long,
    help = "Config file to source",
    env = "MK_CONFIG",
    default_value = "tasks.yaml"
  )]
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
  #[command(about = "Initialize a sample tasks.yaml file in the current directory")]
  Init {
    #[arg(short, long, help = "Overwrite existing config file if present")]
    force: bool,
    #[arg(help = "Optional output path for the created config file")]
    output: Option<String>,
  },
  #[command(visible_aliases = ["r"], arg_required_else_help = true, about = "Run specific tasks")]
  Run {
    #[arg(required = true, help = "The task name to run", value_hint = clap::ValueHint::Other)]
    task_name: String,

    #[arg(long, help = "Print the resolved task plan without executing commands")]
    dry_run: bool,

    #[arg(long, help = "Bypass task cache and force execution")]
    force: bool,

    #[arg(long, help = "Emit newline-delimited JSON execution events")]
    json_events: bool,
  },
  #[command(visible_aliases = ["ls"], about = "List all available tasks")]
  List {
    #[arg(short, long, help = "Show list that does not include headers")]
    plain: bool,

    #[arg(short, long, help = "Show list in JSON format", conflicts_with = "plain")]
    json: bool,
  },
  #[command(visible_aliases = ["comp"], about = "Generate shell completions")]
  Completion {
    #[arg(required = true, help = "The shell to generate completions for")]
    shell: String,
  },
  #[command(about = "Validate task configuration without executing tasks")]
  Validate {
    #[arg(long, help = "Show validation results in JSON format")]
    json: bool,
  },
  #[command(about = "Show the resolved execution plan for a task")]
  Plan {
    #[arg(required = true, help = "The task name to inspect", value_hint = clap::ValueHint::Other)]
    task_name: String,

    #[arg(long, help = "Show the plan in JSON format")]
    json: bool,
  },
  #[command(visible_aliases = ["s"], arg_required_else_help = true, about = "Access stored secrets")]
  Secrets(Secrets),
  // Update does not require a config file.
  #[command(about = "Check for mk (make) updates")]
  Update,
  #[command(about = "Remove mk task cache metadata")]
  CleanCache,
}

/// The CLI entry
pub(super) struct CliEntry {
  args: Args,
  task_root: Arc<TaskRoot>,
}

impl CliEntry {
  /// Create a new CLI entry
  pub fn new() -> anyhow::Result<Self> {
    let args = Args::parse();
    assert!(!args.config.is_empty());

    let (config, allow_without_config) = Self::resolve_config(&args)?;
    log::trace!("Config: {}", config.to_utf8()?);

    if !config.exists() && !allow_without_config {
      let mut message = format!("Config file does not exist: {}", config.to_utf8()?);
      if args.config == "tasks.yaml" {
        message.push_str(". Note: mk also checks for tasks.yml when tasks.yaml is missing.");
      }
      anyhow::bail!(message);
    }

    let task_root = if allow_without_config {
      Arc::new(TaskRoot::default())
    } else {
      Arc::new(TaskRoot::from_file(config.to_utf8()?)?)
    };
    Ok(Self { args, task_root })
  }

  fn resolve_config(args: &Args) -> anyhow::Result<(std::path::PathBuf, bool)> {
    let mut config = Path::new(&args.config).to_path_buf();
    if !config.exists() && args.config == "tasks.yaml" {
      for candidate in ["tasks.yml", ".mk/tasks.yaml", ".mk/tasks.yml", "mk.toml"] {
        let fallback = Path::new(candidate);
        if fallback.exists() {
          config = fallback.to_path_buf();
          break;
        }
      }
    }

    let allow_without_config = matches!(
      args.command,
      Some(Command::Init { .. }) | Some(Command::Completion { .. }) | Some(Command::Update)
    );

    Ok((config, allow_without_config))
  }

  /// Run the CLI entry
  pub fn run(&self) -> anyhow::Result<()> {
    match &self.args.command {
      Some(Command::Init { force, output }) => {
        let config_path = if let Some(ref out) = output {
          Path::new(out)
        } else {
          Path::new(&self.args.config)
        };

        if config_path.exists() && !force {
          anyhow::bail!("Config file already exists. Use `--force` to overwrite");
        }

        let contents = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/init_tasks.yaml"));

        std::fs::write(config_path, contents)?;
        println!("Config file created at {}", config_path.to_utf8()?);
      },
      Some(Command::Run {
        task_name,
        dry_run,
        force,
        json_events,
      }) => {
        if *dry_run {
          self.print_plan(task_name, false)?;
        } else {
          self.run_task(task_name, *force, *json_events)?;
        }
      },
      Some(Command::List { plain, json }) => {
        self.print_available_tasks(*plain, *json)?;
      },
      Some(Command::Completion { shell }) => {
        self.write_completions(shell)?;
      },
      Some(Command::Validate { json }) => {
        self.validate_config(*json)?;
      },
      Some(Command::Plan { task_name, json }) => {
        self.print_plan(task_name, *json)?;
      },
      Some(Command::Secrets(secrets)) => {
        secrets.execute()?;
      },
      Some(Command::Update) => {
        self.update_mk()?;
      },
      Some(Command::CleanCache) => {
        mk_lib::cache::CacheStore::remove_in_dir(&self.task_root.cache_base_dir())?;
        println!("Cache cleared");
      },
      None => {
        if let Some(task_name) = &self.args.task_name {
          self.run_task(task_name, false, false)?;
        } else {
          anyhow::bail!("No subcommand or task name provided. Use `--help` flag for more information.");
        }
      },
    }

    Ok(())
  }

  fn update_mk(&self) -> anyhow::Result<()> {
    println!("Checking for updates...");
    let current_version = VERSION.as_str();
    println!("Current version: {}", current_version);

    // Extract semver without git hash
    let current_semver = current_version.split_whitespace().next().unwrap_or("0.0.0");

    // GitHub API endpoint for latest release
    let client = Client::new();
    let resp = client
      .get("https://api.github.com/repos/ffimnsr/mk-rs/releases/latest")
      .header("User-Agent", "mk-rs/updater")
      .send()?;

    if !resp.status().is_success() {
      anyhow::bail!("Failed to check for updates: {}", resp.status());
    }

    let release: serde_json::Value = resp.json()?;
    let latest_version = release["tag_name"]
      .as_str()
      .ok_or_else(|| anyhow::anyhow!("Invalid release tag"))?
      .trim_start_matches('v');

    // Compare using semver for accurate ordering
    match (
      semver::Version::parse(latest_version),
      semver::Version::parse(current_semver),
    ) {
      (Result::Ok(latest_v), Result::Ok(current_v)) => {
        if latest_v <= current_v {
          println!("You are using the latest version.");
        } else {
          println!(
            "New version {} is available (you have {})",
            latest_version, current_semver
          );
          println!("Visit https://github.com/ffimnsr/mk-rs/releases/latest to update");
        }
      },
      // Fallback to simple equality check if parsing fails
      _ => {
        if latest_version == current_semver {
          println!("You are using the latest version.");
        } else {
          println!(
            "New version {} is available (you have {})",
            latest_version, current_semver
          );
          println!("Visit https://github.com/ffimnsr/mk-rs/releases/latest to update");
        }
      },
    }

    Ok(())
  }

  /// Run the specified tasks
  fn run_task(&self, task_name: &str, force: bool, json_events: bool) -> anyhow::Result<()> {
    assert!(!task_name.is_empty());
    let context = TaskContext::new_with_options(self.task_root.clone(), force, json_events);
    run_task_by_name(&context, task_name)
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

  fn validate_config(&self, json: bool) -> anyhow::Result<()> {
    let report = self.task_root.validate();
    if json {
      println!("{}", serde_json::to_string_pretty(&report)?);
    } else if report.issues.is_empty() {
      println!("Validation passed");
    } else {
      println!("Validation failed");
      println!();
      for issue in &report.issues {
        let task = issue
          .task
          .as_ref()
          .map(|task| format!(" task={}", task))
          .unwrap_or_default();
        let field = issue
          .field
          .as_ref()
          .map(|field| format!(" field={}", field))
          .unwrap_or_default();
        println!(
          "{}{}{}",
          match issue.severity {
            mk_lib::schema::ValidationSeverity::Error => "ERROR",
            mk_lib::schema::ValidationSeverity::Warning => "WARNING",
          },
          task,
          field
        );
        println!("  {}", issue.message);
      }
    }

    if report.has_errors() {
      anyhow::bail!("Validation failed");
    }

    Ok(())
  }

  fn print_plan(&self, task_name: &str, json: bool) -> anyhow::Result<()> {
    let plan = self.task_root.plan_task(task_name)?;
    if json {
      println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
      self.print_plan_text(&plan);
    }
    Ok(())
  }

  fn print_plan_text(&self, plan: &TaskPlan) {
    println!("Plan for task: {}", plan.root_task);
    println!();

    for (index, step) in plan.steps.iter().enumerate() {
      println!("{}. {}", index + 1, step.name);
      println!("   base_dir: {}", step.base_dir);
      if let Some(description) = &step.description {
        if !description.is_empty() {
          println!("   description: {}", description);
        }
      }
      println!(
        "   mode: {}",
        match step.execution_mode {
          mk_lib::schema::PlannedExecutionMode::Sequential => "sequential",
          mk_lib::schema::PlannedExecutionMode::Parallel => "parallel",
        }
      );
      if !step.dependencies.is_empty() {
        println!("   depends_on: {}", step.dependencies.join(", "));
      }
      for command in &step.commands {
        println!("   {}", command.summary());
      }
      if let Some(reason) = &step.skipped_reason {
        println!("   skip: {}", reason);
      }
      println!();
    }
  }
}
