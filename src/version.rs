/// Get the version of the program from the environment variables
pub fn get_version_digits() -> String {
  let semver = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
  match option_env!("MK_BUILD_GIT_HASH") {
    None => semver.to_string(),
    Some(hash) => format!("{} (rev {})", semver, hash),
  }
}
