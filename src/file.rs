use std::ffi::OsStr;
use std::path::Path;

/// This has been adapted from cross-rs file.rs source
/// https://github.com/cross-rs/cross/blob/4090beca3cfffa44371a5bba524de3a578aa46c3/src/file.rs#L12
pub trait ToUtf8 {
  fn to_utf8(&self) -> anyhow::Result<&str>;
}

/// Implement `ToUtf8` for `OsStr`
impl ToUtf8 for OsStr {
  /// Convert `OsStr` to `&str`
  /// This function will return an error if the conversion fails
  fn to_utf8(&self) -> anyhow::Result<&str> {
    self
      .to_str()
      .ok_or_else(|| anyhow::anyhow!("Unable to convert `{self:?}` to UTF-8 string"))
  }
}

/// Implement `ToUtf8` for `Path`
impl ToUtf8 for Path {
  /// Convert `Path` to `&str`
  fn to_utf8(&self) -> anyhow::Result<&str> {
    self.as_os_str().to_utf8()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_os_str_to_utf8() -> anyhow::Result<()> {
    let os_str = OsStr::new("hello");
    assert_eq!(os_str.to_utf8()?, "hello");
    Ok(())
  }

  #[test]
  fn test_path_to_utf8() -> anyhow::Result<()> {
    let path = Path::new("hello");
    assert_eq!(path.to_utf8()?, "hello");
    Ok(())
  }
}
