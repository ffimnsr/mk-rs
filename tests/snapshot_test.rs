use std::fs;

use assert_cmd::{
  cargo,
  Command,
};
use assert_fs::TempDir;
use mk_lib::file::ToUtf8 as _;

mod common;

fn snapshot_path(name: &str) -> String {
  format!(
    "/home/pastel/Projects/playground/rust/mk-rs/tests/snapshots/{}",
    name
  )
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
fn snapshot_init_toml_rejected() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let output_path = temp_dir.path().join("mk.toml");
  let output = Command::new(cargo::cargo_bin!("mk"))
    .current_dir(temp_dir.path())
    .arg("init")
    .arg(output_path.to_utf8()?)
    .assert()
    .failure()
    .code(1)
    .get_output()
    .stderr
    .clone();
  assert_snapshot(
    "init-toml.stderr.snap",
    &common::normalize_snapshot_text(&String::from_utf8(output)?, &[]),
  )
}
