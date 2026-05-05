use std::collections::HashMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::Deserialize;

/// Conditional execution predicate for a task.
///
/// All provided keys are evaluated before the task runs. A task is skipped
/// when any declared condition fails. Unknown keys are rejected at parse time.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WhenCondition {
  /// Target OS values that permit execution. The task is skipped when the
  /// current OS does not appear in this list.
  #[serde(default)]
  pub os: Vec<String>,

  /// Environment variable conditions in `KEY=VALUE` format. The task is
  /// skipped when any entry does not match the current environment.
  #[serde(default)]
  pub env: Vec<String>,

  /// Filesystem paths that must exist before the task runs.
  #[serde(default)]
  pub file_exists: Vec<String>,

  /// Command names that must be present on `PATH` before the task runs.
  #[serde(default)]
  pub command_exists: Vec<String>,
}

/// Outcome of evaluating a [`WhenCondition`].
#[derive(Debug, PartialEq, Eq)]
pub enum WhenOutcome {
  /// All conditions passed; task should run.
  Run,
  /// A condition failed; task should be skipped. Contains a human-readable
  /// reason.
  Skip(String),
}

impl WhenCondition {
  /// Returns `true` when no condition entries are declared.
  pub fn is_empty(&self) -> bool {
    self.os.is_empty() && self.env.is_empty() && self.file_exists.is_empty() && self.command_exists.is_empty()
  }

  /// Evaluate all conditions against the current runtime environment.
  ///
  /// `env` is the resolved task environment map. Conditions are evaluated in
  /// declaration order; the first failure short-circuits and returns its reason.
  pub fn evaluate(&self, env: &HashMap<String, String>) -> WhenOutcome {
    // --- when.os ---
    if !self.os.is_empty() {
      let current_os = std::env::consts::OS;
      if !self.os.iter().any(|v| v == current_os) {
        return WhenOutcome::Skip(format!(
          "when.os: current OS '{}' not in allowed list [{}]",
          current_os,
          self.os.join(", "),
        ));
      }
    }

    // --- when.env ---
    for entry in &self.env {
      // Entry format already validated as KEY=VALUE; split on first '='.
      let (key, expected_value) = match entry.split_once('=') {
        Some(pair) => pair,
        None => {
          // Malformed entry — treat as failure (validation should have caught it).
          return WhenOutcome::Skip(format!("when.env: malformed entry '{}'", entry));
        },
      };

      // Task env takes precedence; fall back to process env.
      let resolved: Option<String> = env.get(key).cloned().or_else(|| std::env::var(key).ok());

      match resolved.as_deref() {
        Some(actual) if actual == expected_value => {},
        Some(actual) => {
          return WhenOutcome::Skip(format!(
            "when.env: '{}' is '{}', expected '{}'",
            key, actual, expected_value,
          ));
        },
        None => {
          return WhenOutcome::Skip(format!("when.env: '{}' is not set", key));
        },
      }
    }

    // --- when.file_exists ---
    for path_str in &self.file_exists {
      if !Path::new(path_str).exists() {
        return WhenOutcome::Skip(format!("when.file_exists: '{}' does not exist", path_str));
      }
    }

    // --- when.command_exists ---
    for cmd in &self.command_exists {
      if !command_on_path(cmd) {
        return WhenOutcome::Skip(format!("when.command_exists: '{}' not found on PATH", cmd));
      }
    }

    WhenOutcome::Run
  }
}

/// Check whether `cmd` is available on `PATH`.
fn command_on_path(cmd: &str) -> bool {
  std::env::var_os("PATH")
    .map(|path_var| {
      std::env::split_paths(&path_var).any(|dir| {
        let candidate = dir.join(cmd);
        candidate.is_file() || {
          // On Windows commands may have an executable extension.
          #[cfg(windows)]
          {
            ["exe", "cmd", "bat", "com"]
              .iter()
              .any(|ext| dir.join(format!("{}.{}", cmd, ext)).is_file())
          }
          #[cfg(not(windows))]
          false
        }
      })
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn env_from(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect()
  }

  #[test]
  fn test_empty_when_always_runs() {
    let when = WhenCondition::default();
    assert_eq!(when.evaluate(&HashMap::new()), WhenOutcome::Run);
  }

  #[test]
  fn test_os_match_runs() {
    let when = WhenCondition {
      os: vec![std::env::consts::OS.to_string()],
      ..Default::default()
    };
    assert_eq!(when.evaluate(&HashMap::new()), WhenOutcome::Run);
  }

  #[test]
  fn test_os_mismatch_skips() {
    let when = WhenCondition {
      os: vec!["this-os-does-not-exist".to_string()],
      ..Default::default()
    };
    assert!(matches!(when.evaluate(&HashMap::new()), WhenOutcome::Skip(_)));
  }

  #[test]
  fn test_env_match_runs() {
    let when = WhenCondition {
      env: vec!["FOO=bar".to_string()],
      ..Default::default()
    };
    let env = env_from(&[("FOO", "bar")]);
    assert_eq!(when.evaluate(&env), WhenOutcome::Run);
  }

  #[test]
  fn test_env_mismatch_skips() {
    let when = WhenCondition {
      env: vec!["FOO=bar".to_string()],
      ..Default::default()
    };
    let env = env_from(&[("FOO", "baz")]);
    assert!(matches!(when.evaluate(&env), WhenOutcome::Skip(_)));
  }

  #[test]
  fn test_env_missing_skips() {
    let when = WhenCondition {
      env: vec!["__MK_UNIT_TEST_ABSENT_VAR__=value".to_string()],
      ..Default::default()
    };
    // Ensure this is not set in the test environment.
    std::env::remove_var("__MK_UNIT_TEST_ABSENT_VAR__");
    assert!(matches!(when.evaluate(&HashMap::new()), WhenOutcome::Skip(_)));
  }

  #[test]
  fn test_file_exists_present_runs() {
    // Use an absolute path that is guaranteed to exist.
    let abs = std::fs::canonicalize("Cargo.toml")
      .map(|p| p.to_string_lossy().into_owned())
      .unwrap_or_else(|_| "Cargo.toml".to_string());
    let when = WhenCondition {
      file_exists: vec![abs],
      ..Default::default()
    };
    assert_eq!(when.evaluate(&HashMap::new()), WhenOutcome::Run);
  }

  #[test]
  fn test_file_exists_missing_skips() {
    let when = WhenCondition {
      file_exists: vec!["/this/path/does/not/exist/ever".to_string()],
      ..Default::default()
    };
    assert!(matches!(when.evaluate(&HashMap::new()), WhenOutcome::Skip(_)));
  }

  #[test]
  fn test_command_exists_present_runs() {
    // Use a command that definitely exists on any CI/dev system.
    let cmd = if cfg!(windows) { "cmd" } else { "sh" };
    let when = WhenCondition {
      command_exists: vec![cmd.to_string()],
      ..Default::default()
    };
    assert_eq!(when.evaluate(&HashMap::new()), WhenOutcome::Run);
  }

  #[test]
  fn test_command_exists_missing_skips() {
    let when = WhenCondition {
      command_exists: vec!["this-command-does-not-exist-ever".to_string()],
      ..Default::default()
    };
    assert!(matches!(when.evaluate(&HashMap::new()), WhenOutcome::Skip(_)));
  }
}
