use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::secrets::Secrets;
use anyhow::Context as _;
use clap::{
  crate_authors,
  CommandFactory,
  Parser,
  Subcommand,
};
use clap_complete::Shell;
use console::style;
use mk_lib::file::DisplayPath as _;
use mk_lib::label_filter::{matches_all, LabelFilter};
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
use reqwest::header::{
  HeaderMap,
  ACCEPT,
  USER_AGENT,
};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use serde::{
  Deserialize,
  Serialize,
};

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
  #[command(visible_aliases = ["r"], about = "Run specific tasks")]
  Run {
    #[arg(help = "The task name to run", value_hint = clap::ValueHint::Other)]
    task_name: Option<String>,

    #[arg(
      long,
      help = "Print the resolved task plan without executing commands",
      conflicts_with = "json_events"
    )]
    dry_run: bool,

    #[arg(long, help = "Bypass task cache and force execution")]
    force: bool,

    #[arg(long, help = "Emit newline-delimited JSON execution events")]
    json_events: bool,

    #[arg(long = "label", help = "Run tasks matching label (KEY or KEY=VALUE). Repeatable; all must match.", value_name = "FILTER")]
    labels: Vec<String>,
  },
  #[command(visible_aliases = ["ls"], about = "List all available tasks")]
  List {
    #[arg(short, long, help = "Show list that does not include headers")]
    plain: bool,

    #[arg(short, long, help = "Show list in JSON format", conflicts_with = "plain")]
    json: bool,

    #[arg(long, help = "Disable colored list output", conflicts_with_all = ["plain", "json"])]
    no_color: bool,

    #[arg(long = "label", help = "Filter tasks by label (KEY or KEY=VALUE). Repeatable; all must match.", value_name = "FILTER")]
    labels: Vec<String>,
  },
  #[command(visible_aliases = ["comp", "completions"], about = "Generate shell completions")]
  Completion {
    #[arg(required = true, value_enum, help = "The shell to generate completions for")]
    shell: Shell,
  },
  #[command(about = "Validate task configuration without executing tasks")]
  Validate {
    #[arg(long, help = "Show validation results in JSON format")]
    json: bool,
  },
  #[command(about = "Show the resolved execution plan for a task")]
  Plan {
    #[arg(help = "The task name to inspect", value_hint = clap::ValueHint::Other)]
    task_name: Option<String>,

    #[arg(long, help = "Show the plan in JSON format")]
    json: bool,

    #[arg(long = "label", help = "Plan tasks matching label (KEY or KEY=VALUE). Repeatable; all must match.", value_name = "FILTER")]
    labels: Vec<String>,
  },
  #[command(visible_aliases = ["s"], arg_required_else_help = true, about = "Access stored secrets")]
  Secrets(Box<Secrets>),
  // Update does not require a config file.
  #[command(about = "Check for mk (make) updates")]
  Update,
  #[command(about = "Remove mk task cache metadata")]
  CleanCache,
  #[command(about = "Print the JSON Schema for the task configuration file")]
  Schema,
}

/// The CLI entry
pub(super) struct CliEntry {
  args: Args,
  task_root: Arc<TaskRoot>,
}

impl CliEntry {
  /// Create a new CLI entry
  pub fn new() -> anyhow::Result<Self> {
    let args = Self::parse_args();
    assert!(!args.config.is_empty());

    let (config, allow_without_config) = Self::resolve_config(&args)?;
    log::trace!("Config: {}", config.display_lossy());

    if !config.exists() && !allow_without_config {
      let mut message = format!("Config file does not exist: {}", config.display_lossy());
      if args.config == "tasks.yaml" {
        message.push_str(". Note: mk also checks for tasks.yml when tasks.yaml is missing.");
      }
      anyhow::bail!(message);
    }

    let task_root = if config.exists() {
      Arc::new(TaskRoot::from_file(&config)?)
    } else {
      debug_assert!(allow_without_config);
      let mut task_root = TaskRoot::default();
      task_root.source_path = Some(Self::absolute_config_path(&config)?);
      Arc::new(task_root)
    };
    Ok(Self { args, task_root })
  }

  fn parse_args() -> Args {
    Args::parse_from(Self::expand_hydra_aliases(env::args_os()))
  }

  fn expand_hydra_aliases(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    let mut args = args.into_iter();
    let mut expanded = Vec::new();

    if let Some(program) = args.next() {
      expanded.push(program);
    }

    let mut expanded_command = false;
    let mut config_value_pending = false;

    for arg in args {
      if config_value_pending {
        expanded.push(arg);
        config_value_pending = false;
        continue;
      }

      if !expanded_command {
        if arg == "-c" || arg == "--config" {
          config_value_pending = true;
          expanded.push(arg);
          continue;
        }

        if arg.to_string_lossy().starts_with("--config=") {
          expanded.push(arg);
          continue;
        }

        // Hydra aliases: single token → multi-word subcommand path.
        // Ordered longest-match first to avoid prefix ambiguity (e.g. `svls`
        // before `sv`).  None of these must shadow a top-level clap
        // visible_alias (`r`, `ls`, `comp`, `completions`, `s`).
        let hydra: Option<&[&str]> = match arg.to_string_lossy().as_ref() {
          // secrets vault <subcmd>
          "svls" | "svl" => Some(&["secrets", "vault", "list-secrets"]),
          "svi" => Some(&["secrets", "vault", "init-vault"]),
          "svst" => Some(&["secrets", "vault", "store-secret"]),
          "svsh" => Some(&["secrets", "vault", "show-secret"]),
          "svp" => Some(&["secrets", "vault", "purge-secret"]),
          "sve" => Some(&["secrets", "vault", "export-secret"]),
          // secrets vault (prefix — remainder passes through unchanged)
          "sv" => Some(&["secrets", "vault"]),
          // secrets <subcmd>
          "sk" => Some(&["secrets", "key"]),
          "slk" => Some(&["secrets", "list-keys"]),
          "sd" => Some(&["secrets", "doctor"]),
          "si" => Some(&["secrets", "init-vault"]),
          "se" => Some(&["secrets", "export-secret"]),
          _ => None,
        };
        if let Some(tokens) = hydra {
          expanded.extend(tokens.iter().map(|t| OsString::from(*t)));
          expanded_command = true;
          continue;
        }

        if !arg.to_string_lossy().starts_with('-') {
          expanded_command = true;
        }
      }

      expanded.push(arg);
    }

    expanded
  }

  fn resolve_config(args: &Args) -> anyhow::Result<(std::path::PathBuf, bool)> {
    let mut config = Path::new(&args.config).to_path_buf();
    if !config.exists() && args.config == "tasks.yaml" {
      for candidate in Self::default_config_candidates() {
        let fallback = Path::new(candidate);
        if fallback.exists() {
          config = fallback.to_path_buf();
          break;
        }
      }
    }

    let allow_without_config = matches!(
      args.command,
      Some(Command::Init { .. })
        | Some(Command::Completion { .. })
        | Some(Command::Update)
        | Some(Command::CleanCache)
        | Some(Command::Schema)
    ) || (matches!(args.command, Some(Command::Secrets(_)))
      && !Self::config_requested_explicitly(args));

    Ok((config, allow_without_config))
  }

  fn config_requested_explicitly(args: &Args) -> bool {
    if args.config != "tasks.yaml" || env::var_os("MK_CONFIG").is_some() {
      return true;
    }

    env::args_os()
      .any(|arg| arg == "-c" || arg == "--config" || arg.to_string_lossy().starts_with("--config="))
  }

  fn default_config_candidates() -> &'static [&'static str] {
    &[
      "tasks.yml",
      ".mk/tasks.yaml",
      ".mk/tasks.yml",
      "mk.toml",
      "tasks.toml",
      "tasks.json",
      "tasks.lua",
      ".mk/tasks.toml",
      ".mk/tasks.json",
      ".mk/tasks.lua",
    ]
  }

  fn absolute_config_path(config: &Path) -> anyhow::Result<std::path::PathBuf> {
    if config.is_absolute() {
      Ok(config.to_path_buf())
    } else {
      Ok(env::current_dir()?.join(config))
    }
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

        Self::ensure_init_path_supported(config_path)?;
        let contents = Self::build_init_contents();

        std::fs::write(config_path, contents)?;
        println!("Config file created at {}", config_path.display_lossy());
      },
      Some(Command::Run {
        task_name,
        dry_run,
        force,
        json_events,
        labels,
      }) => {
        let filters: Vec<LabelFilter> = labels.iter().map(|s| LabelFilter::parse(s)).collect();
        let names = self.resolve_run_tasks(task_name.as_deref(), &filters)?;
        if *dry_run {
          for name in &names {
            self.print_plan(name, false)?;
          }
        } else {
          for name in &names {
            self.run_task(name, *force, *json_events)?;
          }
        }
      },
      Some(Command::List {
        plain,
        json,
        no_color,
        labels,
      }) => {
        let filters: Vec<LabelFilter> = labels.iter().map(|s| LabelFilter::parse(s)).collect();
        self.print_available_tasks(*plain, *json, *no_color, &filters)?;
      },
      Some(Command::Completion { shell }) => {
        self.write_completions(*shell)?;
      },
      Some(Command::Validate { json }) => {
        self.validate_config(*json)?;
      },
      Some(Command::Plan { task_name, json, labels }) => {
        let filters: Vec<LabelFilter> = labels.iter().map(|s| LabelFilter::parse(s)).collect();
        let names = self.resolve_run_tasks(task_name.as_deref(), &filters)?;
        for name in &names {
          self.print_plan(name, *json)?;
        }
      },
      Some(Command::Secrets(secrets)) => {
        secrets.execute(&self.task_root)?;
      },
      Some(Command::Update) => {
        self.update_mk()?;
      },
      Some(Command::CleanCache) => {
        mk_lib::cache::CacheStore::remove_in_dir(&self.task_root.cache_base_dir())?;
        println!("Cache cleared");
      },
      Some(Command::Schema) => {
        let schema = mk_lib::generate_schema()?;
        let schema: serde_json::Value = serde_json::from_str(&schema)?;
        Self::print_json_value(schema)?;
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

  fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    Self::print_json_value(serde_json::to_value(value)?)
  }

  fn print_json_value(value: serde_json::Value) -> anyhow::Result<()> {
    let value = Self::canonicalize_json(value);
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
  }

  fn canonicalize_json(value: serde_json::Value) -> serde_json::Value {
    match value {
      serde_json::Value::Array(items) => {
        serde_json::Value::Array(items.into_iter().map(Self::canonicalize_json).collect::<Vec<_>>())
      },
      serde_json::Value::Object(map) => {
        let mut entries = map.into_iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        let mut canonical = serde_json::Map::new();
        for (key, value) in entries {
          canonical.insert(key, Self::canonicalize_json(value));
        }
        serde_json::Value::Object(canonical)
      },
      other => other,
    }
  }

  fn update_mk(&self) -> anyhow::Result<()> {
    println!("Checking for updates...");
    let current_version = VERSION.as_str();
    println!("Current version: {}", current_version);

    // Extract semver without git hash
    let current_semver = current_version.split_whitespace().next().unwrap_or("0.0.0");

    // GitHub API endpoint for latest release
    let client = Self::build_update_client(Self::update_connect_timeout(), Self::update_request_timeout())?;
    let latest_version = Self::fetch_latest_version(&client, &Self::update_latest_release_url())?;

    // Compare using semver for accurate ordering
    match (
      semver::Version::parse(&latest_version),
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

  /// Resolve task names from an optional explicit name or label filters.
  /// Returns a sorted list of matching task names.
  fn resolve_run_tasks(
    &self,
    task_name: Option<&str>,
    filters: &[LabelFilter],
  ) -> anyhow::Result<Vec<String>> {
    if let Some(name) = task_name {
      if !filters.is_empty() {
        anyhow::bail!("Cannot combine a task name with --label filters");
      }
      return Ok(vec![name.to_owned()]);
    }

    if filters.is_empty() {
      anyhow::bail!("Provide a task name or at least one --label filter");
    }

    let matched: Vec<String> = self
      .filtered_tasks(filters)
      .into_iter()
      .map(|(name, _)| name.to_owned())
      .collect();

    if matched.is_empty() {
      anyhow::bail!("No tasks matched the given label filters");
    }

    Ok(matched)
  }

  /// Run the specified tasks
  fn run_task(&self, task_name: &str, force: bool, json_events: bool) -> anyhow::Result<()> {
    assert!(!task_name.is_empty());
    let context = TaskContext::new_with_options(self.task_root.clone(), force, json_events);
    run_task_by_name(&context, task_name)
  }

  /// Build the contents of a new tasks.yaml, including a modeline and auto-detected integrations.
  fn build_init_contents() -> String {
    let mut out = String::new();

    // yaml-language-server modeline for editor schema support
    out.push_str("# yaml-language-server: $schema=https://raw.githubusercontent.com/ffimnsr/mk-rs/main/docs/schema.json\n");
    out.push('\n');

    let cwd = std::env::current_dir().unwrap_or_default();
    let has_cargo = cwd.join("Cargo.toml").exists();
    let has_npm = cwd.join("package.json").exists();

    if has_cargo {
      out.push_str("use_cargo: true\n");
      out.push('\n');
    }

    if has_npm {
      out.push_str("use_npm: true\n");
      out.push('\n');
    }

    out.push_str("tasks:\n");
    out.push_str("  greet:\n");
    out.push_str("    commands:\n");
    out.push_str("      - echo \"Hello, World!\"\n");
    out.push_str("    description: Sample greet command\n");

    out
  }

  // If TOML/JSON/Lua remain first-class entrypoints, add format-specific init templates.
  fn ensure_init_path_supported(config_path: &Path) -> anyhow::Result<()> {
    match config_path.extension().and_then(|ext| ext.to_str()) {
      Some("yaml") | Some("yml") | None => Ok(()),
      Some(ext) => anyhow::bail!(
        "`mk init` only writes YAML sample configs. Use a .yaml/.yml path instead of .{}",
        ext
      ),
    }
  }

  fn update_connect_timeout() -> Duration {
    Duration::from_secs(3)
  }

  fn update_request_timeout() -> Duration {
    Duration::from_secs(10)
  }

  fn update_latest_release_url() -> String {
    env::var("MK_UPDATE_URL")
      .unwrap_or_else(|_| String::from("https://api.github.com/repos/ffimnsr/mk-rs/releases/latest"))
  }

  fn build_update_client(connect_timeout: Duration, request_timeout: Duration) -> anyhow::Result<Client> {
    Client::builder()
      .connect_timeout(connect_timeout)
      .timeout(request_timeout)
      .redirect(Policy::limited(10))
      .build()
      .context("Failed to build update client")
  }

  fn fetch_latest_version(client: &Client, url: &str) -> anyhow::Result<String> {
    let resp = client
      .get(url)
      .header(USER_AGENT, format!("mk-rs/{}", VERSION.as_str()))
      .header(ACCEPT, "application/vnd.github+json")
      .send()
      .context("Failed to check for updates. Network unavailable or request timed out")?;

    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().context("Failed to read update response body")?;

    if !status.is_success() {
      anyhow::bail!(Self::format_update_error(url, status, &headers, &body));
    }

    let release: GitHubRelease = serde_json::from_str(&body).context("Invalid update response payload")?;
    let latest_version = release.tag_name.trim_start_matches('v').to_owned();

    Ok(latest_version)
  }

  fn format_update_error(url: &str, status: StatusCode, headers: &HeaderMap, body: &str) -> String {
    let mut details = Vec::new();

    if let Ok(api_error) = serde_json::from_str::<GitHubApiError>(body) {
      if let Some(message) = api_error.message.filter(|message| !message.is_empty()) {
        details.push(message);
      }
      if let Some(documentation_url) = api_error.documentation_url.filter(|url| !url.is_empty()) {
        details.push(format!("docs: {}", documentation_url));
      }
    } else {
      let snippet = body.trim();
      if !snippet.is_empty() {
        let snippet = if snippet.len() > 240 {
          format!("{}...", &snippet[..240])
        } else {
          snippet.to_string()
        };
        details.push(format!("body: {}", snippet));
      }
    }

    if let Some(reset) = headers
      .get("x-ratelimit-reset")
      .and_then(|value| value.to_str().ok())
    {
      details.push(format!("rate limit reset: {}", reset));
    }

    let reason = status.canonical_reason().unwrap_or("Unknown Status");
    if details.is_empty() {
      format!(
        "Failed to check for updates via {}: HTTP {} {}",
        url,
        status.as_u16(),
        reason
      )
    } else {
      format!(
        "Failed to check for updates via {}: HTTP {} {} ({})",
        url,
        status.as_u16(),
        reason,
        details.join("; ")
      )
    }
  }

  fn sorted_tasks(&self) -> Vec<(&str, &Task)> {
    let mut tasks = self
      .task_root
      .tasks
      .iter()
      .map(|(name, task)| (name.as_str(), task))
      .collect::<Vec<_>>();
    tasks.sort_unstable_by(|left, right| left.0.cmp(right.0));
    tasks
  }

  fn filtered_tasks<'a>(&'a self, filters: &[LabelFilter]) -> Vec<(&'a str, &'a Task)> {
    if filters.is_empty() {
      return self.sorted_tasks();
    }
    self
      .sorted_tasks()
      .into_iter()
      .filter(|(_, task)| match task {
        Task::Task(t) => matches_all(filters, &t.labels),
        // string shorthand tasks have no labels — never match a label filter
        Task::String(_) => false,
      })
      .collect()
  }

  /// Print all available tasks
  fn print_available_tasks(&self, plain: bool, json: bool, no_color: bool, filters: &[LabelFilter]) -> anyhow::Result<()> {
    if json {
      let tasks: Vec<_> = self
        .filtered_tasks(filters)
        .into_iter()
        .map(|(name, task)| {
          if let Task::Task(task) = task {
            let mut labels: Vec<_> = task.labels.iter().collect();
            labels.sort_by_key(|(k, _)| k.as_str());
            let labels_obj: serde_json::Map<String, serde_json::Value> = labels
              .into_iter()
              .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
              .collect();
            serde_json::json!({
              "description": task.description,
              "labels": labels_obj,
              "name": name,
            })
          } else {
            serde_json::json!({
              "description": "No description provided",
              "labels": {},
              "name": name,
            })
          }
        })
        .collect();
      Self::print_json(&tasks)?;
    } else {
      let mut table = Table::new();
      if !plain {
        table.set_titles(row![Fbb->"Task", Fbb->"Description"]);

        let use_color = !no_color && std::io::stdout().is_terminal();
        println!();
        if use_color {
          println!("{}", style("Available tasks:").bold().cyan());
        } else {
          println!("Available tasks:");
        }
        println!();
      }
      table.set_format(*consts::FORMAT_CLEAN);

      for (task_name, task) in self.filtered_tasks(filters) {
        if let Task::Task(task) = task {
          table.add_row(row![b->task_name, Fg->&task.description]);
        } else {
          table.add_row(row![b->task_name, Fg->"No description provided"]);
        }
      }

      table.printstd();
    }

    Ok(())
  }

  fn write_completions(&self, shell: Shell) -> anyhow::Result<()> {
    let mut app = Args::command();
    clap_complete::generate(shell, &mut app, "mk", &mut std::io::stdout().lock());

    Ok(())
  }

  fn validate_config(&self, json: bool) -> anyhow::Result<()> {
    let report = self.task_root.validate();
    if json {
      Self::print_json(&report)?;
    } else if report.has_errors() {
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
    } else {
      println!("Validation passed");
      if !report.issues.is_empty() {
        println!();
      }
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
      Self::print_json(&plan)?;
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

#[derive(Debug, Deserialize)]
struct GitHubRelease {
  tag_name: String,
}

#[derive(Debug, Deserialize)]
struct GitHubApiError {
  message: Option<String>,
  documentation_url: Option<String>,
}

#[cfg(test)]
mod tests {
  use super::{
    Args,
    CliEntry,
    Command,
  };
  use std::io::Read as _;
  use std::net::TcpListener;
  use std::sync::mpsc;
  use std::thread;

  #[test]
  fn init_rejects_non_yaml_output_paths() {
    let err = CliEntry::ensure_init_path_supported(std::path::Path::new("mk.toml")).unwrap_err();
    assert!(err.to_string().contains("only writes YAML"));
  }

  #[test]
  fn init_accepts_yaml_output_paths() {
    assert!(CliEntry::ensure_init_path_supported(std::path::Path::new("tasks.yaml")).is_ok());
    assert!(CliEntry::ensure_init_path_supported(std::path::Path::new("tasks.yml")).is_ok());
  }

  #[test]
  fn clean_cache_allows_missing_config() {
    let args = Args {
      config: String::from("tasks.yaml"),
      task_name: None,
      command: Some(Command::CleanCache),
    };

    let (_, allow_without_config) = CliEntry::resolve_config(&args).unwrap();
    assert!(allow_without_config);
  }

  #[test]
  fn update_timeouts_stay_bounded() {
    assert_eq!(
      CliEntry::update_connect_timeout(),
      std::time::Duration::from_secs(3)
    );
    assert_eq!(
      CliEntry::update_request_timeout(),
      std::time::Duration::from_secs(10)
    );
  }

  #[test]
  fn update_fetch_times_out_with_clear_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();

    let server = thread::spawn(move || {
      ready_tx.send(()).unwrap();
      let (mut stream, _) = listener.accept().unwrap();
      let mut buf = [0_u8; 1024];
      let _ = stream.read(&mut buf);
      thread::sleep(std::time::Duration::from_millis(250));
    });

    ready_rx.recv().unwrap();

    let client = CliEntry::build_update_client(
      std::time::Duration::from_millis(50),
      std::time::Duration::from_millis(100),
    )
    .unwrap();
    let err = CliEntry::fetch_latest_version(&client, &format!("http://{}/latest", address)).unwrap_err();

    assert!(err
      .to_string()
      .contains("Failed to check for updates. Network unavailable or request timed out"));

    server.join().unwrap();
  }

  #[test]
  fn update_fetch_surfaces_api_error_details() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();

    let server = thread::spawn(move || {
      ready_tx.send(()).unwrap();
      let (mut stream, _) = listener.accept().unwrap();
      let mut buf = [0_u8; 1024];
      let _ = stream.read(&mut buf);
      let body =
        "{\"message\":\"API rate limit exceeded\",\"documentation_url\":\"https://docs.github.com\"}";
      let response = format!(
        concat!(
          "HTTP/1.1 403 Forbidden\r\n",
          "Content-Type: application/json\r\n",
          "X-RateLimit-Reset: 1712345678\r\n",
          "Content-Length: {}\r\n",
          "\r\n",
          "{}"
        ),
        body.len(),
        body
      );
      use std::io::Write as _;
      stream.write_all(response.as_bytes()).unwrap();
    });

    ready_rx.recv().unwrap();

    let client = CliEntry::build_update_client(
      std::time::Duration::from_millis(50),
      std::time::Duration::from_millis(100),
    )
    .unwrap();
    let err = CliEntry::fetch_latest_version(&client, &format!("http://{}/latest", address)).unwrap_err();
    let message = err.to_string();

    assert!(message.contains("HTTP 403 Forbidden"));
    assert!(message.contains("API rate limit exceeded"));
    assert!(message.contains("https://docs.github.com"));
    assert!(message.contains("rate limit reset: 1712345678"));

    server.join().unwrap();
  }

  #[test]
  fn update_url_can_be_overridden_for_tests() {
    unsafe {
      std::env::set_var("MK_UPDATE_URL", "http://127.0.0.1:9/latest");
    }
    assert_eq!(CliEntry::update_latest_release_url(), "http://127.0.0.1:9/latest");
    unsafe {
      std::env::remove_var("MK_UPDATE_URL");
    }
  }

  #[test]
  fn default_config_candidates_include_supported_formats() {
    let candidates = CliEntry::default_config_candidates();
    assert!(candidates.contains(&"tasks.toml"));
    assert!(candidates.contains(&"tasks.json"));
    assert!(candidates.contains(&"tasks.lua"));
  }

  fn os(s: &str) -> std::ffi::OsString {
    std::ffi::OsString::from(s)
  }

  fn expand(args: &[&str]) -> Vec<String> {
    CliEntry::expand_hydra_aliases(args.iter().map(|s| os(s)))
      .into_iter()
      .map(|s| s.to_string_lossy().into_owned())
      .collect()
  }

  #[test]
  fn hydra_svls_expands_to_secrets_vault_list_secrets() {
    assert_eq!(expand(&["mk", "svls"]), ["mk", "secrets", "vault", "list-secrets"]);
  }

  #[test]
  fn hydra_svl_expands_to_secrets_vault_list_secrets() {
    assert_eq!(expand(&["mk", "svl"]), ["mk", "secrets", "vault", "list-secrets"]);
  }

  #[test]
  fn hydra_sv_expands_to_secrets_vault_prefix() {
    assert_eq!(
      expand(&["mk", "sv", "show", "mykey"]),
      ["mk", "secrets", "vault", "show", "mykey"]
    );
  }

  #[test]
  fn hydra_svi_expands_to_secrets_vault_init_vault() {
    assert_eq!(expand(&["mk", "svi"]), ["mk", "secrets", "vault", "init-vault"]);
  }

  #[test]
  fn hydra_svst_expands_to_secrets_vault_store_secret() {
    assert_eq!(
      expand(&["mk", "svst", "app/token", "value"]),
      ["mk", "secrets", "vault", "store-secret", "app/token", "value"]
    );
  }

  #[test]
  fn hydra_svsh_expands_to_secrets_vault_show_secret() {
    assert_eq!(
      expand(&["mk", "svsh", "app/token"]),
      ["mk", "secrets", "vault", "show-secret", "app/token"]
    );
  }

  #[test]
  fn hydra_svp_expands_to_secrets_vault_purge_secret() {
    assert_eq!(
      expand(&["mk", "svp", "app/token"]),
      ["mk", "secrets", "vault", "purge-secret", "app/token"]
    );
  }

  #[test]
  fn hydra_sve_expands_to_secrets_vault_export_secret() {
    assert_eq!(
      expand(&["mk", "sve", "app/token", "--output", "out.txt"]),
      ["mk", "secrets", "vault", "export-secret", "app/token", "--output", "out.txt"]
    );
  }

  #[test]
  fn hydra_sk_expands_to_secrets_key() {
    assert_eq!(
      expand(&["mk", "sk", "generate-key"]),
      ["mk", "secrets", "key", "generate-key"]
    );
  }

  #[test]
  fn hydra_slk_expands_to_secrets_list_keys() {
    assert_eq!(expand(&["mk", "slk"]), ["mk", "secrets", "list-keys"]);
  }

  #[test]
  fn hydra_sd_expands_to_secrets_doctor() {
    assert_eq!(expand(&["mk", "sd"]), ["mk", "secrets", "doctor"]);
  }

  #[test]
  fn hydra_si_expands_to_secrets_init_vault() {
    assert_eq!(expand(&["mk", "si"]), ["mk", "secrets", "init-vault"]);
  }

  #[test]
  fn hydra_se_expands_to_secrets_export_secret() {
    assert_eq!(
      expand(&["mk", "se", "app/token", "--output", "out.txt"]),
      ["mk", "secrets", "export-secret", "app/token", "--output", "out.txt"]
    );
  }

  #[test]
  fn hydra_config_flag_preserved_before_hydra_token() {
    assert_eq!(
      expand(&["mk", "-c", "my.yaml", "svls"]),
      ["mk", "-c", "my.yaml", "secrets", "vault", "list-secrets"]
    );
  }

  #[test]
  fn hydra_unknown_token_passes_through_unchanged() {
    assert_eq!(expand(&["mk", "run", "my-task"]), ["mk", "run", "my-task"]);
  }
}
