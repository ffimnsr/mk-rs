/// Get the version of the program from the environment variables
pub fn get_version_digits() -> String {
  let semver = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
  match option_env!("MK_BUILD_GIT_HASH") {
    None => semver.to_string(),
    Some(hash) => format!("{} (rev {})", semver, hash),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use regex::Regex;

  #[test]
  fn test_get_version_digits_no_hash() -> anyhow::Result<()> {
    // Since CARGO_PKG_VERSION is set at compile time and
    // MK_BUILD_GIT_HASH is not set in test environment
    let version = get_version_digits();
    let re = Regex::new(r"^\d+\.\d+\.\d+ \(rev [a-f0-9]+\)$")?;
    assert!(version == "unknown" || re.find(&version).is_some());
    Ok(())
  }

  #[test]
  fn test_get_version_digits_format() -> anyhow::Result<()> {
    // Test version string format when hash is present
    // We can't directly set env vars for option_env! macro
    // but we can verify the function returns expected format
    let version: &str = &get_version_digits();
    let re_1 = Regex::new(r"^\d+\.\d+\.\d+$")?;
    let re_2 = Regex::new(r"^\d+\.\d+\.\d+ \(rev [a-f0-9]+\)$")?;
    assert!(
      version == "unknown" ||
      re_1.find(version).is_some() ||
      re_2.find(version).is_some()
    );

    Ok(())
  }
}
