use assert_cmd::{
  cargo,
  Command,
};
use assert_fs::TempDir;
use mk_lib::file::ToUtf8 as _;

mod common;

#[test]
fn test_sanity() {
  assert_eq!(2 + 2, 4);
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
    .stderr(predicates::str::contains("Task not found"));
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
    .stderr(predicates::str::contains("Task not found"));
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
    .stdout(predicates::str::contains("\"severity\": \"error\""))
    .stdout(predicates::str::contains("Circular dependency detected"));
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

  let config_file_path = common::setup_yaml(
    &temp_dir,
    "env-cache.yaml",
    "
    tasks:
      build:
        env_file:
          - .env
        outputs:
          - output.txt
        cache:
          enabled: true
        commands:
          - command: printf '%s' \"$FOO\" > output.txt && echo run >> marker.txt
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
fn test_mk_36_container_run_preserves_named_volumes() -> anyhow::Result<()> {
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
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "outputs.yaml",
    &format!(
      "
    tasks:
      build:
        environment:
          IMAGE_TAG: tag-${{{{ outputs.version }}}}
        commands:
          - command: printf '1.2.3\\n'
            save_output_as: version
            verbose: false
          - command: printf '%s|%s' \"${{{{ outputs.version }}}}\" \"$IMAGE_TAG\" > {}
            verbose: false
    ",
      common::sh_path(&result_file)
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
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "multiline-output.yaml",
    &format!(
      "
    tasks:
      build:
        commands:
          - command: |
              printf 'line1\\nline2\\n\\n'
            save_output_as: block
            verbose: false
          - command: printf '%s' \"${{{{ outputs.block }}}}\" > {}
            verbose: false
    ",
      common::sh_path(&result_file)
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
    .stderr(predicates::str::contains("Failed to find task output - version"));

  Ok(())
}

#[test]
fn test_mk_40_nested_tasks_have_isolated_outputs() -> anyhow::Result<()> {
  let temp_dir = TempDir::new()?;
  let parent_file = temp_dir.path().join("parent.txt");
  let child_file = temp_dir.path().join("child.txt");
  let config_file_path = common::setup_yaml(
    &temp_dir,
    "nested-output.yaml",
    &format!(
      "
    tasks:
      root:
        commands:
          - command: printf 'parent\\n'
            save_output_as: shared
            verbose: false
          - task: child
          - command: printf '%s' \"${{{{ outputs.shared }}}}\" > {}
            verbose: false
      child:
        commands:
          - command: printf 'child\\n'
            save_output_as: shared
            verbose: false
          - command: printf '%s' \"${{{{ outputs.shared }}}}\" > {}
            verbose: false
    ",
      common::sh_path(&parent_file),
      common::sh_path(&child_file)
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
