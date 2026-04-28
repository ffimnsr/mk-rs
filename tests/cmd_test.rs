use assert_cmd::{
  cargo,
  Command,
};
use assert_fs::TempDir;
use mk_lib::file::ToUtf8 as _;
use mk_lib::secrets::read_vault_gpg_key_id;
use predicates::prelude::*;

mod common;

#[test]
fn test_sanity() {
  assert_eq!(2 + 2, 4);
}

fn setup_secrets_fixture(
  secret_path: &str,
  secret_value: &str,
) -> anyhow::Result<(TempDir, String, std::path::PathBuf, std::path::PathBuf)> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  let vault_dir = temp_dir.path().join("vault");
  let keys_dir = temp_dir.path().join("keys");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("key")
    .arg("generate-key")
    .arg("--location")
    .arg(&keys_dir)
    .arg("--name")
    .arg("default")
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("init-vault")
    .arg("--vault-location")
    .arg(&vault_dir)
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("store-secret")
    .arg(secret_path)
    .arg(secret_value)
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--keys-location")
    .arg(&keys_dir)
    .arg("--key-name")
    .arg("default")
    .assert()
    .success();

  Ok((temp_dir, config_file_path, vault_dir, keys_dir))
}

#[cfg(unix)]
fn write_fake_selector(
  temp_dir: &TempDir,
  program_name: &str,
  script: &str,
) -> anyhow::Result<std::path::PathBuf> {
  use std::os::unix::fs::PermissionsExt as _;

  let path = temp_dir.path().join(program_name);
  std::fs::write(&path, script)?;
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
  Ok(path)
}

#[cfg(unix)]
fn write_fake_sh(temp_dir: &TempDir) -> anyhow::Result<std::path::PathBuf> {
  use std::os::unix::fs::symlink;

  let path = temp_dir.path().join("sh");
  symlink("/bin/sh", &path)?;
  Ok(path)
}

#[cfg(unix)]
fn write_fake_ssh(temp_dir: &TempDir) -> anyhow::Result<std::path::PathBuf> {
  use std::os::unix::fs::PermissionsExt as _;

  let path = temp_dir.path().join("ssh");
  std::fs::write(
    &path,
    r#"#!/bin/sh
if [ -n "$MK_TEST_SSH_ARGS_FILE" ]; then
  printf '%s\n' "$@" > "$MK_TEST_SSH_ARGS_FILE"
fi

while [ $# -gt 0 ]; do
  case "$1" in
    -T|-tt)
      shift
      ;;
    -p|-i|-o)
      shift 2
      ;;
    --)
      shift
      break
      ;;
    -*)
      shift
      ;;
    *)
      break
      ;;
  esac
done

target="$1"
shift
remote_command="$1"

if [ -z "$target" ] || [ -z "$remote_command" ]; then
  echo "fake ssh missing target or command" >&2
  exit 99
fi

exec /bin/sh -c "$remote_command"
"#,
  )?;
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
  Ok(path)
}

#[test]
fn test_mk_1() -> anyhow::Result<()> {
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("-h").assert();
  assert
    .success()
    .stdout(predicates::str::contains("Yet another simple task runner"))
    .stdout(predicates::str::contains("run"))
    .stdout(predicates::str::contains("list"))
    .stdout(predicates::str::contains("validate"))
    .stdout(predicates::str::contains("plan"))
    .stdout(predicates::str::contains("completions"))
    .stdout(predicates::str::contains("help"))
    .stdout(predicates::str::contains("--config"))
    .stdout(predicates::str::contains("--help"))
    .stdout(predicates::str::contains("--version"));
  Ok(())
}

#[test]
fn test_mk_2() -> anyhow::Result<()> {
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("--version").assert();

  let version = env!("CARGO_PKG_VERSION");
  let version_str = format!("mk {}", version);

  assert.success().stdout(predicates::str::contains(version_str));
  Ok(())
}

#[test]
fn test_mk_3() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("-c").arg(&config_file_path).arg("ls").assert();
  assert.success().stdout(predicates::str::contains("hello"));
  Ok(())
}

#[test]
fn test_mk_4() -> anyhow::Result<()> {
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("help").assert();
  assert
    .success()
    .stdout(predicates::str::contains("mk is a powerful and flexible task runner designed to help you automate and manage your tasks efficiently"))
    .stdout(predicates::str::contains("run"))
    .stdout(predicates::str::contains("list"))
    .stdout(predicates::str::contains("validate"))
    .stdout(predicates::str::contains("plan"))
    .stdout(predicates::str::contains("completion"))
    .stdout(predicates::str::contains("secrets"))
    .stdout(predicates::str::contains("help"))
    .stdout(predicates::str::contains("--config"))
    .stdout(predicates::str::contains("--help"))
    .stdout(predicates::str::contains("--version"));
  Ok(())
}

#[test]
fn test_completion_bash_includes_dynamic_task_hook() -> anyhow::Result<()> {
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .arg("completion")
    .arg("bash")
    .assert()
    .success()
    .stdout(predicates::str::contains("MK_COMPLETE_TASKS=1"))
    .stdout(predicates::str::contains("__mk_should_complete_task"));
  Ok(())
}

#[test]
fn test_completion_task_candidates_respect_prefix_and_config() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
      bundle:
        commands:
          - command: echo bundle
            verbose: false
      lint:
        commands:
          - command: echo lint
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env("MK_COMPLETE_TASKS", "1")
    .env("MK_COMPLETE_PREFIX", "bu")
    .arg("-c")
    .arg(&config_file_path)
    .assert()
    .success()
    .stdout(predicates::str::contains("build"))
    .stdout(predicates::str::contains("bundle"))
    .stdout(predicates::str::contains("lint").not());

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_run_fzf_selects_task_from_fzf() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let marker_file = temp_dir.path().join("selected.txt");
  let capture_file = temp_dir.path().join("fzf-input.txt");
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
    tasks:
      zebra:
        description: Zebra task
        commands:
          - command: printf zebra > {}
            verbose: false
      alpha:
        description: Alpha task
        commands:
          - command: printf alpha > {}
            verbose: false
    ",
      common::sh_path(&marker_file),
      common::sh_path(&marker_file)
    ),
  )?;
  write_fake_selector(
    &temp_dir,
    "fzf",
    &format!(
      "#!/bin/sh\nwhile IFS= read -r line; do printf '%s\\n' \"$line\"; done > \"{}\"\nprintf 'zebra\\tZebra task\\n'\n",
      common::sh_path(&capture_file)
    ),
  )?;
  let path = format!(
    "{}:{}",
    temp_dir.path().to_string_lossy(),
    std::env::var("PATH").unwrap_or_default()
  );

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--fzf")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&marker_file)?, "zebra");
  assert_eq!(
    std::fs::read_to_string(&capture_file)?,
    "alpha\tAlpha task\nzebra\tZebra task\n"
  );
  Ok(())
}

#[cfg(unix)]
#[test]
fn test_run_fzf_filters_candidates_before_selection() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let marker_file = temp_dir.path().join("selected.txt");
  let capture_file = temp_dir.path().join("fzf-input.txt");
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
    tasks:
      build:
        description: Build
        labels:
          area: ci
        commands:
          - command: printf build > {}
            verbose: false
      deploy:
        description: Deploy
        labels:
          area: ci
        commands:
          - command: printf deploy > {}
            verbose: false
      lint:
        description: Lint
        commands:
          - command: printf lint > {}
            verbose: false
    ",
      common::sh_path(&marker_file),
      common::sh_path(&marker_file),
      common::sh_path(&marker_file)
    ),
  )?;
  write_fake_selector(
    &temp_dir,
    "fzf",
    &format!(
      "#!/bin/sh\nwhile IFS= read -r line; do printf '%s\\n' \"$line\"; done > \"{}\"\nprintf 'build\\tBuild\\n'\n",
      common::sh_path(&capture_file)
    ),
  )?;
  let path = format!(
    "{}:{}",
    temp_dir.path().to_string_lossy(),
    std::env::var("PATH").unwrap_or_default()
  );

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--fzf")
    .arg("--label")
    .arg("area=ci")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&marker_file)?, "build");
  assert_eq!(
    std::fs::read_to_string(&capture_file)?,
    "build \tBuild\ndeploy\tDeploy\n"
  );
  Ok(())
}

#[test]
fn test_run_fzf_single_candidate_does_not_require_backend() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let marker_file = temp_dir.path().join("selected.txt");
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
    tasks:
      build:
        description: Build
        labels:
          area: ci
        commands:
          - command: printf build > {}
            verbose: false
    ",
      common::sh_path(&marker_file)
    ),
  )?;
  let path = std::env::var("PATH").unwrap_or_default();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--fzf")
    .arg("--label")
    .arg("area=ci")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&marker_file)?, "build");
  Ok(())
}

#[test]
fn test_run_fzf_reports_missing_label_match() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        labels:
          area: ci
        commands:
          - command: echo build
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--fzf")
    .arg("--label")
    .arg("area=dev")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "No tasks matched the given label filters",
    ));

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_run_fzf_falls_back_to_sk() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let marker_file = temp_dir.path().join("selected.txt");
  let capture_file = temp_dir.path().join("sk-input.txt");
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
    tasks:
      build:
        description: Build
        commands:
          - command: printf build > {}
            verbose: false
      lint:
        description: Lint
        commands:
          - command: printf lint > {}
            verbose: false
    ",
      common::sh_path(&marker_file),
      common::sh_path(&marker_file)
    ),
  )?;
  write_fake_selector(
    &temp_dir,
    "sk",
    &format!(
      "#!/bin/sh\nwhile IFS= read -r line; do printf '%s\\n' \"$line\"; done > \"{}\"\nprintf 'lint\\tLint\\n'\n",
      common::sh_path(&capture_file)
    ),
  )?;
  write_fake_sh(&temp_dir)?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env("PATH", temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--fzf")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&marker_file)?, "lint");
  assert_eq!(
    std::fs::read_to_string(&capture_file)?,
    "build\tBuild\nlint \tLint\n"
  );
  Ok(())
}

#[cfg(unix)]
#[test]
fn test_run_fzf_reports_missing_backend() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
      lint:
        commands:
          - command: echo lint
            verbose: false
    ",
  )?;
  write_fake_sh(&temp_dir)?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env("PATH", temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--fzf")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "No fuzzy finder found. Install fzf or skim (sk).",
    ));

  Ok(())
}

#[test]
fn test_run_fzf_conflicts_with_task_name() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("hello")
    .arg("--fzf")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "the argument '[TASK_NAME]' cannot be used with '--fzf'",
    ));

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_run_fzf_cancel_returns_no_task_selected() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
      lint:
        commands:
          - command: echo lint
            verbose: false
    ",
  )?;
  write_fake_selector(&temp_dir, "fzf", "#!/bin/sh\nexit 130\n")?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env("PATH", temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--fzf")
    .assert()
    .failure()
    .stderr(predicates::str::contains("No task selected"));

  Ok(())
}

#[test]
fn test_mk_5() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("hello")
    .assert();
  assert.success();
  Ok(())
}

#[test]
fn test_mk_6() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("r")
    .arg("hello")
    .assert();
  assert.success();
  Ok(())
}

#[test]
fn test_mk_7() -> anyhow::Result<()> {
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("u").assert();
  assert
    .failure()
    .code(1)
    .stderr(predicates::str::contains("Task 'u' not found"));
  Ok(())
}

#[test]
fn test_mk_8() -> anyhow::Result<()> {
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("-c").arg("hello.yaml").assert();
  assert
    .failure()
    .code(1)
    .stderr(predicates::str::contains("Config file does not exist"));
  Ok(())
}

#[test]
fn test_mk_9() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("-c").arg(&config_file_path).arg("hello").assert();
  assert.success();
  Ok(())
}

#[test]
fn test_mk_10() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("-c").arg(&config_file_path).arg("hello0").assert();
  assert
    .failure()
    .stderr(predicates::str::contains("Task 'hello0' not found"));
  Ok(())
}

#[test]
fn test_mk_11() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("test")
    .assert();
  assert.failure();
  Ok(())
}

#[test]
fn test_mk_12() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("-c").arg(&config_file_path).arg("run").assert();
  assert.failure();
  Ok(())
}

#[test]
fn test_mk_13() -> anyhow::Result<()> {
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("run").assert();
  assert.failure();
  Ok(())
}

#[test]
fn test_mk_14() -> anyhow::Result<()> {
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("r").assert();
  assert.failure();
  Ok(())
}

#[test]
fn test_mk_15_validate() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("-c").arg(&config_file_path).arg("validate").assert();
  assert
    .success()
    .stdout(predicates::str::contains("Validation passed"));
  Ok(())
}

#[test]
fn test_mk_16_validate_fails_for_missing_dependency() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "invalid.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
        depends_on:
          - missing
    ",
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("-c").arg(&config_file_path).arg("validate").assert();
  assert
    .failure()
    .stderr(predicates::str::contains("Validation failed"))
    .stdout(predicates::str::contains("Missing dependency: missing"));
  Ok(())
}

#[test]
fn test_mk_17_plan() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "plan.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
        depends_on:
          - check
      check:
        commands:
          - command: echo check
            verbose: false
    ",
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("build")
    .assert();
  assert
    .success()
    .stdout(predicates::str::contains("Plan for task: build"))
    .stdout(predicates::str::contains("1. check"))
    .stdout(predicates::str::contains("2. build"));
  Ok(())
}

#[test]
fn test_mk_17_plan_json() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "plan-json.yaml",
    "
    tasks:
      build:
        shell: bash
        commands:
          - command: echo build
            verbose: false
    ",
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("build")
    .arg("--json")
    .assert();
  assert
    .success()
    .stdout(predicates::str::contains("\"root_task\": \"build\""))
    .stdout(predicates::str::contains("\"shell\": \"bash\""));
  Ok(())
}

#[test]
fn test_mk_18_dry_run() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let marker_file = temp_dir.path().join("executed.txt");
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "dry-run.yaml",
    &format!(
      "
    tasks:
      build:
        commands:
          - command: touch {}
            verbose: false
    ",
      marker_file.to_utf8()?
    ),
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .arg("--dry-run")
    .assert();
  assert
    .success()
    .stdout(predicates::str::contains("Plan for task: build"))
    .stdout(predicates::str::contains("local: touch"));
  assert!(!marker_file.exists());
  Ok(())
}

#[test]
fn test_mk_18_dry_run_conflicts_with_json_events() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("hello")
    .arg("--dry-run")
    .arg("--json-events")
    .assert();
  assert
    .failure()
    .stderr(predicate::str::contains("cannot be used with '--json-events'"));
  Ok(())
}

#[test]
fn test_mk_18_init_rejects_unsupported_output() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_path = temp_dir.path().join("mk.txt");
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .current_dir(temp_dir.path())
    .arg("init")
    .arg(output_path.to_utf8()?)
    .assert();
  assert.failure().stderr(predicate::str::contains(
    "Unsupported `mk init` output extension .txt",
  ));
  assert!(!output_path.exists());
  Ok(())
}

#[test]
fn test_mk_18_clean_cache_without_config_uses_cwd() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let cache_dir = temp_dir.path().join(".mk");
  let cache_file = cache_dir.join("cache.json");
  std::fs::create_dir_all(&cache_dir)?;
  std::fs::write(&cache_file, "{}")?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.current_dir(temp_dir.path()).arg("clean-cache").assert();
  assert.success().stdout(predicate::str::contains("Cache cleared"));
  assert!(!cache_file.exists());
  Ok(())
}

#[test]
fn test_mk_18_list_output_is_sorted() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "sorted.yaml",
    "
    tasks:
      zebra:
        commands:
          - command: echo zebra
            verbose: false
      alpha:
        commands:
          - command: echo alpha
            verbose: false
      middle:
        commands:
          - command: echo middle
            verbose: false
    ",
  )?;

  let plain = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("list")
    .arg("--plain")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let plain = String::from_utf8(plain)?;
  let alpha_index = plain.find("alpha").unwrap();
  let middle_index = plain.find("middle").unwrap();
  let zebra_index = plain.find("zebra").unwrap();
  assert!(alpha_index < middle_index);
  assert!(middle_index < zebra_index);

  let json = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("list")
    .arg("--json")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let json: serde_json::Value = serde_json::from_slice(&json)?;
  let names = json
    .as_array()
    .unwrap()
    .iter()
    .map(|task| task["name"].as_str().unwrap())
    .collect::<Vec<_>>();
  assert_eq!(names, vec!["alpha", "middle", "zebra"]);

  Ok(())
}

#[test]
fn test_mk_19_validate_json_cycle() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "cycle.yaml",
    "
    tasks:
      a:
        commands:
          - command: echo a
            verbose: false
        depends_on:
          - b
      b:
        commands:
          - command: echo b
            verbose: false
        depends_on:
          - a
    ",
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .arg("--json")
    .assert();
  assert
    .failure()
    .code(1)
    .stdout(predicates::str::contains("\"severity\": \"error\""))
    .stdout(predicates::str::contains("Circular dependency detected"));
  Ok(())
}

#[test]
fn test_mk_19_update_network_failure_exit_code() -> anyhow::Result<()> {
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .env("MK_UPDATE_URL", "http://127.0.0.1:9/latest")
    .arg("update")
    .assert();
  assert
    .failure()
    .code(1)
    .stdout(predicates::str::contains("Checking for updates..."))
    .stderr(predicates::str::contains(
      "Failed to check for updates. Network unavailable or request timed out",
    ));
  Ok(())
}

#[test]
fn test_mk_19_list_no_color_has_no_ansi_sequences() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let output = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("list")
    .arg("--no-color")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  assert!(stdout.contains("Available tasks:"));
  assert!(!stdout.contains("\u{1b}["));
  Ok(())
}

#[test]
fn test_mk_20_config_discovery_from_dot_mk() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  std::fs::create_dir_all(temp_dir.path().join(".mk"))?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    ".mk/tasks.yaml",
    "
    tasks:
      hello:
        commands:
          - command: echo discovered
            verbose: false
    ",
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.current_dir(temp_dir.path()).arg("hello").assert();
  assert.success();
  assert!(std::path::Path::new(&config_file_path).exists());
  Ok(())
}

#[test]
fn test_mk_21_json_events() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "events.yaml",
    "
    tasks:
      hello:
        commands:
          - command: echo hello
            verbose: false
    ",
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("hello")
    .arg("--json-events")
    .assert();
  assert
    .success()
    .stdout(predicates::str::contains("\"event\":\"task_started\""))
    .stdout(predicates::str::contains("\"event\":\"command_started\""))
    .stdout(predicates::str::contains("\"event\":\"task_finished\""));
  Ok(())
}

#[test]
fn test_mk_22_cache_skips_second_run() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let input_file = temp_dir.path().join("input.txt");
  let output_file = temp_dir.path().join("output.txt");
  let marker_file = temp_dir.path().join("marker.txt");
  std::fs::write(&input_file, "hello")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "cache.yaml",
    &format!(
      "
    tasks:
      build:
        inputs:
          - {}
        outputs:
          - {}
        cache:
          enabled: true
        commands:
          - command: cat {} > {} && echo run >> {}
            verbose: false
    ",
      common::sh_path(&input_file),
      common::sh_path(&output_file),
      common::sh_path(&input_file),
      common::sh_path(&output_file),
      common::sh_path(&marker_file),
    ),
  )?;

  let mut first = Command::new(cargo::cargo_bin!("mk"));
  first
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  let mut second = Command::new(cargo::cargo_bin!("mk"));
  second
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert_eq!(marker.lines().count(), 1);
  Ok(())
}

#[test]
fn test_mk_23_parallel_execution_config_fail_fast() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let marker_file = temp_dir.path().join("should-not-run.txt");
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "parallel.yaml",
    &format!(
      "
    tasks:
      build:
        execution:
          mode: parallel
          max_parallel: 1
          fail_fast: true
        commands:
          - command: false
            verbose: false
          - command: touch {}
            verbose: false
    ",
      marker_file.to_utf8()?,
    ),
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .failure();
  assert!(!marker_file.exists());
  Ok(())
}

#[test]
fn test_mk_23_parallel_execution_fail_fast_cancels_running_commands() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let marker_file = temp_dir.path().join("side.txt");
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "parallel-cancel.yaml",
    &format!(
      "
    tasks:
      build:
        execution:
          mode: parallel
          max_parallel: 2
          fail_fast: true
        commands:
          - command: false
            verbose: false
          - command: sleep 1 && printf side > {}
            verbose: false
    ",
      common::sh_path(&marker_file),
    ),
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .failure();

  assert!(!marker_file.exists());
  Ok(())
}

#[test]
fn test_mk_24_plan_json_runtime_and_parallel() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "runtime-plan.yaml",
    "
    container_runtime: podman
    tasks:
      image:
        execution:
          mode: parallel
          max_parallel: 2
        commands:
          - container_build:
              image_name: example/test
              context: .
              runtime: docker
    ",
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("image")
    .arg("--json")
    .assert();
  assert
    .success()
    .stdout(predicates::str::contains("\"max_parallel\": 2"))
    .stdout(predicates::str::contains("\"runtime\": \"docker\""));
  Ok(())
}

#[test]
fn test_mk_25_rejects_include_configs() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "include.yaml",
    "
    include:
      - shared.yaml
    tasks:
      hello:
        commands:
          - command: echo hello
            verbose: false
    ",
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd.arg("-c").arg(&config_file_path).arg("list").assert();
  assert.failure().stderr(predicates::str::contains(
    "`include` is no longer supported. Use `extends` instead.",
  ));
  Ok(())
}

#[test]
fn test_mk_26_rejects_extends_cycles() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let a_path = common::setup_yaml(
    &temp_dir,
    "a.yaml",
    "
    extends: b.yaml
    tasks:
      a:
        commands:
          - command: echo a
            verbose: false
    ",
  )?;
  let _b_path = common::setup_yaml(
    &temp_dir,
    "b.yaml",
    "
    extends: a.yaml
    tasks:
      b:
        commands:
          - command: echo b
            verbose: false
    ",
  )?;
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&a_path)
    .arg("list")
    .assert();
  assert
    .failure()
    .stderr(predicates::str::contains("Circular extends detected:"));
  Ok(())
}

#[test]
fn test_mk_27_cache_paths_resolve_from_task_work_dir() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let workspace = temp_dir.path().join("workspace");
  std::fs::create_dir_all(&workspace)?;
  let input_file = workspace.join("input.txt");
  let output_file = workspace.join("output.txt");
  let marker_file = workspace.join("marker.txt");
  std::fs::write(&input_file, "hello")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "cache-workdir.yaml",
    "
    tasks:
      build:
        inputs:
          - input.txt
        outputs:
          - output.txt
        cache:
          enabled: true
        commands:
          - command: cat input.txt > output.txt && echo run >> marker.txt
            work_dir: workspace
            verbose: false
    ",
  )?;

  let mut first = Command::new(cargo::cargo_bin!("mk"));
  first
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  let mut second = Command::new(cargo::cargo_bin!("mk"));
  second
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert_eq!(marker.lines().count(), 1);
  assert_eq!(std::fs::read_to_string(&output_file)?, "hello");
  Ok(())
}

#[test]
fn test_mk_28_env_file_content_invalidates_cache() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let env_file = temp_dir.path().join(".env");
  let output_file = temp_dir.path().join("output.txt");
  let marker_file = temp_dir.path().join("marker.txt");
  std::fs::write(&env_file, "# one\nFOO=bar\n")?;
  let shell = common::portable_test_shell();
  let build_command = if cfg!(windows) {
    "[System.IO.File]::WriteAllText('output.txt', $env:FOO); [System.IO.File]::AppendAllText('marker.txt', \"run`n\")"
  } else {
    "printf '%s' \"$FOO\" > output.txt && echo run >> marker.txt"
  };

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "env-cache.yaml",
    &format!(
      "
    tasks:
      build:
        env_file:
          - .env
        outputs:
          - output.txt
        cache:
          enabled: true
        shell: {shell}
        commands:
          - command: {build_command}
            verbose: false
    "
    ),
  )?;

  let mut first = Command::new(cargo::cargo_bin!("mk"));
  first
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  std::fs::write(&env_file, "# two\nFOO=bar\n")?;

  let mut second = Command::new(cargo::cargo_bin!("mk"));
  second
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert_eq!(marker.lines().count(), 2);
  assert_eq!(std::fs::read_to_string(&output_file)?, "bar");
  Ok(())
}

#[test]
fn test_mk_28_cache_output_content_drift_invalidates_cache() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_file = temp_dir.path().join("output.txt");
  let marker_file = temp_dir.path().join("marker.txt");
  let shell = common::portable_test_shell();
  let build_command = if cfg!(windows) {
    "[System.IO.File]::WriteAllText('output.txt', 'fresh'); [System.IO.File]::AppendAllText('marker.txt', \"run`n\")"
  } else {
    "printf fresh > output.txt && echo run >> marker.txt"
  };

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "output-drift-cache.yaml",
    &format!(
      "
    tasks:
      build:
        outputs:
          - output.txt
        cache:
          enabled: true
        shell: {shell}
        commands:
          - command: {build_command}
            verbose: false
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  std::fs::write(&output_file, "tampered")?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert_eq!(marker.lines().count(), 2);
  assert_eq!(std::fs::read_to_string(&output_file)?, "fresh");
  Ok(())
}

#[test]
fn test_mk_28_cache_missing_output_invalidates_cache() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_file = temp_dir.path().join("output.txt");
  let marker_file = temp_dir.path().join("marker.txt");
  let shell = common::portable_test_shell();
  let build_command = if cfg!(windows) {
    "[System.IO.File]::WriteAllText('output.txt', 'fresh'); [System.IO.File]::AppendAllText('marker.txt', \"run`n\")"
  } else {
    "printf fresh > output.txt && echo run >> marker.txt"
  };

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "missing-output-cache.yaml",
    &format!(
      "
    tasks:
      build:
        outputs:
          - output.txt
        cache:
          enabled: true
        shell: {shell}
        commands:
          - command: {build_command}
            verbose: false
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  std::fs::remove_file(&output_file)?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert_eq!(marker.lines().count(), 2);
  assert!(output_file.exists());
  Ok(())
}

#[test]
fn test_mk_28_directory_input_changes_invalidate_cache() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let input_dir = temp_dir.path().join("input");
  let nested_dir = input_dir.join("nested");
  let marker_file = temp_dir.path().join("marker.txt");
  std::fs::create_dir_all(&nested_dir)?;
  std::fs::write(nested_dir.join("file.txt"), "one")?;

  let shell = common::portable_test_shell();
  let build_command = if cfg!(windows) {
    "[System.IO.File]::WriteAllText('output.txt', 'fresh'); [System.IO.File]::AppendAllText('marker.txt', \"run`n\")"
  } else {
    "printf fresh > output.txt && echo run >> marker.txt"
  };

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "directory-input-cache.yaml",
    &format!(
      "
    tasks:
      build:
        inputs:
          - input
        outputs:
          - output.txt
        cache:
          enabled: true
        shell: {shell}
        commands:
          - command: {build_command}
            verbose: false
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  std::fs::write(nested_dir.join("file.txt"), "two")?;
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  std::fs::write(input_dir.join("added.txt"), "three")?;
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  std::fs::remove_file(input_dir.join("added.txt"))?;
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert_eq!(marker.lines().count(), 4);
  Ok(())
}

#[cfg(unix)]
#[test]
fn test_mk_29_container_runtime_inherits_root_default_at_execution() -> anyhow::Result<()> {
  use std::os::unix::fs::PermissionsExt as _;

  let temp_dir = TempDir::new()?;
  let podman_path = temp_dir.path().join("podman");
  let docker_path = temp_dir.path().join("docker");
  let marker_file = temp_dir.path().join("runtime.txt");

  std::fs::write(
    &podman_path,
    format!(
      "#!/bin/sh\nprintf 'podman %s\\n' \"$*\" > {}\n",
      marker_file.to_str().unwrap()
    ),
  )?;
  std::fs::write(
    &docker_path,
    format!(
      "#!/bin/sh\nprintf 'docker %s\\n' \"$*\" > {}\n",
      marker_file.to_str().unwrap()
    ),
  )?;
  std::fs::set_permissions(&podman_path, std::fs::Permissions::from_mode(0o755))?;
  std::fs::set_permissions(&docker_path, std::fs::Permissions::from_mode(0o755))?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "runtime-exec.yaml",
    "
    container_runtime: podman
    tasks:
      hello:
        commands:
          - image: docker.io/library/bash:latest
            container_command:
              - echo
              - hello
            verbose: false
    ",
  )?;

  let path = format!(
    "{}:{}",
    temp_dir.path().to_str().unwrap(),
    std::env::var("PATH").unwrap_or_default()
  );

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("hello")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert!(
    marker.starts_with("podman "),
    "expected podman invocation, got: {}",
    marker
  );
  Ok(())
}

#[test]
fn test_mk_30_cache_file_is_stored_under_config_root() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  std::fs::create_dir_all(temp_dir.path().join("nested"))?;
  let input_file = temp_dir.path().join("nested/input.txt");
  std::fs::write(&input_file, "hello")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "nested/tasks.yaml",
    "
    tasks:
      build:
        inputs:
          - input.txt
        outputs:
          - output.txt
        cache:
          enabled: true
        commands:
          - command: cat input.txt > output.txt
            work_dir: .
            verbose: false
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  assert!(temp_dir.path().join("nested/.mk/cache.json").exists());
  assert!(!temp_dir.path().join(".mk/cache.json").exists());
  Ok(())
}

#[test]
fn test_mk_31_validate_resolves_relative_paths_from_config_dir() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  std::fs::create_dir_all(temp_dir.path().join("nested/app"))?;
  std::fs::write(temp_dir.path().join("nested/package.json"), "{}")?;
  std::fs::write(temp_dir.path().join("nested/app/Containerfile"), "FROM scratch\n")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "nested/tasks.yaml",
    "
    use_npm: true
    use_cargo:
      work_dir: app
    tasks:
      image:
        commands:
          - container_build:
              image_name: example/test
              context: app
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .success()
    .stdout(predicates::str::contains("Validation passed"));

  Ok(())
}

#[test]
fn test_mk_31_validate_warns_about_cached_dependency_side_effects_without_inputs() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "validate-cache-deps.yaml",
    "
    tasks:
      dep:
        commands:
          - command: echo dep
            verbose: false
      build:
        depends_on:
          - dep
        outputs:
          - output.txt
        cache:
          enabled: true
        commands:
          - command: printf done > output.txt
            verbose: false
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .success()
    .stdout(predicates::str::contains(
      "Cached task depends_on other tasks but declares no inputs; dependency side effects may bypass cache invalidation",
    ));

  Ok(())
}

#[test]
fn test_mk_32_plan_reports_effective_base_dir() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  std::fs::create_dir_all(temp_dir.path().join("nested").join("work"))?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "nested/tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo hello
            work_dir: work
            verbose: false
    ",
  )?;

  let expected_base_dir = temp_dir.path().join("nested").join("work");

  let mut json_cmd = Command::new(cargo::cargo_bin!("mk"));
  json_cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("build")
    .arg("--json")
    .assert()
    .success()
    .stdout(predicates::str::contains(format!(
      "\"base_dir\": \"{}\"",
      expected_base_dir.to_string_lossy().replace('\\', "\\\\")
    )));

  let mut text_cmd = Command::new(cargo::cargo_bin!("mk"));
  text_cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("build")
    .assert()
    .success()
    .stdout(predicates::str::contains(format!(
      "base_dir: {}",
      expected_base_dir.to_string_lossy()
    )));

  Ok(())
}

#[test]
fn test_mk_33_local_run_and_precondition_resolve_work_dir_from_config_dir() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  std::fs::create_dir_all(temp_dir.path().join("nested/work"))?;
  let marker_file = temp_dir.path().join("nested/work/marker.txt");

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "nested/tasks.yaml",
    "
    tasks:
      build:
        preconditions:
          - command: test -f input.txt
            work_dir: work
            verbose: false
        commands:
          - command: printf 'ok' > marker.txt
            work_dir: work
            verbose: false
    ",
  )?;

  std::fs::write(temp_dir.path().join("nested/work/input.txt"), "hello")?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&marker_file)?, "ok");
  Ok(())
}

#[test]
fn test_mk_33_cache_still_runs_dependencies_before_cache_hit() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let dep_file = temp_dir.path().join("dep.txt");
  let input_file = temp_dir.path().join("input.txt");
  let output_file = temp_dir.path().join("output.txt");
  std::fs::write(&input_file, "hello")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "cache-deps.yaml",
    &format!(
      "
    tasks:
      dep:
        outputs:
          - {}
        commands:
          - command: printf dep > {}
            verbose: false
      build:
        depends_on:
          - dep
        inputs:
          - {}
        outputs:
          - {}
        cache:
          enabled: true
        commands:
          - command: cat {} > {}
            verbose: false
    ",
      common::sh_path(&dep_file),
      common::sh_path(&dep_file),
      common::sh_path(&input_file),
      common::sh_path(&output_file),
      common::sh_path(&input_file),
      common::sh_path(&output_file),
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  std::fs::remove_file(&dep_file)?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  assert!(dep_file.exists());
  Ok(())
}

#[test]
fn test_mk_33_cache_still_runs_preconditions_before_cache_hit() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let gate_file = temp_dir.path().join("gate.txt");
  let input_file = temp_dir.path().join("input.txt");
  let output_file = temp_dir.path().join("output.txt");
  std::fs::write(&gate_file, "ok")?;
  std::fs::write(&input_file, "hello")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "cache-precondition.yaml",
    &format!(
      "
    tasks:
      build:
        inputs:
          - {}
        outputs:
          - {}
        cache:
          enabled: true
        preconditions:
          - command: test -f {}
            verbose: false
        commands:
          - command: cat {} > {}
            verbose: false
    ",
      common::sh_path(&input_file),
      common::sh_path(&output_file),
      common::sh_path(&gate_file),
      common::sh_path(&input_file),
      common::sh_path(&output_file),
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  std::fs::remove_file(&gate_file)?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .failure()
    .stderr(predicates::str::contains("Precondition failed"));

  Ok(())
}

#[test]
fn test_mk_33_default_config_discovery_supports_tasks_json() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  std::fs::write(
    temp_dir.path().join("tasks.json"),
    r#"{
  "tasks": {
    "hello": {
      "commands": [
        {
          "command": "echo hello",
          "verbose": false
        }
      ]
    }
  }
}"#,
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("hello")
    .assert()
    .success();

  Ok(())
}

#[test]
fn test_mk_33_default_config_discovery_supports_tasks_toml() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  std::fs::write(
    temp_dir.path().join("tasks.toml"),
    r#"[tasks.hello]
commands = [{ command = "echo hello", verbose = false }]
"#,
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("hello")
    .assert()
    .success();

  Ok(())
}

#[test]
fn test_mk_33_default_config_discovery_supports_tasks_lua() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  std::fs::write(
    temp_dir.path().join("tasks.lua"),
    r#"return {
  tasks = {
    hello = {
      commands = {
        { command = "echo hello", verbose = false }
      }
    }
  }
}"#,
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("hello")
    .assert()
    .success();

  Ok(())
}

#[test]
fn test_mk_33_env_file_expands_home_directory() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let home_dir = temp_dir.path().join("home");
  let workspace_dir = temp_dir.path().join("workspace");
  std::fs::create_dir_all(&home_dir)?;
  std::fs::create_dir_all(&workspace_dir)?;
  std::fs::write(home_dir.join(".mk-test-env"), "HOME_VALUE=expanded\n")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "workspace/tasks.yaml",
    "
    tasks:
      hello:
        env_file:
          - ~/.mk-test-env
        commands:
          - command: test \"$HOME_VALUE\" = expanded
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(&workspace_dir)
    .env("HOME", &home_dir)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("hello")
    .assert()
    .success();

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_mk_34_container_build_resolves_context_and_containerfile_from_config_dir() -> anyhow::Result<()> {
  use std::os::unix::fs::PermissionsExt as _;

  let temp_dir = TempDir::new()?;
  std::fs::create_dir_all(temp_dir.path().join("nested/buildctx"))?;
  let podman_path = temp_dir.path().join("podman");
  let marker_file = temp_dir.path().join("build-args.txt");
  std::fs::write(
    temp_dir.path().join("nested/buildctx/Customfile"),
    "FROM scratch\n",
  )?;

  std::fs::write(
    &podman_path,
    format!(
      "#!/bin/sh\nprintf '%s\\n' \"$*\" > {}\n",
      marker_file.to_string_lossy()
    ),
  )?;
  std::fs::set_permissions(&podman_path, std::fs::Permissions::from_mode(0o755))?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "nested/tasks.yaml",
    "
    container_runtime: podman
    tasks:
      image:
        commands:
          - container_build:
              image_name: example/test
              context: buildctx
              containerfile: buildctx/Customfile
    ",
  )?;

  let path = format!(
    "{}:{}",
    temp_dir.path().to_string_lossy(),
    std::env::var("PATH").unwrap_or_default()
  );

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("image")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert!(marker.contains(
    &temp_dir
      .path()
      .join("nested/buildctx")
      .to_string_lossy()
      .into_owned()
  ));
  assert!(marker.contains(
    &temp_dir
      .path()
      .join("nested/buildctx/Customfile")
      .to_string_lossy()
      .into_owned()
  ));

  let mut plan_cmd = Command::new(cargo::cargo_bin!("mk"));
  plan_cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("image")
    .arg("--json")
    .assert()
    .success()
    .stdout(predicates::str::contains(format!(
      "\"context\": \"{}\"",
      temp_dir.path().join("nested/buildctx").to_string_lossy()
    )))
    .stdout(predicates::str::contains(format!(
      "\"containerfile\": \"{}\"",
      temp_dir
        .path()
        .join("nested/buildctx/Customfile")
        .to_string_lossy()
    )));

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_mk_35_container_run_resolves_relative_mount_host_paths_from_config_dir() -> anyhow::Result<()> {
  use std::os::unix::fs::PermissionsExt as _;

  let temp_dir = TempDir::new()?;
  std::fs::create_dir_all(temp_dir.path().join("nested/data"))?;
  let podman_path = temp_dir.path().join("podman");
  let marker_file = temp_dir.path().join("run-args.txt");

  std::fs::write(
    &podman_path,
    format!(
      "#!/bin/sh\nprintf '%s\\n' \"$*\" > {}\n",
      marker_file.to_string_lossy()
    ),
  )?;
  std::fs::set_permissions(&podman_path, std::fs::Permissions::from_mode(0o755))?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "nested/tasks.yaml",
    "
    container_runtime: podman
    tasks:
      hello:
        commands:
          - image: docker.io/library/bash:latest
            container_command:
              - echo
              - hello
            mounted_paths:
              - ./data:/data:ro,z
    ",
  )?;

  let path = format!(
    "{}:{}",
    temp_dir.path().to_string_lossy(),
    std::env::var("PATH").unwrap_or_default()
  );

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("hello")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert!(
    marker.contains(&format!(
      "{}:/workdir:z",
      temp_dir.path().join("nested").to_string_lossy()
    )),
    "expected config-root workdir mount, got: {}",
    marker
  );
  assert!(
    marker.contains(&format!(
      "{}:/data:ro,z",
      temp_dir.path().join("nested/data").to_string_lossy()
    )),
    "expected resolved relative host mount, got: {}",
    marker
  );

  let mut plan_cmd = Command::new(cargo::cargo_bin!("mk"));
  plan_cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("hello")
    .arg("--json")
    .assert()
    .success()
    .stdout(predicates::str::contains(format!(
      "\"{}:/data:ro,z\"",
      temp_dir.path().join("nested/data").to_string_lossy()
    )));

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_mk_36_container_build_supports_explicit_nerdctl_runtime() -> anyhow::Result<()> {
  use std::os::unix::fs::PermissionsExt as _;

  let temp_dir = TempDir::new()?;
  let nerdctl_path = temp_dir.path().join("nerdctl");
  let marker_file = temp_dir.path().join("nerdctl-build-args.txt");
  std::fs::write(temp_dir.path().join("Containerfile"), "FROM scratch\n")?;

  std::fs::write(
    &nerdctl_path,
    format!(
      "#!/bin/sh\nprintf '%s\\n' \"$0 $*\" > {}\n",
      marker_file.to_string_lossy()
    ),
  )?;
  std::fs::set_permissions(&nerdctl_path, std::fs::Permissions::from_mode(0o755))?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      image:
        commands:
          - container_build:
              image_name: example/test
              context: .
              runtime: nerdctl
    ",
  )?;

  let path = format!(
    "{}:{}",
    temp_dir.path().to_string_lossy(),
    std::env::var("PATH").unwrap_or_default()
  );

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("image")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert!(
    marker.contains("nerdctl build"),
    "expected nerdctl runtime, got: {}",
    marker
  );

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_mk_37_container_build_auto_falls_back_to_nerdctl() -> anyhow::Result<()> {
  use std::os::unix::fs::PermissionsExt as _;

  let temp_dir = TempDir::new()?;
  let nerdctl_path = temp_dir.path().join("nerdctl");
  let marker_file = temp_dir.path().join("nerdctl-auto-build-args.txt");
  std::fs::write(temp_dir.path().join("Containerfile"), "FROM scratch\n")?;

  std::fs::write(
    &nerdctl_path,
    format!(
      "#!/bin/sh\nprintf '%s\\n' \"$0 $*\" > {}\n",
      marker_file.to_string_lossy()
    ),
  )?;
  std::fs::set_permissions(&nerdctl_path, std::fs::Permissions::from_mode(0o755))?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      image:
        commands:
          - container_build:
              image_name: example/test
              context: .
    ",
  )?;

  let path = format!("{}", temp_dir.path().to_string_lossy(),);

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("image")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert!(
    marker.contains("nerdctl build"),
    "expected auto runtime to use nerdctl, got: {}",
    marker
  );

  Ok(())
}

#[test]
fn test_mk_38_repo_pack_task_uses_native_container_build() -> anyhow::Result<()> {
  let repo_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(repo_dir)
    .arg("-c")
    .arg(repo_dir.join("tasks.yaml"))
    .arg("plan")
    .arg("pack")
    .arg("--json")
    .assert()
    .success()
    .stdout(predicates::str::contains("\"type\": \"container_build\""))
    .stdout(predicates::str::contains("\"runtime\": \"auto\""))
    .stdout(predicates::str::contains(
      "\"image_name\": \"ghcr.io/ffimnsr/mk-rs\"",
    ))
    .stdout(predicates::str::contains("\"tags\": ["))
    .stdout(predicates::str::contains("\"${{ env.VERSION }}\""));

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_mk_39_container_run_preserves_named_volumes() -> anyhow::Result<()> {
  use std::os::unix::fs::PermissionsExt as _;

  let temp_dir = TempDir::new()?;
  std::fs::create_dir_all(temp_dir.path().join("nested"))?;
  let podman_path = temp_dir.path().join("podman");
  let marker_file = temp_dir.path().join("named-volume-args.txt");

  std::fs::write(
    &podman_path,
    format!(
      "#!/bin/sh\nprintf '%s\\n' \"$*\" > {}\n",
      marker_file.to_string_lossy()
    ),
  )?;
  std::fs::set_permissions(&podman_path, std::fs::Permissions::from_mode(0o755))?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "nested/tasks.yaml",
    "
    container_runtime: podman
    tasks:
      hello:
        commands:
          - image: docker.io/library/bash:latest
            container_command:
              - echo
              - hello
            mounted_paths:
              - cache:/data
    ",
  )?;

  let path = format!(
    "{}:{}",
    temp_dir.path().to_string_lossy(),
    std::env::var("PATH").unwrap_or_default()
  );

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("hello")
    .assert()
    .success();

  let marker = std::fs::read_to_string(&marker_file)?;
  assert!(
    marker.contains("cache:/data"),
    "expected named volume to remain unchanged, got: {}",
    marker
  );
  assert!(
    !marker.contains(
      &temp_dir
        .path()
        .join("nested/cache")
        .to_string_lossy()
        .into_owned()
    ),
    "named volume was incorrectly rewritten: {}",
    marker
  );

  let mut plan_cmd = Command::new(cargo::cargo_bin!("mk"));
  plan_cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("hello")
    .arg("--json")
    .assert()
    .success()
    .stdout(predicates::str::contains("\"cache:/data\""));

  Ok(())
}

#[test]
fn test_mk_37_save_and_reuse_command_output() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let result_file = temp_dir.path().join("result.txt");
  let shell = common::portable_test_shell();
  let version_command = if cfg!(windows) {
    "Write-Output '1.2.3'"
  } else {
    "printf '1.2.3\\n'"
  };
  let result_command = if cfg!(windows) {
    format!(
      "[System.IO.File]::WriteAllText('{}', '${{{{ outputs.version }}}}|' + $env:IMAGE_TAG)",
      common::sh_path(&result_file)
    )
  } else {
    format!(
      "printf '%s|%s' \"${{{{ outputs.version }}}}\" \"$IMAGE_TAG\" > {}",
      common::sh_path(&result_file)
    )
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "outputs.yaml",
    &format!(
      "
    tasks:
      build:
        shell: {shell}
        environment:
          IMAGE_TAG: tag-${{{{ outputs.version }}}}
        commands:
          - command: {version_command}
            save_output_as: version
            verbose: false
          - command: {result_command}
            verbose: false
    "
    ),
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&result_file)?, "1.2.3|tag-1.2.3");
  Ok(())
}

#[test]
fn test_mk_38_capture_multiline_output_trims_trailing_newlines() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let result_file = temp_dir.path().join("multiline.txt");
  let shell = common::portable_test_shell();
  let capture_command = if cfg!(windows) {
    "[Console]::Write(\"line1`nline2`n`n\")"
  } else {
    "printf 'line1\\nline2\\n\\n'"
  };
  let result_command = if cfg!(windows) {
    format!(
      "[System.IO.File]::WriteAllText('{}', '${{{{ outputs.block }}}}')",
      common::sh_path(&result_file)
    )
  } else {
    format!(
      "printf '%s' \"${{{{ outputs.block }}}}\" > {}",
      common::sh_path(&result_file)
    )
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "multiline-output.yaml",
    &format!(
      "
    tasks:
      build:
        shell: {shell}
        commands:
          - command: |
              {capture_command}
            save_output_as: block
            verbose: false
          - command: {result_command}
            verbose: false
    "
    ),
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&result_file)?, "line1\nline2");
  Ok(())
}

#[test]
fn test_mk_39_failed_command_does_not_publish_output() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "failed-output.yaml",
    "
    tasks:
      build:
        commands:
          - command: printf 'broken' && false
            save_output_as: version
            ignore_errors: true
            verbose: false
          - command: printf '%s' \"${{ outputs.version }}\"
            verbose: false
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("build")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "Task output 'version' is not available",
    ));

  Ok(())
}

#[test]
fn test_mk_40_nested_tasks_have_isolated_outputs() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let parent_file = temp_dir.path().join("parent.txt");
  let child_file = temp_dir.path().join("child.txt");
  let shell = common::portable_test_shell();
  let parent_capture_command = if cfg!(windows) {
    "Write-Output 'parent'"
  } else {
    "printf 'parent\\n'"
  };
  let child_capture_command = if cfg!(windows) {
    "Write-Output 'child'"
  } else {
    "printf 'child\\n'"
  };
  let parent_write_command = if cfg!(windows) {
    format!(
      "[System.IO.File]::WriteAllText('{}', '${{{{ outputs.shared }}}}')",
      common::sh_path(&parent_file)
    )
  } else {
    format!(
      "printf '%s' \"${{{{ outputs.shared }}}}\" > {}",
      common::sh_path(&parent_file)
    )
  };
  let child_write_command = if cfg!(windows) {
    format!(
      "[System.IO.File]::WriteAllText('{}', '${{{{ outputs.shared }}}}')",
      common::sh_path(&child_file)
    )
  } else {
    format!(
      "printf '%s' \"${{{{ outputs.shared }}}}\" > {}",
      common::sh_path(&child_file)
    )
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "nested-output.yaml",
    &format!(
      "
    tasks:
      root:
        shell: {shell}
        commands:
          - command: {parent_capture_command}
            save_output_as: shared
            verbose: false
          - task: child
          - command: {parent_write_command}
            verbose: false
      child:
        shell: {shell}
        commands:
          - command: {child_capture_command}
            save_output_as: shared
            verbose: false
          - command: {child_write_command}
            verbose: false
    "
    ),
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("root")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&parent_file)?, "parent");
  assert_eq!(std::fs::read_to_string(&child_file)?, "child");
  Ok(())
}

#[test]
fn test_mk_41_validate_rejects_duplicate_saved_outputs() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "duplicate-output.yaml",
    "
    tasks:
      build:
        commands:
          - command: printf 'one'
            save_output_as: version
            verbose: false
          - command: printf 'two'
            save_output_as: version
            verbose: false
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .failure()
    .stdout(predicates::str::contains("Duplicate saved output name: version"));

  Ok(())
}

#[test]
fn test_mk_42_validate_rejects_forward_output_reference() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "forward-output.yaml",
    "
    tasks:
      build:
        commands:
          - command: printf '%s' \"${{ outputs.version }}\"
            verbose: false
          - command: printf '1.2.3'
            save_output_as: version
            verbose: false
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .failure()
    .stdout(predicates::str::contains(
      "Output reference must come from an earlier command: version",
    ));

  Ok(())
}

#[test]
fn test_mk_43_validate_rejects_unknown_output_reference_in_environment() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "unknown-output-env.yaml",
    "
    tasks:
      build:
        environment:
          IMAGE_TAG: ${{ outputs.version }}
        commands:
          - command: printf 'ok'
            verbose: false
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .failure()
    .stdout(predicates::str::contains(
      "Unknown task output reference: version",
    ));

  Ok(())
}

#[test]
fn test_mk_44_validate_rejects_parallel_saved_outputs() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "parallel-output.yaml",
    "
    tasks:
      build:
        execution:
          mode: parallel
        commands:
          - command: printf '1.2.3'
            save_output_as: version
            verbose: false
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .failure()
    .stdout(predicates::str::contains(
      "Parallel execution does not support saved command outputs",
    ));

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_mk_45_ssh_run_captures_output_and_exit_code() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  write_fake_ssh(&temp_dir)?;
  let args_file = temp_dir.path().join("ssh-args.txt");
  let remote_dir = temp_dir.path().join("remote-work");
  std::fs::create_dir_all(&remote_dir)?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "ssh-run.yaml",
    &format!(
      "
    tasks:
      remote:
        commands:
          - ssh_run:
              host: buildbox
              user: deploy
              port: 2222
              options:
                - BatchMode=yes
              work_dir: {}
              command: pwd
              save_output_as: remote_pwd
              save_exit_code_as: remote_code
            verbose: false
          - command: printf '%s|%s' \"${{{{ outputs.remote_pwd }}}}\" \"${{{{ outputs.remote_code }}}}\"
            verbose: false
    ",
      remote_dir.to_string_lossy()
    ),
  )?;

  let path = format!("{}:{}", temp_dir.path().to_utf8()?, std::env::var("PATH")?);
  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .env("MK_TEST_SSH_ARGS_FILE", &args_file)
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("remote")
    .assert()
    .success()
    .stdout(predicates::str::contains(format!(
      "{}|0",
      remote_dir.to_string_lossy()
    )));

  let args = std::fs::read_to_string(&args_file)?;
  assert!(args.contains("-T"));
  assert!(args.contains("-p"));
  assert!(args.contains("2222"));
  assert!(args.contains("-o"));
  assert!(args.contains("BatchMode=yes"));
  assert!(args.contains("deploy@buildbox"));

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_mk_46_validate_rejects_duplicate_saved_outputs_from_ssh_run() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "ssh-duplicate-output.yaml",
    "
    tasks:
      remote:
        commands:
          - ssh_run:
              host: buildbox
              command: printf 'one'
              save_output_as: shared
            verbose: false
          - command: printf 'two'
            save_output_as: shared
            verbose: false
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  cmd
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .failure()
    .stdout(predicates::str::contains("Duplicate saved output name: shared"));

  Ok(())
}

#[test]
fn test_mk_47_secrets_rejects_conflicting_key_name_and_gpg_key_id() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("--key-name")
    .arg("default")
    .arg("--gpg-key-id")
    .arg("ABC123")
    .arg("list-keys")
    .assert();

  assert
    .failure()
    .code(2)
    .stderr(predicates::str::contains("--key-name"))
    .stderr(predicates::str::contains("--gpg-key-id"))
    .stderr(predicates::str::contains("cannot be used with"));
  Ok(())
}

#[test]
fn test_mk_46_secrets_vault_show_rejects_conflicting_key_name_and_gpg_key_id() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  let vault_dir = temp_dir.path().join("vault");
  let keys_dir = temp_dir.path().join("keys");

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("show-secret")
    .arg("app/token")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--keys-location")
    .arg(&keys_dir)
    .arg("--key-name")
    .arg("default")
    .arg("--gpg-key-id")
    .arg("ABC123")
    .assert();

  assert
    .failure()
    .code(2)
    .stderr(predicates::str::contains("--key-name"))
    .stderr(predicates::str::contains("--gpg-key-id"))
    .stderr(predicates::str::contains("cannot be used with"));
  Ok(())
}

#[test]
fn test_mk_47_secrets_vault_show_reports_missing_vault() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  let vault_dir = temp_dir.path().join("missing-vault");
  let keys_dir = temp_dir.path().join("keys");

  let mut cmd = Command::new(cargo::cargo_bin!("mk"));
  let assert = cmd
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("show-secret")
    .arg("app/token")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--keys-location")
    .arg(&keys_dir)
    .assert();

  assert
    .failure()
    .code(1)
    .stderr(predicates::str::contains("Vault not found at '"))
    .stderr(predicates::str::contains(
      "Initialize it first with: mk secrets vault init",
    ));
  Ok(())
}

#[test]
fn test_mk_48_secrets_vault_purge_requires_yes() -> anyhow::Result<()> {
  let (temp_dir, config_file_path, vault_dir, _keys_dir) =
    setup_secrets_fixture("app/token", "secret-value")?;
  let secret_dir = vault_dir.join("app/token");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("purge-secret")
    .arg("app/token")
    .arg("--vault-location")
    .arg(&vault_dir)
    .assert()
    .failure()
    .code(1)
    .stderr(predicates::str::contains(
      "Refusing to delete secret 'app/token' without --yes",
    ));

  assert!(secret_dir.exists());

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("purge-secret")
    .arg("app/token")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--yes")
    .assert()
    .success()
    .stdout(predicates::str::contains(
      "Secret 'app/token' removed from vault.",
    ));

  assert!(!secret_dir.exists());
  Ok(())
}

#[test]
fn test_mk_49_secrets_vault_list_plain_is_scriptable() -> anyhow::Result<()> {
  let (temp_dir, config_file_path, vault_dir, _keys_dir) =
    setup_secrets_fixture("app/token", "secret-value")?;

  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("list-secrets")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--plain")
    .output()?;

  assert!(output.status.success());
  assert_eq!(String::from_utf8(output.stdout)?, "app/token\n");
  Ok(())
}

#[test]
fn test_mk_50_secrets_vault_show_plain_is_scriptable() -> anyhow::Result<()> {
  let (temp_dir, config_file_path, vault_dir, keys_dir) = setup_secrets_fixture("app/token", "secret-value")?;

  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("show-secret")
    .arg("app/token")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--keys-location")
    .arg(&keys_dir)
    .arg("--plain")
    .output()?;

  assert!(output.status.success());
  assert_eq!(String::from_utf8(output.stdout)?, "secret-value");
  Ok(())
}

#[test]
fn test_mk_51_secrets_vault_export_output_has_no_trailing_newline() -> anyhow::Result<()> {
  let (temp_dir, config_file_path, vault_dir, keys_dir) = setup_secrets_fixture("app/token", "secret-value")?;
  let output_path = temp_dir.path().join("secret.txt");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("export-secret")
    .arg("app/token")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--keys-location")
    .arg(&keys_dir)
    .arg("--output")
    .arg(&output_path)
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&output_path)?, "secret-value");
  Ok(())
}

#[test]
fn test_mk_52_secrets_vault_init_warns_before_overwriting_gpg_metadata() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  let vault_dir = temp_dir.path().join("vault");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("init-vault")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--gpg-key-id")
    .arg("OLDKEY")
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("init-vault")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--gpg-key-id")
    .arg("NEWKEY")
    .assert()
    .success()
    .stderr(predicates::str::contains(
      "Warning: vault GPG key changed from 'OLDKEY' to 'NEWKEY'. Overwriting metadata.",
    ));

  assert_eq!(read_vault_gpg_key_id(&vault_dir), Some("NEWKEY".to_string()));
  Ok(())
}

#[test]
fn test_mk_53_secrets_vault_list_runs_without_config_file() -> anyhow::Result<()> {
  let (temp_dir, config_file_path, vault_dir, _keys_dir) =
    setup_secrets_fixture("app/token", "secret-value")?;
  std::fs::remove_file(&config_file_path)?;

  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("secrets")
    .arg("vault")
    .arg("list-secrets")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--plain")
    .output()?;

  assert!(output.status.success());
  assert_eq!(String::from_utf8(output.stdout)?, "app/token\n");
  Ok(())
}

#[test]
fn test_mk_53_hydra_svls_lists_vault_secrets() -> anyhow::Result<()> {
  let (temp_dir, config_file_path, vault_dir, _keys_dir) =
    setup_secrets_fixture("app/token", "secret-value")?;
  std::fs::remove_file(&config_file_path)?;

  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("svls")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--plain")
    .output()?;

  assert!(output.status.success());
  assert_eq!(String::from_utf8(output.stdout)?, "app/token\n");
  Ok(())
}

#[test]
fn test_mk_54_secrets_explicit_missing_config_still_fails() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg("missing.yaml")
    .arg("secrets")
    .arg("vault")
    .arg("list-secrets")
    .arg("--vault-location")
    .arg(temp_dir.path().join("vault"))
    .assert()
    .failure()
    .code(1)
    .stderr(predicates::str::contains("Config file does not exist"));

  Ok(())
}

#[test]
fn test_mk_55_secrets_commands_use_config_defaults() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    secrets:
      vault_location: vault
      keys_location: keys
      key_name: default
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("secrets")
    .arg("key")
    .arg("generate-key")
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("secrets")
    .arg("vault")
    .arg("init-vault")
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("secrets")
    .arg("vault")
    .arg("store-secret")
    .arg("app/token")
    .arg("secret-value")
    .assert()
    .success();

  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("secrets")
    .arg("vault")
    .arg("list-secrets")
    .arg("--plain")
    .output()?;

  assert!(
    output.status.success(),
    "command failed with stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(String::from_utf8(output.stdout)?, "app/token\n");
  assert!(temp_dir.path().join("vault").exists());
  assert!(temp_dir.path().join("keys/default.key").exists());
  assert!(std::path::Path::new(&config_file_path).exists());
  Ok(())
}

#[test]
fn test_mk_56_secrets_doctor_reports_config_and_sources() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    secrets:
      vault_location: vault
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("secrets")
    .arg("vault")
    .arg("init-vault")
    .arg("--gpg-key-id")
    .arg("METAKEY")
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("secrets")
    .arg("doctor")
    .assert()
    .success()
    .stdout(predicates::str::contains("Active config file: "))
    .stdout(predicates::str::is_match(r"Active config file: .*tasks\.yaml")?)
    .stdout(predicates::str::contains("Resolved backend: gpg"))
    .stdout(predicates::str::contains("Resolved backend source: vault-meta"))
    .stdout(predicates::str::contains("Resolved vault path source: root"))
    .stdout(predicates::str::contains("Resolved gpg key id: METAKEY"))
    .stdout(predicates::str::contains(
      "Resolved gpg key id source: vault-meta",
    ))
    .stdout(predicates::str::contains("Vault metadata used: yes"));

  Ok(())
}

#[test]
fn test_mk_57_validate_rejects_conflicting_legacy_and_secrets_fields() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "validate-secret-conflict.yaml",
    "
    vault_location: ./.mk/legacy-vault
    secrets:
      vault_location: ./.mk/new-vault
    tasks:
      demo:
        commands:
          - command: echo ready
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .failure()
    .stderr(predicates::str::contains("Validation failed"))
    .stdout(predicates::str::contains(
      "Legacy secret field 'vault_location' conflicts with `secrets.vault_location`",
    ));

  Ok(())
}

#[test]
fn test_mk_58_secrets_vault_init_write_config_updates_yaml() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    # yaml-language-server: $schema=https://example.invalid/schema.json
    environment:
      FOO: bar
    secrets:
      secrets_path:
        - app/common
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  let vault_dir = temp_dir.path().join("vault");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("secrets")
    .arg("vault")
    .arg("init-vault")
    .arg("--write-config")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--gpg-key-id")
    .arg("TEAMKEY")
    .assert()
    .success()
    .stdout(predicates::str::contains("Config updated at"));

  let config = std::fs::read_to_string(&config_file_path)?;
  assert!(config.contains("# yaml-language-server: $schema=https://example.invalid/schema.json"));
  assert!(config.contains("environment:"));
  assert!(config.contains("FOO: bar"));
  assert!(config.contains("backend: gpg"));
  assert!(config.contains("vault_location: vault"));
  assert!(config.contains("gpg_key_id: TEAMKEY"));
  assert!(config.contains("secrets_path:"));
  assert!(config.contains("- app/common"));
  assert!(config.contains("tasks:"));
  assert!(config.contains("noop:"));
  assert!(vault_dir.join(".vault-meta.toml").exists());
  assert_eq!(read_vault_gpg_key_id(&vault_dir), Some("TEAMKEY".to_string()));
  Ok(())
}

#[test]
fn test_mk_59_secrets_vault_init_write_config_creates_default_yaml() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("secrets")
    .arg("vault")
    .arg("init-vault")
    .arg("--write-config")
    .arg("--gpg-key-id")
    .arg("TEAMKEY")
    .assert()
    .success();

  let config_path = temp_dir.path().join("tasks.yaml");
  let config = std::fs::read_to_string(&config_path)?;
  assert!(config.contains("secrets:"));
  assert!(config.contains("backend: gpg"));
  assert!(config.contains("vault_location: .mk/vault"));
  assert!(config.contains("gpg_key_id: TEAMKEY"));
  assert!(config.contains("tasks: {}"));
  assert!(temp_dir.path().join(".mk/vault/.vault-meta.toml").exists());
  Ok(())
}

#[test]
fn test_mk_60_secrets_vault_init_write_config_rejects_toml() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(&temp_dir, "tasks.toml", "tasks = {}")?;
  let vault_dir = temp_dir.path().join("vault");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("secrets")
    .arg("vault")
    .arg("init-vault")
    .arg("--write-config")
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--gpg-key-id")
    .arg("TEAMKEY")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "Config mutation via --write-config only supports YAML files",
    ));

  assert!(!vault_dir.exists());
  Ok(())
}

#[test]
fn test_mk_61_root_secrets_block_injects_env_via_secrets_path() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let vault_dir = temp_dir.path().join("vault");
  let keys_dir = temp_dir.path().join("keys");

  // Generate key
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "key", "generate-key", "--location"])
    .arg(&keys_dir)
    .args(["--name", "default"])
    .assert()
    .success();

  // Init vault
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .assert()
    .success();

  // Store dotenv content as secret
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args([
      "secrets",
      "vault",
      "store-secret",
      "app/env",
      "MYSECRET=hello",
      "--vault-location",
    ])
    .arg(&vault_dir)
    .args(["--keys-location"])
    .arg(&keys_dir)
    .args(["--key-name", "default"])
    .assert()
    .success();

  let vault_str = vault_dir.to_utf8()?;
  let keys_str = keys_dir.to_utf8()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
      secrets:
        vault_location: {vault_str}
        keys_location: {keys_str}
        key_name: default
        secrets_path:
          - app/env
      tasks:
        check:
          commands:
            - command: test \"$MYSECRET\" = hello
              verbose: false
      "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("check")
    .assert()
    .success();

  Ok(())
}

#[test]
fn test_mk_62_task_secrets_block_overrides_root_key_name() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let vault_dir = temp_dir.path().join("vault");
  let keys_dir = temp_dir.path().join("keys");

  // Generate two keys: "default" and "task-key"
  for name in ["default", "task-key"] {
    Command::new(cargo::cargo_bin!("mk"))
      .current_dir(temp_dir.path())
      .args(["secrets", "key", "generate-key", "--location"])
      .arg(&keys_dir)
      .arg("--name")
      .arg(name)
      .assert()
      .success();
  }

  // Init vault
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .assert()
    .success();

  // Store secret encrypted with "task-key"
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args([
      "secrets",
      "vault",
      "store-secret",
      "app/task-env",
      "TASK_VAL=world",
      "--vault-location",
    ])
    .arg(&vault_dir)
    .args(["--keys-location"])
    .arg(&keys_dir)
    .args(["--key-name", "task-key"])
    .assert()
    .success();

  let vault_str = vault_dir.to_utf8()?;
  let keys_str = keys_dir.to_utf8()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
      secrets:
        vault_location: {vault_str}
        keys_location: {keys_str}
        key_name: default
      tasks:
        check:
          secrets:
            key_name: task-key
            secrets_path:
              - app/task-env
          commands:
            - command: test \"$TASK_VAL\" = world
              verbose: false
      "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("check")
    .assert()
    .success();

  Ok(())
}

#[test]
fn test_mk_63_secrets_cli_reads_vault_location_from_secrets_block() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let vault_dir = temp_dir.path().join("custom-vault");
  let keys_dir = temp_dir.path().join("keys");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "key", "generate-key", "--location"])
    .arg(&keys_dir)
    .args(["--name", "default"])
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args([
      "secrets",
      "vault",
      "store-secret",
      "app/token",
      "s3cr3t",
      "--vault-location",
    ])
    .arg(&vault_dir)
    .args(["--keys-location"])
    .arg(&keys_dir)
    .args(["--key-name", "default"])
    .assert()
    .success();

  let vault_str = vault_dir.to_utf8()?;
  let keys_str = keys_dir.to_utf8()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
      secrets:
        vault_location: {vault_str}
        keys_location: {keys_str}
        key_name: default
      tasks:
        noop:
          commands:
            - command: echo noop
              verbose: false
      "
    ),
  )?;

  // list-secrets should use vault_location from secrets block without --vault-location flag
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "vault", "list-secrets"])
    .assert()
    .success()
    .stdout(predicates::str::contains("app/token"));

  Ok(())
}

#[test]
fn test_mk_64_vault_metadata_fills_missing_config_values() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let vault_dir = temp_dir.path().join("vault");

  // Init vault with a GPG key ID so metadata records it
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .args(["--gpg-key-id", "META_KEY_ID"])
    .assert()
    .success();

  let vault_str = vault_dir.to_utf8()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
      secrets:
        vault_location: {vault_str}
      tasks:
        noop:
          commands:
            - command: echo noop
              verbose: false
      "
    ),
  )?;

  // doctor should show gpg_key_id resolved from vault metadata
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "doctor"])
    .assert()
    .success()
    .stdout(predicates::str::contains("META_KEY_ID"))
    .stdout(predicates::str::contains("vault-meta"));

  Ok(())
}

#[test]
fn test_mk_65_cli_flags_override_config_secrets_block() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_vault = temp_dir.path().join("config-vault");
  let cli_vault = temp_dir.path().join("cli-vault");
  let keys_dir = temp_dir.path().join("keys");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "key", "generate-key", "--location"])
    .arg(&keys_dir)
    .args(["--name", "default"])
    .assert()
    .success();

  // Init the CLI vault and store a secret there
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&cli_vault)
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args([
      "secrets",
      "vault",
      "store-secret",
      "app/token",
      "cli-secret",
      "--vault-location",
    ])
    .arg(&cli_vault)
    .args(["--keys-location"])
    .arg(&keys_dir)
    .args(["--key-name", "default"])
    .assert()
    .success();

  // Init the config vault (different, empty vault)
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&config_vault)
    .assert()
    .success();

  let config_vault_str = config_vault.to_utf8()?;
  let keys_str = keys_dir.to_utf8()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
      secrets:
        vault_location: {config_vault_str}
        keys_location: {keys_str}
        key_name: default
      tasks:
        noop:
          commands:
            - command: echo noop
              verbose: false
      "
    ),
  )?;

  // CLI --vault-location flag overrides the config block; the secret in cli_vault is found
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "vault", "list-secrets", "--vault-location"])
    .arg(&cli_vault)
    .assert()
    .success()
    .stdout(predicates::str::contains("app/token"));

  Ok(())
}

#[test]
fn test_mk_66_old_scalar_root_fields_still_work() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let vault_dir = temp_dir.path().join("vault");
  let keys_dir = temp_dir.path().join("keys");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "key", "generate-key", "--location"])
    .arg(&keys_dir)
    .args(["--name", "default"])
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args([
      "secrets",
      "vault",
      "store-secret",
      "app/legacy-env",
      "LEGACY_VAR=ok",
      "--vault-location",
    ])
    .arg(&vault_dir)
    .args(["--keys-location"])
    .arg(&keys_dir)
    .args(["--key-name", "default"])
    .assert()
    .success();

  let vault_str = vault_dir.to_utf8()?;
  let keys_str = keys_dir.to_utf8()?;
  // Config uses legacy root-level scalar fields (deprecated but still supported)
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
      vault_location: {vault_str}
      keys_location: {keys_str}
      key_name: default
      secrets_path:
        - app/legacy-env
      tasks:
        check:
          commands:
            - command: test \"$LEGACY_VAR\" = ok
              verbose: false
      "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("check")
    .assert()
    .success();

  Ok(())
}

#[test]
fn test_mk_67_validate_rejects_gpg_backend_without_gpg_key_id() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    secrets:
      backend: gpg
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .failure()
    .stdout(predicates::str::contains("GPG backend requires gpg_key_id"));

  Ok(())
}

#[test]
fn test_mk_68_validate_rejects_pgp_backend_without_key_name() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    secrets:
      backend: built_in_pgp
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .assert()
    .failure()
    .stdout(predicates::str::contains("PGP backend requires key_name"));

  Ok(())
}

#[test]
fn test_mk_69_secrets_doctor_shows_all_resolved_fields() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let vault_dir = temp_dir.path().join("vault");
  let keys_dir = temp_dir.path().join("keys");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .assert()
    .success();

  let vault_str = vault_dir.to_utf8()?;
  let keys_str = keys_dir.to_utf8()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
      secrets:
        vault_location: {vault_str}
        keys_location: {keys_str}
        key_name: mykey
      tasks:
        noop:
          commands:
            - command: echo noop
              verbose: false
      "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "doctor"])
    .assert()
    .success()
    .stdout(predicates::str::contains(vault_str))
    .stdout(predicates::str::contains(keys_str))
    .stdout(predicates::str::contains("mykey"))
    .stdout(predicates::str::contains("config"));

  Ok(())
}

#[test]
fn test_mk_70_list_label_key_filter_text() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      ci-build:
        labels:
          area: ci
        description: CI build task
        commands:
          - command: echo ci
            verbose: false
      deploy:
        labels:
          area: deploy
        description: deploy task
        commands:
          - command: echo deploy
            verbose: false
      lint:
        description: lint task
        commands:
          - command: echo lint
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("list")
    .arg("--plain")
    .arg("--label")
    .arg("area")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  assert!(stdout.contains("ci-build"));
  assert!(stdout.contains("deploy"));
  assert!(!stdout.contains("lint"));
  Ok(())
}

#[test]
fn test_mk_70_list_label_key_value_filter_text() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      ci-build:
        labels:
          area: ci
        description: CI build task
        commands:
          - command: echo ci
            verbose: false
      deploy:
        labels:
          area: deploy
        description: deploy task
        commands:
          - command: echo deploy
            verbose: false
      lint:
        description: lint task
        commands:
          - command: echo lint
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("list")
    .arg("--plain")
    .arg("--label")
    .arg("area=ci")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  assert!(stdout.contains("ci-build"));
  assert!(!stdout.contains("deploy"));
  assert!(!stdout.contains("lint"));
  Ok(())
}

#[test]
fn test_mk_70_list_label_filter_json() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      ci-build:
        labels:
          area: ci
          kind: test
        description: CI build task
        commands:
          - command: echo ci
            verbose: false
      deploy:
        labels:
          area: deploy
        description: deploy task
        commands:
          - command: echo deploy
            verbose: false
      lint:
        description: lint task
        commands:
          - command: echo lint
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("list")
    .arg("--json")
    .arg("--label")
    .arg("area=ci")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  let parsed: serde_json::Value = serde_json::from_str(&stdout)?;
  let arr = parsed.as_array().expect("expected array");
  assert_eq!(arr.len(), 1);
  assert_eq!(arr[0]["name"], "ci-build");
  assert_eq!(arr[0]["labels"]["area"], "ci");
  assert_eq!(arr[0]["labels"]["kind"], "test");
  Ok(())
}

#[test]
fn test_mk_70_list_json_includes_labels() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      alpha:
        labels:
          area: build
        description: alpha task
        commands:
          - command: echo alpha
            verbose: false
      beta:
        description: beta task
        commands:
          - command: echo beta
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("list")
    .arg("--json")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  let parsed: serde_json::Value = serde_json::from_str(&stdout)?;
  let arr = parsed.as_array().expect("expected array");
  assert_eq!(arr.len(), 2);
  // alpha has labels
  assert_eq!(arr[0]["name"], "alpha");
  assert_eq!(arr[0]["labels"]["area"], "build");
  // beta has empty labels object
  assert_eq!(arr[1]["name"], "beta");
  assert!(arr[1]["labels"]
    .as_object()
    .map(|m| m.is_empty())
    .unwrap_or(false));
  Ok(())
}

#[test]
fn test_mk_70_list_label_multiple_filters_and() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      ci-test:
        labels:
          area: ci
          kind: test
        description: CI test
        commands:
          - command: echo ci-test
            verbose: false
      ci-build:
        labels:
          area: ci
          kind: build
        description: CI build
        commands:
          - command: echo ci-build
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("list")
    .arg("--plain")
    .arg("--label")
    .arg("area=ci")
    .arg("--label")
    .arg("kind=test")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  assert!(stdout.contains("ci-test"));
  assert!(!stdout.contains("ci-build"));
  Ok(())
}

#[test]
fn test_mk_71_run_label_filter_runs_matching_tasks() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      ci-test:
        labels:
          kind: test
        description: CI test
        commands:
          - command: echo ran-ci-test
            verbose: false
      ci-build:
        labels:
          kind: build
        description: CI build
        commands:
          - command: echo ran-ci-build
            verbose: false
      deploy:
        description: deploy (no labels)
        commands:
          - command: echo ran-deploy
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--label")
    .arg("kind=test")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  assert!(stdout.contains("ran-ci-test"), "expected ci-test to run");
  assert!(!stdout.contains("ran-ci-build"), "expected ci-build not to run");
  assert!(!stdout.contains("ran-deploy"), "expected deploy not to run");
  Ok(())
}

#[test]
fn test_mk_71_run_label_filter_multiple_matches_sorted_order() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      b-task:
        labels:
          kind: test
        description: b task
        commands:
          - command: echo ran-b
            verbose: false
      a-task:
        labels:
          kind: test
        description: a task
        commands:
          - command: echo ran-a
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--label")
    .arg("kind=test")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  let pos_a = stdout.find("ran-a").expect("ran-a not found");
  let pos_b = stdout.find("ran-b").expect("ran-b not found");
  assert!(pos_a < pos_b, "a-task should run before b-task");
  Ok(())
}

#[test]
fn test_mk_71_run_label_filter_no_match_errors() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        description: noop
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("--label")
    .arg("kind=missing")
    .assert()
    .failure()
    .stderr(predicates::str::contains("No tasks matched"));
  Ok(())
}

#[test]
fn test_mk_71_run_no_name_no_label_errors() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "Provide a task name or at least one --label filter",
    ));
  Ok(())
}

#[test]
fn test_mk_72_plan_label_filter_text() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      a-task:
        labels:
          kind: test
        description: a task
        commands:
          - command: echo a
            verbose: false
      b-task:
        labels:
          kind: test
        description: b task
        commands:
          - command: echo b
            verbose: false
      other:
        description: other
        commands:
          - command: echo other
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("--label")
    .arg("kind=test")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  assert!(stdout.contains("a-task"), "a-task should appear in plan");
  assert!(stdout.contains("b-task"), "b-task should appear in plan");
  assert!(!stdout.contains("other"), "other should not appear in plan");
  let pos_a = stdout.find("a-task").expect("a-task not found");
  let pos_b = stdout.find("b-task").expect("b-task not found");
  assert!(pos_a < pos_b, "a-task should come before b-task");
  Ok(())
}

#[test]
fn test_mk_72_plan_label_filter_json() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      alpha:
        labels:
          kind: ci
        description: alpha
        commands:
          - command: echo alpha
            verbose: false
      beta:
        description: beta (no labels)
        commands:
          - command: echo beta
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("--json")
    .arg("--label")
    .arg("kind=ci")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = String::from_utf8(output)?;
  // One JSON object per matched task — alpha only
  assert!(stdout.contains("\"alpha\""), "alpha plan expected");
  assert!(!stdout.contains("\"beta\""), "beta should not appear");
  Ok(())
}

#[test]
fn test_mk_72_plan_no_name_no_label_errors() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "Provide a task name or at least one --label filter",
    ));
  Ok(())
}

#[test]
fn test_mk_73_vault_store_secret_requires_path() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  let vault_dir = temp_dir.path().join("vault");
  let keys_dir = temp_dir.path().join("keys");

  Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "vault", "store-secret"])
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--keys-location")
    .arg(&keys_dir)
    .assert()
    .failure()
    .code(2);

  Ok(())
}

#[test]
fn test_mk_78_vault_init_with_gpg_key() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let vault_dir = temp_dir.path().join("vault");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .args(["--gpg-key-id", "ABC123DEF456"])
    .assert()
    .success()
    .stdout(predicates::str::contains("Vault created"))
    .stdout(predicates::str::contains("GPG key: ABC123DEF456"));

  // Verify metadata was written
  assert!(vault_dir.exists());
  assert!(vault_dir.join(".vault-meta.toml").exists());

  Ok(())
}

#[test]
fn test_mk_78_vault_init_already_exists() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let vault_dir = temp_dir.path().join("vault");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .assert()
    .success()
    .stdout(predicates::str::contains("Vault created"));

  // Try initializing again
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .assert()
    .success()
    .stdout(predicates::str::contains("Vault already exists"));

  Ok(())
}

#[test]
fn test_mk_79_vault_purge_secret_not_found() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  let vault_dir = temp_dir.path().join("vault");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "vault", "init-vault", "--vault-location"])
    .arg(&vault_dir)
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "vault", "purge-secret", "nonexistent"])
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--yes")
    .assert()
    .success()
    .stdout(predicates::str::contains("not found in vault"));

  Ok(())
}

#[test]
fn test_mk_79_vault_export_missing_vault() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  let vault_dir = temp_dir.path().join("missing-vault");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "vault", "export-secret", "some/secret"])
    .arg("--vault-location")
    .arg(&vault_dir)
    .assert()
    .failure()
    .stderr(predicates::str::contains("Vault not found"));

  Ok(())
}

#[test]
fn test_mk_80_vault_store_secret_missing_vault() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  let vault_dir = temp_dir.path().join("missing-vault");
  let keys_dir = temp_dir.path().join("keys");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "vault", "store-secret", "test/key", "test-value"])
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--keys-location")
    .arg(&keys_dir)
    .arg("--key-name")
    .arg("default")
    .assert()
    .failure()
    .stderr(predicates::str::contains("Vault not found"));

  Ok(())
}

#[test]
fn test_mk_80_vault_show_secret_missing_vault() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  let vault_dir = temp_dir.path().join("missing-vault");
  let keys_dir = temp_dir.path().join("keys");

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .args(["secrets", "vault", "show-secret", "test/key"])
    .arg("--vault-location")
    .arg(&vault_dir)
    .arg("--keys-location")
    .arg(&keys_dir)
    .arg("--key-name")
    .arg("default")
    .assert()
    .failure()
    .stderr(predicates::str::contains("Vault not found"));

  Ok(())
}

// ---- doctor diagnostics ----

#[test]
fn test_doctor_config_found() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("doctor")
    .assert()
    .success()
    .stdout(predicates::str::contains("[ok]"))
    .stdout(predicates::str::contains("tasks.yaml"))
    .stdout(predicates::str::contains("format: yaml"))
    .stdout(predicates::str::contains("All checks passed."));

  Ok(())
}

#[test]
fn test_doctor_config_missing() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("doctor")
    .assert()
    .failure()
    .stdout(predicates::str::contains("[fail]"))
    .stdout(predicates::str::contains("config not found"))
    .stderr(predicates::str::contains("One or more checks failed."));

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_doctor_with_fake_docker_runtime() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  write_fake_sh(&temp_dir)?;
  write_fake_selector(&temp_dir, "docker", "#!/bin/sh\nexit 0\n")?;
  let path = format!(
    "{}:{}",
    temp_dir.path().to_string_lossy(),
    std::env::var("PATH").unwrap_or_default()
  );

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env("PATH", path)
    .arg("-c")
    .arg(&config_file_path)
    .arg("doctor")
    .assert()
    .success()
    .stdout(predicates::str::contains("[ok]   docker:"))
    .stdout(predicates::str::contains("[ok]   auto:"));

  Ok(())
}

#[cfg(unix)]
#[test]
fn test_doctor_no_container_runtime() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  // Use an empty temp dir as PATH so no container runtimes are found.
  let empty_path_dir = TempDir::new()?;
  write_fake_sh(&empty_path_dir)?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env("PATH", empty_path_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("doctor")
    .assert()
    .success()
    .stdout(predicates::str::contains(
      "[warn] auto: no container runtime found",
    ));

  Ok(())
}

#[test]
fn test_doctor_cache_not_yet_created() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("doctor")
    .assert()
    .success()
    .stdout(predicates::str::contains("cache.json (not yet created)"));

  Ok(())
}

#[test]
fn test_doctor_cache_exists() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;
  // Run a task first to create cache.
  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("noop")
    .assert()
    .success();

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("doctor")
    .assert()
    .success()
    .stdout(predicates::str::contains("[ok]   cache:"));

  Ok(())
}

#[test]
fn test_doctor_secrets_not_configured() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("doctor")
    .assert()
    .success()
    .stdout(predicates::str::contains("secrets: not configured"));

  Ok(())
}

#[test]
fn test_doctor_secrets_configured_vault_missing() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let vault_dir = temp_dir.path().join("vault");
  let keys_dir = temp_dir.path().join("keys");
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    &format!(
      "
      secrets:
        vault_location: {}
        keys_location: {}
      tasks:
        noop:
          commands:
            - command: echo noop
              verbose: false
      ",
      vault_dir.to_string_lossy(),
      keys_dir.to_string_lossy(),
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("doctor")
    .assert()
    .failure()
    .stdout(predicates::str::contains("[fail] vault not found:"))
    .stderr(predicates::str::contains("One or more checks failed."));

  Ok(())
}

#[test]
fn test_doctor_secrets_configured_healthy() -> anyhow::Result<()> {
  let (temp_dir, config_file_path, vault_dir, keys_dir) =
    setup_secrets_fixture("myapp/db-password", "supersecret")?;

  let config_file_path_with_secrets = common::setup_yaml(
    &temp_dir,
    "tasks-with-secrets.yaml",
    &format!(
      "
      secrets:
        vault_location: {}
        keys_location: {}
      tasks:
        noop:
          commands:
            - command: echo noop
              verbose: false
      ",
      vault_dir.to_string_lossy(),
      keys_dir.to_string_lossy(),
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path_with_secrets)
    .arg("doctor")
    .assert()
    .success()
    .stdout(predicates::str::contains("[ok]   vault:"))
    .stdout(predicates::str::contains("[ok]   keys:"))
    .stdout(predicates::str::contains("All checks passed."));

  // Suppress unused warning for config_file_path kept alive as fixture owner.
  let _ = config_file_path;

  Ok(())
}

// ── Output Plumbing – Phase 1: Capture Expansion ─────────────────────────────

#[test]
fn test_output_plumbing_p1_stderr_capture_only() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let result_file = temp_dir.path().join("stderr.txt");
  let shell = common::portable_test_shell();
  let stderr_command = if cfg!(windows) {
    "[Console]::Error.Write('stderr-value')".to_string()
  } else {
    "printf 'stderr-value' >&2".to_string()
  };
  let write_command = if cfg!(windows) {
    format!(
      "[System.IO.File]::WriteAllText('{}', '${{{{ outputs.err }}}}')",
      common::sh_path(&result_file)
    )
  } else {
    format!(
      "printf '%s' \"${{{{ outputs.err }}}}\" > {}",
      common::sh_path(&result_file)
    )
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "stderr-capture.yaml",
    &format!(
      "
    tasks:
      capture:
        shell: {shell}
        commands:
          - command: {stderr_command}
            save_stderr_as: err
            verbose: false
          - command: {write_command}
            verbose: false
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("capture")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&result_file)?, "stderr-value");
  Ok(())
}

#[test]
fn test_output_plumbing_p1_stdout_and_stderr_capture_combined() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let result_file = temp_dir.path().join("combined.txt");
  let shell = common::portable_test_shell();
  let (stdout_command, stderr_command) = if cfg!(windows) {
    (
      "Write-Output 'out-value'".to_string(),
      "[Console]::Error.Write('err-value')".to_string(),
    )
  } else {
    (
      "printf 'out-value\\n'".to_string(),
      "printf 'err-value' >&2".to_string(),
    )
  };
  let write_command = if cfg!(windows) {
    format!(
      "[System.IO.File]::WriteAllText('{}', '${{{{ outputs.out }}}}|${{{{ outputs.err }}}}')",
      common::sh_path(&result_file)
    )
  } else {
    format!(
      "printf '%s|%s' \"${{{{ outputs.out }}}}\" \"${{{{ outputs.err }}}}\" > {}",
      common::sh_path(&result_file)
    )
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "combined-capture.yaml",
    &format!(
      "
    tasks:
      capture:
        shell: {shell}
        commands:
          - command: {stdout_command}
            save_output_as: out
            verbose: false
          - command: {stderr_command}
            save_stderr_as: err
            verbose: false
          - command: {write_command}
            verbose: false
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("capture")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&result_file)?, "out-value|err-value");
  Ok(())
}

#[test]
fn test_output_plumbing_p1_exit_code_capture_success() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let result_file = temp_dir.path().join("exit_code.txt");
  let shell = common::portable_test_shell();
  let true_command = if cfg!(windows) {
    "exit 0".to_string()
  } else {
    "true".to_string()
  };
  let write_command = if cfg!(windows) {
    format!(
      "[System.IO.File]::WriteAllText('{}', '${{{{ outputs.code }}}}')",
      common::sh_path(&result_file)
    )
  } else {
    format!(
      "printf '%s' \"${{{{ outputs.code }}}}\" > {}",
      common::sh_path(&result_file)
    )
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "exit-code-capture.yaml",
    &format!(
      "
    tasks:
      capture:
        shell: {shell}
        commands:
          - command: \"{true_command}\"
            save_exit_code_as: code
            verbose: false
          - command: {write_command}
            verbose: false
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("capture")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&result_file)?, "0");
  Ok(())
}

#[test]
fn test_output_plumbing_p1_exit_code_capture_failing_command_with_ignore_errors() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let result_file = temp_dir.path().join("exit_code.txt");
  let shell = common::portable_test_shell();
  let fail_command = if cfg!(windows) {
    "exit 1".to_string()
  } else {
    "false".to_string()
  };
  let write_command = if cfg!(windows) {
    format!(
      "[System.IO.File]::WriteAllText('{}', '${{{{ outputs.code }}}}')",
      common::sh_path(&result_file)
    )
  } else {
    format!(
      "printf '%s' \"${{{{ outputs.code }}}}\" > {}",
      common::sh_path(&result_file)
    )
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "exit-code-fail.yaml",
    &format!(
      "
    tasks:
      capture:
        shell: {shell}
        commands:
          - command: \"{fail_command}\"
            save_exit_code_as: code
            ignore_errors: true
            verbose: false
          - command: {write_command}
            verbose: false
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("capture")
    .assert()
    .success();

  let code = std::fs::read_to_string(&result_file)?;
  assert_ne!(code, "0", "exit code should be non-zero for failing command");
  Ok(())
}

// ── Output Plumbing – Phase 2: Structured Extraction ─────────────────────────

#[test]
fn test_output_plumbing_p2_json_extract_valid() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let result_file = temp_dir.path().join("json_out.txt");
  let shell = common::portable_test_shell();
  let json_command = if cfg!(windows) {
    "Write-Output '{\"version\":\"2.0.0\"}'".to_string()
  } else {
    r#"printf '{"version":"2.0.0"}\n'"#.to_string()
  };
  let write_command = if cfg!(windows) {
    format!(
      "[System.IO.File]::WriteAllText('{}', '${{{{ outputs.ver }}}}')",
      common::sh_path(&result_file)
    )
  } else {
    format!(
      "printf '%s' \"${{{{ outputs.ver }}}}\" > {}",
      common::sh_path(&result_file)
    )
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "json-extract.yaml",
    &format!(
      "
    tasks:
      extract:
        shell: {shell}
        commands:
          - command: {json_command}
            save_output_as: raw
            verbose: false
          - extract_json_from: raw
            json_path: version
            save_as: ver
          - command: {write_command}
            verbose: false
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("extract")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&result_file)?, "2.0.0");
  Ok(())
}

#[test]
fn test_output_plumbing_p2_json_extract_invalid_json_fails() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let shell = common::portable_test_shell();
  let bad_json_command = if cfg!(windows) {
    "Write-Output 'not json'".to_string()
  } else {
    "printf 'not json'".to_string()
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "bad-json.yaml",
    &format!(
      "
    tasks:
      extract:
        shell: {shell}
        commands:
          - command: {bad_json_command}
            save_output_as: raw
            verbose: false
          - extract_json_from: raw
            json_path: key
            save_as: result
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("extract")
    .assert()
    .failure()
    .stderr(predicates::str::contains("Failed to parse"));

  Ok(())
}

#[test]
fn test_output_plumbing_p2_json_extract_missing_path_fails() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let shell = common::portable_test_shell();
  let json_command = if cfg!(windows) {
    "Write-Output '{\"key\":\"val\"}'".to_string()
  } else {
    r#"printf '{"key":"val"}\n'"#.to_string()
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "missing-path.yaml",
    &format!(
      "
    tasks:
      extract:
        shell: {shell}
        commands:
          - command: {json_command}
            save_output_as: raw
            verbose: false
          - extract_json_from: raw
            json_path: nonexistent
            save_as: result
    "
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("extract")
    .assert()
    .failure()
    .stderr(predicates::str::contains("JSON path"));

  Ok(())
}

// ── Output Plumbing – Phase 3: Artifact Writes ───────────────────────────────

#[test]
fn test_output_plumbing_p3_write_output_to_file() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let dest_file = temp_dir.path().join("artifact.txt");
  let shell = common::portable_test_shell();
  let capture_command = if cfg!(windows) {
    "Write-Output 'artifact-content'".to_string()
  } else {
    "printf 'artifact-content\\n'".to_string()
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "write-output.yaml",
    &format!(
      "
    tasks:
      write:
        shell: {shell}
        commands:
          - command: {capture_command}
            save_output_as: content
            verbose: false
          - write_output: content
            to_file: {}
    ",
      common::sh_path(&dest_file)
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("write")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&dest_file)?, "artifact-content");
  Ok(())
}

#[test]
fn test_output_plumbing_p3_write_output_fails_on_existing_file() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let dest_file = temp_dir.path().join("existing.txt");
  std::fs::write(&dest_file, "existing")?;
  let shell = common::portable_test_shell();
  let capture_command = if cfg!(windows) {
    "Write-Output 'new-content'".to_string()
  } else {
    "printf 'new-content\\n'".to_string()
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "write-existing.yaml",
    &format!(
      "
    tasks:
      write:
        shell: {shell}
        commands:
          - command: {capture_command}
            save_output_as: content
            verbose: false
          - write_output: content
            to_file: {}
    ",
      common::sh_path(&dest_file)
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("write")
    .assert()
    .failure()
    .stderr(predicates::str::contains("already exists"));

  Ok(())
}

#[test]
fn test_output_plumbing_p3_write_output_with_create_parents() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let dest_file = temp_dir.path().join("nested").join("dir").join("out.txt");
  let shell = common::portable_test_shell();
  let capture_command = if cfg!(windows) {
    "Write-Output 'nested-content'".to_string()
  } else {
    "printf 'nested-content\\n'".to_string()
  };
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "write-parents.yaml",
    &format!(
      "
    tasks:
      write:
        shell: {shell}
        commands:
          - command: {capture_command}
            save_output_as: content
            verbose: false
          - write_output: content
            to_file: {}
            create_parents: true
    ",
      common::sh_path(&dest_file)
    ),
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("write")
    .assert()
    .success();

  assert_eq!(std::fs::read_to_string(&dest_file)?, "nested-content");
  Ok(())
}

// --- Trailing args passthrough tests ---

#[test]
fn test_trailing_args_via_run_subcommand() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      greet:
        commands:
          - command: echo '${{ args.0 }} ${{ args.1 }}'
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("greet")
    .arg("--")
    .arg("hello")
    .arg("world")
    .assert()
    .success()
    .stdout(predicates::str::contains("hello world"));

  Ok(())
}

#[test]
fn test_trailing_args_via_direct_task_shortcut() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      greet:
        commands:
          - command: echo '${{ args.0 }}'
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("greet")
    .arg("--")
    .arg("alpha")
    .assert()
    .success()
    .stdout(predicates::str::contains("alpha"));

  Ok(())
}

#[test]
fn test_trailing_args_len_expression() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      count:
        commands:
          - command: echo '${{ args.len }}'
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("count")
    .arg("--")
    .arg("a")
    .arg("b")
    .arg("c")
    .assert()
    .success()
    .stdout(predicates::str::contains("3"));

  Ok(())
}

#[test]
fn test_trailing_args_env_vars_injected() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      envcheck:
        commands:
          - command: echo \"argc=$MK_RUN_ARGC arg0=$MK_RUN_ARG_0\"
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("envcheck")
    .arg("--")
    .arg("first")
    .assert()
    .success()
    .stdout(predicates::str::contains("argc=1"))
    .stdout(predicates::str::contains("arg0=first"));

  Ok(())
}

#[test]
fn test_trailing_args_preserve_leading_hyphens() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      flags:
        commands:
          - command: echo '${{ args.0 }} ${{ args.1 }}'
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("flags")
    .arg("--")
    .arg("--flag")
    .arg("value")
    .assert()
    .success()
    .stdout(predicates::str::contains("--flag value"));

  Ok(())
}

#[test]
fn test_trailing_args_out_of_range_fails_with_message() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      oob:
        commands:
          - command: echo '${{ args.5 }}'
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("oob")
    .arg("--")
    .arg("only-one")
    .assert()
    .failure()
    .stderr(predicates::str::contains("args.5 is out of range"));

  Ok(())
}

#[test]
fn test_trailing_args_no_args_gives_zero_len() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      count:
        commands:
          - command: echo '${{ args.len }}'
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("run")
    .arg("count")
    .assert()
    .success()
    .stdout(predicates::str::contains("0"));

  Ok(())
}

#[test]
fn test_watch_requires_task_name_or_label() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "Provide a task name or at least one --label filter",
    ));

  Ok(())
}

#[test]
fn test_watch_accepts_task_name() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
    ",
  )?;

  // Task has no inputs and no --path: watch should error with a clear message.
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("build")
    .output()?;

  assert!(!output.status.success());
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("No watch paths"),
    "expected 'No watch paths' in stderr, got: {}",
    String::from_utf8_lossy(&output.stderr)
  );

  Ok(())
}

#[test]
fn test_watch_accepts_repeated_path_flags() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
    ",
  )?;

  // Create the directories so the watcher can actually watch them.
  std::fs::create_dir_all(temp_dir.path().join("src"))?;
  std::fs::create_dir_all(temp_dir.path().join("tests"))?;

  // Multiple --path flags must parse without error; watcher starts and prints paths.
  let mut child = std::process::Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("build")
    .arg("--path")
    .arg("src")
    .arg("--path")
    .arg("tests")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;

  std::thread::sleep(std::time::Duration::from_millis(500));
  child.kill()?;
  let output = child.wait_with_output()?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("src") && stdout.contains("tests"),
    "expected path list in output, got: {}",
    stdout
  );

  Ok(())
}

#[test]
fn test_watch_accepts_debounce_ms() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
    ",
  )?;

  // Task has no inputs and no --path: parse succeeds but exits with path error.
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("build")
    .arg("--debounce")
    .arg("500ms")
    .output()?;

  assert!(!output.status.success());
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    !stderr.contains("invalid duration") && !stderr.contains("debounce"),
    "debounce parse should not fail, got: {stderr}"
  );

  Ok(())
}

#[test]
fn test_watch_accepts_debounce_seconds() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
    ",
  )?;

  // Task has no inputs and no --path: parse succeeds but exits with path error.
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("build")
    .arg("--debounce")
    .arg("2s")
    .output()?;

  assert!(!output.status.success());
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    !stderr.contains("invalid duration") && !stderr.contains("debounce"),
    "debounce parse should not fail, got: {stderr}"
  );

  Ok(())
}

#[test]
fn test_watch_rejects_zero_debounce() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("build")
    .arg("--debounce")
    .arg("0ms")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "debounce duration must be greater than zero",
    ));

  Ok(())
}

#[test]
fn test_watch_rejects_invalid_debounce_format() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("build")
    .arg("--debounce")
    .arg("fast")
    .assert()
    .failure()
    .stderr(predicates::str::contains("invalid duration"));

  Ok(())
}

#[test]
fn test_watch_label_filter_resolves_matching_tasks() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        labels:
          kind: test
        commands:
          - command: echo build
            verbose: false
      deploy:
        labels:
          kind: deploy
        commands:
          - command: echo deploy
            verbose: false
    ",
  )?;

  // Error message names the matched task; unmatched task should not appear.
  let src_dir = temp_dir.path().join("src");
  std::fs::create_dir_all(&src_dir)?;

  let mut child = std::process::Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("--label")
    .arg("kind=test")
    .arg("--path")
    .arg("src")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;

  std::thread::sleep(std::time::Duration::from_millis(500));
  child.kill()?;
  let output = child.wait_with_output()?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("build"),
    "expected matched task in output, got: {}",
    stdout
  );
  assert!(
    !stdout.contains("deploy"),
    "unexpected task in output: {}",
    stdout
  );

  Ok(())
}

#[test]
fn test_watch_label_filter_no_match_errors() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      noop:
        commands:
          - command: echo noop
            verbose: false
    ",
  )?;

  Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("--label")
    .arg("kind=missing")
    .assert()
    .failure()
    .stderr(predicates::str::contains("No tasks matched"));

  Ok(())
}

// ---------------------------------------------------------------------------
// Phase 2: Watch Execution Behavior
// ---------------------------------------------------------------------------

#[test]
fn test_watch_errors_when_no_paths_and_no_inputs() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        commands:
          - command: echo build
            verbose: false
    ",
  )?;

  // No --path and no task inputs → must fail with a clear message.
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("build")
    .output()?;

  assert!(!output.status.success());
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("No watch paths"),
    "expected 'No watch paths' in stderr, got: {}",
    String::from_utf8_lossy(&output.stderr)
  );

  Ok(())
}

#[test]
fn test_watch_uses_task_inputs_as_default_paths() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;

  // Create the file declared in task inputs so it resolves.
  let input_file = temp_dir.path().join("input.txt");
  std::fs::write(&input_file, "seed")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        inputs:
          - input.txt
        commands:
          - command: echo build-ran
            verbose: false
    ",
  )?;

  // No --path: watch must infer paths from task inputs and start successfully.
  let mut child = std::process::Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("build")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;

  std::thread::sleep(std::time::Duration::from_millis(600));
  child.kill()?;
  let output = child.wait_with_output()?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("input.txt"),
    "expected watched path in output, got: {stdout}"
  );
  assert!(
    stdout.contains("build-ran"),
    "expected initial task run in output, got: {stdout}"
  );

  Ok(())
}

#[test]
fn test_watch_file_change_triggers_rerun() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;

  // Create the watched file before starting the watcher.
  let watched = temp_dir.path().join("trigger.txt");
  std::fs::write(&watched, "v1")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      greet:
        commands:
          - command: echo greeted
            verbose: false
    ",
  )?;

  let mut child = std::process::Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("greet")
    .arg("--path")
    .arg(&watched)
    .arg("--debounce")
    .arg("100ms")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;

  // Wait for initial task run and watcher setup.
  std::thread::sleep(std::time::Duration::from_millis(600));

  // Trigger a file change.
  std::fs::write(&watched, "v2")?;

  // Wait for debounce + rerun.
  std::thread::sleep(std::time::Duration::from_millis(600));

  child.kill()?;
  let output = child.wait_with_output()?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  // Task output should appear at least twice (initial + rerun).
  let occurrences = stdout.matches("greeted").count();
  assert!(
    occurrences >= 2,
    "expected at least 2 task runs, got {occurrences} in:\n{stdout}"
  );
  assert!(
    stdout.contains("Change detected") || stdout.contains("re-running"),
    "expected rerun reason in output, got:\n{stdout}"
  );

  Ok(())
}

// ---------------------------------------------------------------------------
// Phase 3: Quality of Life
// ---------------------------------------------------------------------------

#[test]
fn test_watch_clear_flag_is_accepted() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let watched = temp_dir.path().join("src.txt");
  std::fs::write(&watched, "init")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      echo:
        commands:
          - command: echo hello
            verbose: false
    ",
  )?;

  // --clear must parse without error; watcher starts normally.
  let mut child = std::process::Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("echo")
    .arg("--path")
    .arg(&watched)
    .arg("--clear")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;

  std::thread::sleep(std::time::Duration::from_millis(400));
  child.kill()?;
  let output = child.wait_with_output()?;

  // Watcher must have started (initial task run visible).
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("hello"),
    "expected initial run in output: {stdout}"
  );

  Ok(())
}

#[test]
fn test_watch_clear_flag_emits_ansi_clear_on_rerun() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let watched = temp_dir.path().join("trigger.txt");
  std::fs::write(&watched, "v1")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      stamp:
        commands:
          - command: echo stamped
            verbose: false
    ",
  )?;

  let mut child = std::process::Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("stamp")
    .arg("--path")
    .arg(&watched)
    .arg("--debounce")
    .arg("100ms")
    .arg("--clear")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;

  std::thread::sleep(std::time::Duration::from_millis(600));
  std::fs::write(&watched, "v2")?;
  std::thread::sleep(std::time::Duration::from_millis(600));

  child.kill()?;
  let output = child.wait_with_output()?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  // ANSI clear sequence must appear before the second run.
  assert!(stdout.contains("\x1B[2J"), "expected ANSI clear escape in output");
  assert!(
    stdout.matches("stamped").count() >= 2,
    "expected at least 2 task runs in output: {stdout}"
  );

  Ok(())
}

#[test]
fn test_watch_mkignore_suppresses_ignored_file_changes() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;

  // Two files: one matched by .mkignore, one not.
  let ignored_file = temp_dir.path().join("build.log");
  let watched_file = temp_dir.path().join("src.txt");
  std::fs::write(&ignored_file, "old")?;
  std::fs::write(&watched_file, "old")?;

  // .mkignore lives at config root (same dir as tasks.yaml).
  std::fs::write(temp_dir.path().join(".mkignore"), "*.log\n")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      count:
        commands:
          - command: echo run
            verbose: false
    ",
  )?;

  let mut child = std::process::Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("count")
    .arg("--path")
    .arg(temp_dir.path())
    .arg("--debounce")
    .arg("100ms")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;

  // Wait for watcher setup.
  std::thread::sleep(std::time::Duration::from_millis(600));

  // Touch only the ignored file — should NOT trigger a rerun.
  std::fs::write(&ignored_file, "new")?;
  std::thread::sleep(std::time::Duration::from_millis(400));

  // Touch the non-ignored file — SHOULD trigger a rerun.
  std::fs::write(&watched_file, "new")?;
  std::thread::sleep(std::time::Duration::from_millis(600));

  child.kill()?;
  let output = child.wait_with_output()?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  // At least 2 runs: initial + one rerun from src.txt change.
  let runs = stdout.matches("run").count();
  assert!(
    runs >= 2,
    "expected >=2 runs (initial + src.txt rerun), got {runs} in:\n{stdout}"
  );

  Ok(())
}

#[test]
fn test_watch_mkignore_negated_pattern_reinclude() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;

  let special_log = temp_dir.path().join("important.log");
  std::fs::write(&special_log, "old")?;

  // Ignore all *.log but re-include important.log via negated pattern.
  std::fs::write(temp_dir.path().join(".mkignore"), "*.log\n!important.log\n")?;

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      stamp:
        commands:
          - command: echo stamped
            verbose: false
    ",
  )?;

  let mut child = std::process::Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("-c")
    .arg(&config_file_path)
    .arg("watch")
    .arg("stamp")
    .arg("--path")
    .arg(temp_dir.path())
    .arg("--debounce")
    .arg("100ms")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;

  std::thread::sleep(std::time::Duration::from_millis(600));

  // important.log is re-included, so this should trigger a rerun.
  std::fs::write(&special_log, "new")?;
  std::thread::sleep(std::time::Duration::from_millis(600));

  child.kill()?;
  let output = child.wait_with_output()?;

  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.matches("stamped").count() >= 2,
    "expected rerun from re-included path: {stdout}"
  );

  Ok(())
}
