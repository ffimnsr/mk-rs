use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UseCargoArgs {
  /// The package manager to use
  #[serde(default)]
  pub package_manager: String,

  /// The working directory to run the command in
  #[serde(default)]
  pub work_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UseCargo {
  Bool(bool),
  UseCargo(Box<UseCargoArgs>),
}

impl UseCargo {
  pub fn capture(&self) -> anyhow::Result<()> {
    match self {
      UseCargo::Bool(true) => self.capture_tasks(),
      UseCargo::UseCargo(args) => args.capture_tasks(),
      _ => Ok(()),
    }
  }

  fn capture_tasks(&self) -> anyhow::Result<()> {
    unimplemented!()
  }
}

impl UseCargoArgs {
  pub fn capture_tasks(&self) -> anyhow::Result<()> {
    unimplemented!()
  }
}
