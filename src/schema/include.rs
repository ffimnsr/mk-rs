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
      Include::String(name) => self.capture_root(name),
      Include::Include(args) => args.capture_root(),
    }
  }

  fn capture_root(&self, name: &str) -> anyhow::Result<()> {
    IncludeArgs {
      name: name.to_string(),
      overwrite: false,
    }
    .capture_root()
  }
}

impl IncludeArgs {
  pub fn capture_root(&self) -> anyhow::Result<()> {
    unimplemented!()
  }
}
