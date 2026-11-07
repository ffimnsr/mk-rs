use std::env;
use std::process::{Command, Stdio};

/// Print local GPG secret key IDs for shell completion when requested.
pub(crate) fn maybe_print_gpg_key_completion_from_env() -> anyhow::Result<bool> {
  if env::var_os("MK_COMPLETE_GPG_KEYS").is_none() {
    return Ok(false);
  }

  let prefix = env::var("MK_COMPLETE_PREFIX").ok();
  for id in list_secret_key_ids() {
    if prefix_matches(&id, prefix.as_deref()) {
      println!("{id}");
    }
  }

  Ok(true)
}

fn prefix_matches(value: &str, prefix: Option<&str>) -> bool {
  prefix
    .map(|prefix| {
      value
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
    })
    .unwrap_or(true)
}

fn list_secret_key_ids() -> Vec<String> {
  let Ok(output) = Command::new("gpg")
    .args([
      "--batch",
      "--no-tty",
      "--list-secret-keys",
      "--with-colons",
      "--keyid-format=long",
    ])
    .stdin(Stdio::null())
    .output()
  else {
    return Vec::new();
  };

  if !output.status.success() {
    return Vec::new();
  }

  parse_secret_key_ids(&String::from_utf8_lossy(&output.stdout))
}

fn parse_secret_key_ids(output: &str) -> Vec<String> {
  let mut ids = output
    .lines()
    .filter_map(|line| {
      let fields = line.split(':').collect::<Vec<_>>();
      if fields.first().copied() != Some("sec") {
        return None;
      }

      fields.get(4).filter(|id| !id.is_empty()).map(|id| id.to_string())
    })
    .collect::<Vec<_>>();
  ids.sort_unstable();
  ids.dedup();
  ids
}

#[cfg(test)]
mod tests {
  use super::{parse_secret_key_ids, prefix_matches};

  #[test]
  fn parse_secret_key_ids_extracts_primary_secret_keys() {
    let output = "\
sec:u:4096:1:105AF578CF730F08:1700000000:::::::
fpr:::::::::8A1B2C3D4E5F60718293A4B5C6D7E8F901234567:
ssb:u:4096:1:ABCDEF0123456789:1700000000:::::::
sec:u:256:22:0123456789ABCDEF:1700000000:::::::
";

    assert_eq!(
      parse_secret_key_ids(output),
      vec!["0123456789ABCDEF".to_string(), "105AF578CF730F08".to_string()]
    );
  }

  #[test]
  fn parse_secret_key_ids_ignores_empty_key_ids() {
    assert!(parse_secret_key_ids("sec:u:4096:1::1700000000:::::::\n").is_empty());
  }

  #[test]
  fn prefix_matches_ignores_ascii_case() {
    assert!(prefix_matches("105AF578CF730F08", Some("105a")));
    assert!(!prefix_matches("105AF578CF730F08", Some("ABC")));
    assert!(prefix_matches("105AF578CF730F08", None));
  }
}
