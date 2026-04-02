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
  pub fn name(&self) -> &str {
    match self {
      Include::String(name) => name,
      Include::Include(args) => &args.name,
    }
  }

  pub fn overwrite(&self) -> bool {
    match self {
      Include::String(_) => false,
      Include::Include(args) => args.overwrite,
    }
  }
}
