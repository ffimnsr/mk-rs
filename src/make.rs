use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context as _;
use std::collections::HashMap;

use crate::file::ToUtf8 as _;
use crate::schema::{Task, TaskArgs, TaskContext, TaskDependency, TaskRoot, TaskRootFormat};

pub(crate) const MAKE_TARGET_FALLBACK_DESCRIPTION: &str = "Imported from Makefile target";

pub fn locate_make_binary() -> anyhow::Result<PathBuf> {
  which::which("make").context("GNU Make is not available in PATH. Install `make` to use Makefile configs.")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeRuleRecord {
  pub target: String,
  pub is_not_a_target: bool,
  pub prerequisites: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportedMakeTarget {
  name: String,
  description: String,
  depends_on: Vec<String>,
}

pub(crate) fn load_makefile_task_root(file_path: &Path) -> anyhow::Result<TaskRoot> {
  let tasks = discover_make_imports(file_path)?
    .into_iter()
    .map(|target| {
      (
        target.name,
        Task::Task(Box::new(TaskArgs {
          description: target.description,
          depends_on: target
            .depends_on
            .into_iter()
            .map(TaskDependency::String)
            .collect(),
          ..Default::default()
        })),
      )
    })
    .collect::<HashMap<_, _>>();

  let mut root = TaskRoot::from_hashmap(tasks);
  root.format = TaskRootFormat::Makefile;
  Ok(root)
}

fn discover_make_imports(file_path: &Path) -> anyhow::Result<Vec<ImportedMakeTarget>> {
  let records = discover_make_rule_records(file_path)?;
  let descriptions = extract_make_target_descriptions(file_path)?;
  Ok(build_imported_make_targets(records, &descriptions))
}

pub(crate) fn run_make_target(context: &TaskContext, task_name: &str) -> anyhow::Result<()> {
  if !context.forwarded_args.is_empty() {
    anyhow::bail!(
      "Forwarded arguments are not supported for Makefile configs yet. Remove trailing args after `--`."
    );
  }

  let file_path = context
    .task_root
    .source_path
    .as_ref()
    .ok_or_else(|| anyhow::anyhow!("Makefile config path is missing from task context"))?;
  let make_binary = locate_make_binary()?;

  let status = Command::new(&make_binary)
    .arg("-f")
    .arg(file_path)
    .arg(task_name)
    .status()
    .with_context(|| {
      format!(
        "Failed to execute `{}` for Makefile target `{}`",
        make_binary.to_utf8().unwrap_or("<non-utf8-path>"),
        task_name
      )
    })?;

  if !status.success() {
    anyhow::bail!("Make target '{}' failed with status {}", task_name, status);
  }

  Ok(())
}

fn discover_make_rule_records(file_path: &Path) -> anyhow::Result<Vec<MakeRuleRecord>> {
  let make_binary = locate_make_binary()?;

  let output = Command::new(&make_binary)
    .arg("-pRrq")
    .arg("-f")
    .arg(file_path)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .with_context(|| {
      format!(
        "Failed to execute `{}` for Makefile target discovery",
        make_binary.to_utf8().unwrap_or("<non-utf8-path>")
      )
    })?;

  if !matches!(output.status.code(), Some(0 | 1)) {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let message = if stderr.is_empty() {
      format!("`make` exited with status {}", output.status)
    } else {
      stderr
    };
    anyhow::bail!(
      "Failed to discover Makefile targets from {}: {}",
      file_path.to_utf8().unwrap_or("<non-utf8-path>"),
      message
    );
  }

  let stdout =
    String::from_utf8(output.stdout).context("`make` target discovery output is not valid UTF-8")?;
  Ok(parse_make_database_rule_records(&stdout))
}

pub fn parse_make_database_rule_records(stdout: &str) -> Vec<MakeRuleRecord> {
  let mut in_files_section = false;
  let mut pending_not_a_target = false;
  let mut phony_targets = BTreeSet::new();
  let mut rules = Vec::new();

  for line in stdout.lines() {
    let line = line.trim_end();

    if !in_files_section {
      if line == "# Files" {
        in_files_section = true;
      }
      continue;
    }

    if line == "# files hash-table stats:" {
      break;
    }

    if line == "# Not a target:" {
      pending_not_a_target = true;
      continue;
    }

    if line.is_empty() || line.starts_with('#') {
      continue;
    }

    let Some((target, prerequisites)) = parse_make_rule_line(line) else {
      pending_not_a_target = false;
      continue;
    };

    if target == ".PHONY" {
      for prerequisite in prerequisites.split_whitespace() {
        if is_user_target_name(prerequisite) {
          phony_targets.insert(prerequisite.to_string());
        }
      }
      pending_not_a_target = false;
      continue;
    }

    rules.push(MakeRuleRecord {
      target: target.to_string(),
      is_not_a_target: pending_not_a_target,
      prerequisites: prerequisites
        .split_whitespace()
        .filter(|prerequisite| is_user_target_name(prerequisite))
        .map(str::to_string)
        .collect(),
    });
    pending_not_a_target = false;
  }

  let runnable_targets = rules
    .iter()
    .filter(|rule| is_user_runnable_target(&rule.target, rule.is_not_a_target, &phony_targets))
    .map(|rule| rule.target.clone())
    .collect::<BTreeSet<_>>();

  let mut rules = rules
    .into_iter()
    .filter(|rule| runnable_targets.contains(&rule.target) || phony_targets.contains(&rule.target))
    .map(|mut rule| {
      rule
        .prerequisites
        .retain(|prerequisite| runnable_targets.contains(prerequisite));
      rule
    })
    .collect::<Vec<_>>();

  rules.sort_by(|left, right| left.target.cmp(&right.target));
  rules
}

fn build_imported_make_targets(
  records: Vec<MakeRuleRecord>,
  descriptions: &HashMap<String, String>,
) -> Vec<ImportedMakeTarget> {
  let runnable_targets = records
    .iter()
    .map(|record| record.target.clone())
    .collect::<BTreeSet<_>>();

  let mut imported = records
    .into_iter()
    .map(|record| ImportedMakeTarget {
      description: descriptions
        .get(&record.target)
        .cloned()
        .unwrap_or_else(|| MAKE_TARGET_FALLBACK_DESCRIPTION.to_string()),
      depends_on: dedupe_dependencies(
        record
          .prerequisites
          .into_iter()
          .filter(|prerequisite| prerequisite != &record.target && runnable_targets.contains(prerequisite))
          .collect(),
      ),
      name: record.target,
    })
    .collect::<Vec<_>>();

  imported.sort_by(|left, right| left.name.cmp(&right.name));
  imported
}

fn dedupe_dependencies(dependencies: Vec<String>) -> Vec<String> {
  let mut seen = BTreeSet::new();
  let mut ordered = Vec::new();

  for dependency in dependencies {
    if seen.insert(dependency.clone()) {
      ordered.push(dependency);
    }
  }

  ordered
}

fn extract_make_target_descriptions(file_path: &Path) -> anyhow::Result<HashMap<String, String>> {
  let contents = fs::read_to_string(file_path).with_context(|| {
    format!(
      "Failed to read Makefile for target descriptions - {}",
      file_path.to_utf8().unwrap_or("<non-utf8-path>")
    )
  })?;

  Ok(parse_make_target_descriptions(&contents))
}

fn parse_make_target_descriptions(contents: &str) -> HashMap<String, String> {
  let mut descriptions = HashMap::new();

  for line in contents.lines() {
    if line.starts_with('\t') {
      continue;
    }

    let Some((target, remainder)) = parse_make_rule_line(line) else {
      continue;
    };
    if !is_user_target_name(target) {
      continue;
    }

    let Some((_, description)) = remainder.split_once("##") else {
      continue;
    };
    let description = description.trim();
    if description.is_empty() {
      continue;
    }

    descriptions.insert(target.to_string(), description.to_string());
  }

  descriptions
}

fn parse_make_rule_line(line: &str) -> Option<(&str, &str)> {
  if line.starts_with('\t') {
    return None;
  }

  let colon_index = line.find(':')?;
  let target = line[..colon_index].trim();
  if target.is_empty() {
    return None;
  }

  let remainder = line[colon_index..].trim_start_matches(':');
  if remainder.trim_start().starts_with('=') {
    return None;
  }

  Some((target, remainder.trim()))
}

fn is_user_runnable_target(target: &str, is_not_a_target: bool, phony_targets: &BTreeSet<String>) -> bool {
  if !is_user_target_name(target) {
    return false;
  }

  if is_not_a_target {
    return phony_targets.contains(target);
  }

  true
}

fn is_user_target_name(target: &str) -> bool {
  !target.is_empty() && !target.starts_with('.') && !target.contains('%')
}

#[cfg(test)]
mod tests {
  use super::*;
  use assert_fs::TempDir;
  use once_cell::sync::Lazy;
  use std::env;
  use std::ffi::OsString;
  use std::fs;
  use std::path::{Path, PathBuf};
  use std::sync::{Mutex, MutexGuard};

  static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

  fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner())
  }

  struct EnvGuard {
    original_path: Option<OsString>,
    original_cwd: PathBuf,
  }

  impl Drop for EnvGuard {
    fn drop(&mut self) {
      unsafe {
        match &self.original_path {
          Some(path) => env::set_var("PATH", path),
          None => env::remove_var("PATH"),
        }
      }
      let _ = env::set_current_dir(&self.original_cwd);
    }
  }

  #[cfg(unix)]
  fn write_fake_make(temp_dir: &TempDir, stdout: &str) -> anyhow::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;

    let path = temp_dir.path().join("make");
    let script = format!(
      "#!/bin/sh\nif [ -n \"$MK_TEST_MAKE_PWD_FILE\" ]; then\n  printf '%s\\n' \"$PWD\" > \"$MK_TEST_MAKE_PWD_FILE\"\nfi\nif [ -n \"$MK_TEST_MAKE_ARGS_FILE\" ]; then\n  printf '%s\\n' \"$@\" > \"$MK_TEST_MAKE_ARGS_FILE\"\nfi\ncat <<'EOF'\n{stdout}\nEOF\nexit \"${{MK_TEST_MAKE_EXIT_CODE:-0}}\"\n"
    );
    fs::write(&path, script)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    Ok(path)
  }

  fn install_test_path(dir: &Path) -> anyhow::Result<EnvGuard> {
    let original_path = env::var_os("PATH");
    let original_cwd = env::current_dir()?;
    let joined = env::join_paths([dir.as_os_str()])?;
    unsafe {
      env::set_var("PATH", joined);
    }
    Ok(EnvGuard {
      original_path,
      original_cwd,
    })
  }

  fn prepend_test_path(dir: &Path) -> anyhow::Result<EnvGuard> {
    let original_path = env::var_os("PATH");
    let original_cwd = env::current_dir()?;
    let joined = match &original_path {
      Some(path) => env::join_paths(
        std::iter::once(dir.as_os_str()).chain(
          env::split_paths(path)
            .map(|p| p.into_os_string())
            .collect::<Vec<_>>()
            .iter()
            .map(|p| p.as_os_str()),
        ),
      )?,
      None => env::join_paths([dir.as_os_str()])?,
    };
    unsafe {
      env::set_var("PATH", joined);
    }
    Ok(EnvGuard {
      original_path,
      original_cwd,
    })
  }

  #[test]
  fn parse_make_database_targets_filters_special_and_pattern_rules() {
    let stdout = "# GNU Make 4.4\n# Files\n\n# Not a target:\nclean:\n#  File has not been updated.\n\nall: build\n#  File has not been updated.\n\nbuild:\n#  File has not been updated.\n\n# Not a target:\ntest:\n#  File has not been updated.\n\n.PHONY: clean test\n#  File has not been updated.\n\n.DEFAULT:\n#  File has not been updated.\n\n%.o: %.c\n#  File has not been updated.\n\n.SUFFIXES: .c .o\n#  File has not been updated.\n\n# files hash-table stats:\n";

    assert_eq!(
      parse_make_database_rule_records(stdout)
        .into_iter()
        .map(|rule| rule.target)
        .collect::<Vec<_>>(),
      vec![
        String::from("all"),
        String::from("build"),
        String::from("clean"),
        String::from("test"),
      ]
    );
  }

  #[test]
  fn parse_make_target_descriptions_extracts_inline_comments_only() {
    let contents = "build: prep ## Build project\n\t@echo build\nprep: ## Prepare deps\nlint: testdata.txt\n";

    let descriptions = parse_make_target_descriptions(contents);
    assert_eq!(descriptions.get("build"), Some(&String::from("Build project")));
    assert_eq!(descriptions.get("prep"), Some(&String::from("Prepare deps")));
    assert!(!descriptions.contains_key("lint"));
  }

  #[test]
  fn build_imported_make_targets_keeps_fallback_description_and_safe_dependencies() {
    let records = vec![
      MakeRuleRecord {
        target: String::from("build"),
        is_not_a_target: false,
        prerequisites: vec![
          String::from("prep"),
          String::from("README.md"),
          String::from("prep"),
          String::from("lint"),
        ],
      },
      MakeRuleRecord {
        target: String::from("lint"),
        is_not_a_target: false,
        prerequisites: vec![String::from("src")],
      },
      MakeRuleRecord {
        target: String::from("prep"),
        is_not_a_target: false,
        prerequisites: Vec::new(),
      },
    ];
    let descriptions = HashMap::from([(String::from("build"), String::from("Build project"))]);

    let imported = build_imported_make_targets(records, &descriptions);

    assert_eq!(
      imported,
      vec![
        ImportedMakeTarget {
          name: String::from("build"),
          description: String::from("Build project"),
          depends_on: vec![String::from("prep"), String::from("lint")],
        },
        ImportedMakeTarget {
          name: String::from("lint"),
          description: String::from(MAKE_TARGET_FALLBACK_DESCRIPTION),
          depends_on: Vec::new(),
        },
        ImportedMakeTarget {
          name: String::from("prep"),
          description: String::from(MAKE_TARGET_FALLBACK_DESCRIPTION),
          depends_on: Vec::new(),
        },
      ]
    );
  }

  #[cfg(unix)]
  #[test]
  fn load_makefile_task_root_imports_descriptions_and_dependencies() -> anyhow::Result<()> {
    let _lock = lock_env();
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("Makefile");

    fs::write(
      &config_path,
      "build: prep README.md ## Build project\n\t@echo build\nprep:\n\t@echo prep\nlint: ## Run lints\n\t@echo lint\n",
    )?;

    write_fake_make(
      &temp_dir,
      "# GNU Make 4.4\n# Files\nbuild: prep README.md lint\n#  File has not been updated.\n\nlint:\n#  File has not been updated.\n\nprep:\n#  File has not been updated.\n\n# files hash-table stats:\n",
    )?;
    let _env_guard = prepend_test_path(temp_dir.path())?;

    let root = load_makefile_task_root(&config_path)?;

    match &root.tasks["build"] {
      Task::Task(task) => {
        assert_eq!(task.description, "Build project");
        assert_eq!(task.depends_on.len(), 2);
        assert_eq!(task.depends_on[0].resolve_name(), "prep");
        assert_eq!(task.depends_on[1].resolve_name(), "lint");
      },
      Task::String(_) => panic!("Expected imported Make target to use Task::Task"),
    }

    match &root.tasks["prep"] {
      Task::Task(task) => assert_eq!(task.description, MAKE_TARGET_FALLBACK_DESCRIPTION),
      Task::String(_) => panic!("Expected imported Make target to use Task::Task"),
    }

    match &root.tasks["lint"] {
      Task::Task(task) => assert_eq!(task.description, "Run lints"),
      Task::String(_) => panic!("Expected imported Make target to use Task::Task"),
    }

    Ok(())
  }

  #[cfg(unix)]
  #[test]
  fn discover_make_targets_invokes_make_with_config_path_and_preserves_cwd() -> anyhow::Result<()> {
    let _lock = lock_env();
    let temp_dir = TempDir::new()?;
    let cwd_dir = temp_dir.path().join("cwd");
    let config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&cwd_dir)?;
    fs::create_dir_all(&config_dir)?;

    let pwd_file = temp_dir.path().join("pwd.txt");
    let args_file = temp_dir.path().join("args.txt");
    let config_path = config_dir.join("Makefile");
    fs::write(&config_path, "all:\n\t@echo hi\n")?;

    write_fake_make(
      &temp_dir,
      "# GNU Make 4.4\n# Files\nall:\n# files hash-table stats:\n",
    )?;
    let _env_guard = prepend_test_path(temp_dir.path())?;

    unsafe {
      env::set_var("MK_TEST_MAKE_PWD_FILE", &pwd_file);
      env::set_var("MK_TEST_MAKE_ARGS_FILE", &args_file);
    }
    env::set_current_dir(&cwd_dir)?;

    let targets = discover_make_imports(&config_path)?;

    unsafe {
      env::remove_var("MK_TEST_MAKE_PWD_FILE");
      env::remove_var("MK_TEST_MAKE_ARGS_FILE");
    }

    assert_eq!(
      targets.into_iter().map(|target| target.name).collect::<Vec<_>>(),
      vec![String::from("all")]
    );
    let actual_cwd = Path::new(fs::read_to_string(&pwd_file)?.trim()).canonicalize()?;
    let expected_cwd = cwd_dir.canonicalize()?;
    assert_eq!(actual_cwd, expected_cwd);

    let args = fs::read_to_string(&args_file)?;
    assert!(args.contains("-pRrq"));
    assert!(args.contains("-f"));
    assert!(args.contains(config_path.to_string_lossy().as_ref()));

    Ok(())
  }

  #[test]
  fn discover_make_targets_reports_missing_make_binary() {
    let _lock = lock_env();
    let temp_dir = TempDir::new().unwrap();
    let _env_guard = install_test_path(temp_dir.path()).unwrap();
    let error = discover_make_imports(Path::new("Makefile")).unwrap_err();

    assert!(error.to_string().contains("GNU Make is not available in PATH"));
  }
}
