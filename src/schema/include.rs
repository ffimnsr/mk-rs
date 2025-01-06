use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct IncludeArgs {
  pub name: String,

  #[serde(default)]
  pub overwrite: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Include {
  String(String),
  Include(Box<IncludeArgs>),
}

impl Include {
  pub fn capture(&self) -> anyhow::Result<()> {
    match self {
      Include::String(_) => self.capture_root(),
      Include::Include(args) => args.capture_root(),
    }
  }

  fn capture_root(&self) -> anyhow::Result<()> {
    unimplemented!()
  }
}

impl IncludeArgs {
  pub fn capture_root(&self) -> anyhow::Result<()> {
    unimplemented!()
  }
}
