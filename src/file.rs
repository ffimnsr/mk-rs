use std::ffi::OsStr;
use std::path::Path;

/// This has been adapted from cross-rs file.rs source
/// https://github.com/cross-rs/cross/blob/4090beca3cfffa44371a5bba524de3a578aa46c3/src/file.rs#L12
pub trait ToUtf8 {
  fn to_utf8(&self) -> anyhow::Result<&str>;
}

impl ToUtf8 for OsStr {
  fn to_utf8(&self) -> anyhow::Result<&str> {
    self
      .to_str()
      .ok_or_else(|| anyhow::anyhow!("unable to convert `{self:?}` to UTF-8 string"))
  }
}

impl ToUtf8 for Path {
  fn to_utf8(&self) -> anyhow::Result<&str> {
    self.as_os_str().to_utf8()
  }
}
