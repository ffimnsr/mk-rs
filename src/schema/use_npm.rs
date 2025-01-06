use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UseNpmArgs {
  /// The package manager to use
  #[serde(default)]
  pub package_manager: String,

  /// The working directory to run the command in
  #[serde(default)]
  pub work_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UseNpm {
  Bool(bool),
  UseNpm(Box<UseNpmArgs>),
}

impl UseNpm {
  pub fn capture(&self) -> anyhow::Result<()> {
    match self {
      UseNpm::Bool(true) => self.capture_tasks(),
      UseNpm::UseNpm(args) => args.capture_tasks(),
      _ => Ok(()),
    }
  }

  fn capture_tasks(&self) -> anyhow::Result<()> {
    unimplemented!()
  }
}

impl UseNpmArgs {
  pub fn capture_tasks(&self) -> anyhow::Result<()> {
    unimplemented!()
  }
}
