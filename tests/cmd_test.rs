use assert_cmd::Command;
use assert_fs::TempDir;

mod common;

#[test]
fn test_sanity() {
  assert_eq!(2 + 2, 4);
}

#[test]
fn test_mk_1() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("-h").assert();
  assert
    .success()
    .stdout(predicates::str::contains("Yet another simple task runner"))
    .stdout(predicates::str::contains("run"))
    .stdout(predicates::str::contains("list"))
    .stdout(predicates::str::contains("completions"))
    .stdout(predicates::str::contains("help"))
    .stdout(predicates::str::contains("--config"))
    .stdout(predicates::str::contains("--help"))
    .stdout(predicates::str::contains("--version"));
  Ok(())
}

#[test]
fn test_mk_2() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
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
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("-c").arg(&config_file_path).arg("ls").assert();
  assert.success().stdout(predicates::str::contains("hello"));
  Ok(())
}

#[test]
fn test_mk_4() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("help").assert();
  assert
    .success()
    .stdout(predicates::str::contains("mk is a powerful and flexible task runner designed to help you automate and manage your tasks efficiently"))
    .stdout(predicates::str::contains("run"))
    .stdout(predicates::str::contains("list"))
    .stdout(predicates::str::contains("completion"))
    .stdout(predicates::str::contains("secrets"))
    .stdout(predicates::str::contains("help"))
    .stdout(predicates::str::contains("--config"))
    .stdout(predicates::str::contains("--help"))
    .stdout(predicates::str::contains("--version"));
  Ok(())
}

#[test]
fn test_mk_5() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::cargo_bin("mk")?;
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
  let mut cmd = Command::cargo_bin("mk")?;
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
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("u").assert();
  assert
    .failure()
    .code(1)
    .stderr(predicates::str::contains("Task not found"));
  Ok(())
}

#[test]
fn test_mk_8() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
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
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("-c").arg(&config_file_path).arg("hello").assert();
  assert.success();
  Ok(())
}

#[test]
fn test_mk_10() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("-c").arg(&config_file_path).arg("hello0").assert();
  assert
    .failure()
    .stderr(predicates::str::contains("Task not found"));
  Ok(())
}

#[test]
fn test_mk_11() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let config_file_path = common::setup_hello_yaml(&temp_dir)?;
  let mut cmd = Command::cargo_bin("mk")?;
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
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("-c").arg(&config_file_path).arg("run").assert();
  assert.failure();
  Ok(())
}

#[test]
fn test_mk_13() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("run").assert();
  assert.failure();
  Ok(())
}

#[test]
fn test_mk_14() -> anyhow::Result<()> {
  let mut cmd = Command::cargo_bin("mk")?;
  let assert = cmd.arg("r").assert();
  assert.failure();
  Ok(())
}
