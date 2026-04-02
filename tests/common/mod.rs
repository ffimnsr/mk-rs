use std::fs::File;
use std::io::Write as _;

use anyhow::Context as _;
use assert_fs::TempDir;

// Helper function to create a hello.yaml file
// Temp directory is referenced as when it goes out of scope, it will be deleted
pub fn setup_hello_yaml(temp_dir: &TempDir) -> anyhow::Result<String> {
  setup_yaml(
    temp_dir,
    "hello.yaml",
    "
    tasks:
      hello:
        commands:
          - command: echo \"Hello, world!\"
            verbose: true
        description: This is a task
  ",
  )
}

pub fn setup_yaml(temp_dir: &TempDir, file_name: &str, contents: &str) -> anyhow::Result<String> {
  let config_file = temp_dir.path().join(file_name);
  let mut config = File::create(config_file.clone())?;
  writeln!(config, "{}", contents)?;
  let config_file_path: &str = config_file
    .to_str()
    .with_context(|| "Failed to convert path to string")?;

  Ok(config_file_path.to_string())
}
