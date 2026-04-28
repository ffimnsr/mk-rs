use std::fs;
use std::path::PathBuf;

use assert_cmd::{
  cargo,
  Command,
};
use assert_fs::TempDir;
use mk_lib::file::ToUtf8 as _;

mod common;

fn snapshot_path(name: &str) -> String {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests")
    .join("snapshots")
    .join(name)
    .to_string_lossy()
    .into_owned()
}

fn assert_snapshot(name: &str, actual: &str) -> anyhow::Result<()> {
  let expected = fs::read_to_string(snapshot_path(name))?;
  let actual = common::normalize_snapshot_text(actual, &[]);
  let expected = common::normalize_snapshot_text(&expected, &[]);
  assert_eq!(actual.trim_end_matches('\n'), expected.trim_end_matches('\n'));
  Ok(())
}

#[test]
fn snapshot_help() -> anyhow::Result<()> {
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("--help")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  assert_snapshot("help.snap", &String::from_utf8(output)?)
}

#[test]
fn snapshot_list_help() -> anyhow::Result<()> {
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("list")
    .arg("--help")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  assert_snapshot("list-help.snap", &String::from_utf8(output)?)
}

#[test]
fn snapshot_run_help() -> anyhow::Result<()> {
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("run")
    .arg("--help")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  assert_snapshot("run-help.snap", &String::from_utf8(output)?)
}

#[test]
fn snapshot_completion_help() -> anyhow::Result<()> {
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("completion")
    .arg("--help")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  assert_snapshot("completion-help.snap", &String::from_utf8(output)?)
}

#[test]
fn snapshot_doctor_help() -> anyhow::Result<()> {
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("doctor")
    .arg("--help")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  assert_snapshot("doctor-help.snap", &String::from_utf8(output)?)
}

#[test]
fn snapshot_watch_help() -> anyhow::Result<()> {
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("watch")
    .arg("--help")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  assert_snapshot("watch-help.snap", &String::from_utf8(output)?)
}

#[test]
fn snapshot_list_json() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      zebra:
        commands:
          - command: echo zebra
            verbose: false
      alpha:
        description: alpha task
        commands:
          - command: echo alpha
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
  assert_snapshot("list-json.snap", &String::from_utf8(output)?)
}

#[test]
fn snapshot_validate_json() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      zebra:
        depends_on:
          - missing
        outputs:
          - output.txt
        cache:
          enabled: true
        commands:
          - command: printf zebra > output.txt
            verbose: false
      alpha:
        commands:
          - command: echo alpha
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("validate")
    .arg("--json")
    .assert()
    .failure()
    .code(1)
    .get_output()
    .stdout
    .clone();
  assert_snapshot("validate-json.snap", &String::from_utf8(output)?)
}

#[test]
fn snapshot_plan_json() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "tasks.yaml",
    "
    tasks:
      build:
        description: build task
        depends_on:
          - check
        commands:
          - command: echo build
            verbose: false
      check:
        commands:
          - command: echo check
            verbose: false
    ",
  )?;
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("-c")
    .arg(&config_file_path)
    .arg("plan")
    .arg("build")
    .arg("--json")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let temp_root = temp_dir.path().to_utf8()?;
  let actual = common::normalize_snapshot_text(&String::from_utf8(output)?, &[(temp_root, "<TMPDIR>")]);
  assert_snapshot("plan-json.snap", &actual)
}

#[test]
fn snapshot_schema() -> anyhow::Result<()> {
  let output = Command::new(cargo::cargo_bin!("mk"))
    .arg("schema")
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  assert_snapshot("schema.snap", &String::from_utf8(output)?)
}

#[test]
fn manpage_generator_writes_root_and_nested_pages() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_dir = temp_dir.path().join("man1");

  Command::new(cargo::cargo_bin!("mk-manpages"))
    .arg(&output_dir)
    .assert()
    .success();

  let root = fs::read_to_string(output_dir.join("mk.1"))?;
  let nested = fs::read_to_string(output_dir.join("mk-secrets-vault-store-secret.1"))?;

  assert!(root.contains(".TH mk 1"));
  assert!(root.contains("mk\\-watch(1)"));
  assert!(nested.contains(".TH mk-secrets-vault-store-secret 1"));
  assert!(nested.contains("\\fBmk secrets vault store\\-secret\\fR"));

  Ok(())
}

#[test]
fn snapshot_init_yaml() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_path = temp_dir.path().join("tasks.yaml");
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("init")
    .arg(output_path.to_utf8()?)
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = common::normalize_snapshot_text(
    &String::from_utf8(output)?,
    &[(output_path.to_utf8()?, "<OUTPUT_PATH>")],
  );
  assert_snapshot("init-yaml.stdout.snap", &stdout)?;
  let contents = fs::read_to_string(&output_path)?;
  assert_snapshot(
    "init-yaml.contents.snap",
    &common::normalize_snapshot_text(&contents, &[]),
  )
}

#[test]
fn snapshot_init_yml() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_path = temp_dir.path().join("tasks.yml");
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("init")
    .arg(output_path.to_utf8()?)
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = common::normalize_snapshot_text(
    &String::from_utf8(output)?,
    &[(output_path.to_utf8()?, "<OUTPUT_PATH>")],
  );
  assert_snapshot("init-yml.stdout.snap", &stdout)?;
  let contents = fs::read_to_string(&output_path)?;
  assert_snapshot(
    "init-yml.contents.snap",
    &common::normalize_snapshot_text(&contents, &[]),
  )
}

#[test]
fn snapshot_init_toml() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_path = temp_dir.path().join("mk.toml");
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("init")
    .arg(output_path.to_utf8()?)
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = common::normalize_snapshot_text(
    &String::from_utf8(output)?,
    &[(output_path.to_utf8()?, "<OUTPUT_PATH>")],
  );
  assert_snapshot("init-toml.stdout.snap", &stdout)?;
  let contents = fs::read_to_string(&output_path)?;
  assert_snapshot(
    "init-toml.contents.snap",
    &common::normalize_snapshot_text(&contents, &[]),
  )
}

#[test]
fn snapshot_init_json() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_path = temp_dir.path().join("tasks.json");
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("init")
    .arg(output_path.to_utf8()?)
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = common::normalize_snapshot_text(
    &String::from_utf8(output)?,
    &[(output_path.to_utf8()?, "<OUTPUT_PATH>")],
  );
  assert_snapshot("init-json.stdout.snap", &stdout)?;
  let contents = fs::read_to_string(&output_path)?;
  assert_snapshot(
    "init-json.contents.snap",
    &common::normalize_snapshot_text(&contents, &[]),
  )
}

#[test]
fn snapshot_init_lua() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_path = temp_dir.path().join("tasks.lua");
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("init")
    .arg(output_path.to_utf8()?)
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let stdout = common::normalize_snapshot_text(
    &String::from_utf8(output)?,
    &[(output_path.to_utf8()?, "<OUTPUT_PATH>")],
  );
  assert_snapshot("init-lua.stdout.snap", &stdout)?;
  let contents = fs::read_to_string(&output_path)?;
  assert_snapshot(
    "init-lua.contents.snap",
    &common::normalize_snapshot_text(&contents, &[]),
  )
}

#[test]
fn snapshot_init_unsupported_extension() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_path = temp_dir.path().join("tasks.txt");
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .env_remove("RUST_BACKTRACE")
    .env_remove("RUST_LIB_BACKTRACE")
    .arg("init")
    .arg(output_path.to_utf8()?)
    .assert()
    .failure()
    .code(1)
    .get_output()
    .stderr
    .clone();
  assert_snapshot(
    "init-unsupported.stderr.snap",
    &common::normalize_snapshot_text(&String::from_utf8(output)?, &[]),
  )
}
