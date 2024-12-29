use std::fs::File;
use std::io::Write as _;

use anyhow::Context;
use assert_cmd::Command;
use tempfile::{tempdir, TempDir};

#[test]
fn test_sanity() {
  assert_eq!(2 + 2, 4);
}

#[test]
fn test_mk_1() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("-h").assert();

  assert.success();
  Ok(())
}

#[test]
fn test_mk_2() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("--version").assert();

  let version = env!("CARGO_PKG_VERSION");
  let version_str = format!("mk {}", version);

  assert.success()
    .stdout(predicates::str::contains(version_str));
  Ok(())
}

#[test]
fn test_mk_3() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("ls").assert();
  assert.success()
    .stdout(predicates::str::contains("build-in-container"))
    .stdout(predicates::str::contains("check"));
  Ok(())
}

#[test]
fn test_mk_4() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("help").assert();
  assert.success();
  Ok(())
}

#[test]
fn test_mk_5() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("run").assert();
  assert.success();
  Ok(())
}

#[test]
fn test_mk_6() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("r").assert();
  assert.success();
  Ok(())
}

#[test]
fn test_mk_7() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("u").assert();
  assert.failure()
    .code(1)
    .stderr(predicates::str::contains("Task not found"));
  Ok(())
}


#[test]
fn test_mk_8() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("-c").arg("hello.yaml").assert();
  assert.failure()
    .code(1)
    .stderr(predicates::str::contains("No such file or directory"));
  Ok(())
}

// Helper function to create a hello.yaml file
// Temp directory is referenced as when it goes out of scope, it will be deleted
fn setup_hello_yaml(temp_dir: &TempDir) -> anyhow::Result<String> {
  let config_file = temp_dir.path().join("hello.yaml");
  let mut config = File::create(config_file.clone())?;
  let yaml_config = "
    tasks:
      hello:
        commands:
          - command: echo \"Hello, world!\"
            verbose: true
        description: This is a task
  ";

  writeln!(config, "{}", yaml_config)?;
  let config_file_path: &str = &config_file.to_str()
    .with_context(|| "Failed to convert path to string")?;

  Ok(config_file_path.to_string())
}

#[test]
fn test_mk_9() -> anyhow::Result<()> {
  let temp_dir = tempdir()?;
  let config_file_path = setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("-c").arg(&config_file_path).arg("hello").assert();
  assert.success();
  Ok(())
}

#[test]
fn test_mk_10() -> anyhow::Result<()> {
  let temp_dir = tempdir()?;
  let config_file_path = setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("-c").arg(&config_file_path).arg("hello0").assert();
  assert.failure()
    .stderr(predicates::str::contains("Task not found"));
  Ok(())
}
