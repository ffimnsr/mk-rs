use std::io::{ErrorKind, Write as _};
use std::process::{Command, Stdio};

use anyhow::Context as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskSelectorCandidate {
  pub(crate) name: String,
  pub(crate) description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
  Fzf,
  Sk,
}

impl Backend {
  fn command(self) -> &'static str {
    match self {
      Self::Fzf => "fzf",
      Self::Sk => "sk",
    }
  }

  fn args(self) -> [&'static str; 6] {
    ["--prompt", "task> ", "--delimiter", "\t", "--with-nth", "1,2"]
  }
}

pub(crate) struct FuzzyTaskSelector;

impl FuzzyTaskSelector {
  pub(crate) fn select(candidates: &[TaskSelectorCandidate]) -> anyhow::Result<String> {
    let backend = detect_backend()?;
    let lines = format_candidates(candidates);
    let mut child = Command::new(backend.command())
      .args(backend.args())
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()
      .with_context(|| format!("Failed to launch fuzzy finder `{}`", backend.command()))?;

    {
      let mut stdin = child
        .stdin
        .take()
        .with_context(|| format!("Failed to open stdin for fuzzy finder `{}`", backend.command()))?;
      for line in lines {
        if let Err(error) = writeln!(stdin, "{line}") {
          if error.kind() == ErrorKind::BrokenPipe {
            break;
          }
          return Err(error).with_context(|| format!("Failed to write task list to `{}`", backend.command()));
        }
      }
    }

    let output = child
      .wait_with_output()
      .with_context(|| format!("Failed to read selection from `{}`", backend.command()))?;

    if !output.status.success() {
      anyhow::bail!("No task selected");
    }

    let stdout = String::from_utf8(output.stdout)
      .with_context(|| format!("`{}` returned invalid UTF-8 output", backend.command()))?;
    parse_selection(&stdout).ok_or_else(|| anyhow::anyhow!("No task selected"))
  }
}

fn detect_backend() -> anyhow::Result<Backend> {
  if which::which("fzf").is_ok() {
    return Ok(Backend::Fzf);
  }
  if which::which("sk").is_ok() {
    return Ok(Backend::Sk);
  }
  anyhow::bail!("No fuzzy finder found. Install fzf or skim (sk).");
}

fn format_candidates(candidates: &[TaskSelectorCandidate]) -> Vec<String> {
  let width = candidates
    .iter()
    .map(|candidate| candidate.name.chars().count())
    .max()
    .unwrap_or(0);

  candidates
    .iter()
    .map(|candidate| {
      format!(
        "{name:<width$}\t{description}",
        name = candidate.name,
        description = candidate.description,
      )
    })
    .collect()
}

fn parse_selection(stdout: &str) -> Option<String> {
  let line = stdout.lines().find(|line| !line.trim().is_empty())?;
  let name = line.split_once('\t').map(|(name, _)| name).unwrap_or(line).trim();
  if name.is_empty() {
    return None;
  }
  Some(name.to_owned())
}

#[cfg(test)]
mod tests {
  use super::{format_candidates, parse_selection, TaskSelectorCandidate};

  #[cfg(unix)]
  use super::{detect_backend, Backend};

  #[test]
  fn parse_selection_reads_first_tsv_column() {
    assert_eq!(
      parse_selection("build    \tBuild project\n"),
      Some(String::from("build"))
    );
  }

  #[test]
  fn parse_selection_rejects_empty_output() {
    assert_eq!(parse_selection("\n"), None);
  }

  #[test]
  fn format_candidates_aligns_description_column() {
    let candidates = vec![
      TaskSelectorCandidate {
        name: String::from("lint"),
        description: String::from("Lint project"),
      },
      TaskSelectorCandidate {
        name: String::from("uninstall-githooks"),
        description: String::from("Uninstall git hooks"),
      },
    ];

    assert_eq!(
      format_candidates(&candidates),
      vec![
        String::from("lint              \tLint project"),
        String::from("uninstall-githooks\tUninstall git hooks"),
      ]
    );
  }

  #[cfg(unix)]
  #[test]
  fn detect_backend_prefers_fzf() -> anyhow::Result<()> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    let dir = std::env::temp_dir().join(format!(
      "mk-task-selector-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos()
    ));
    fs::create_dir_all(&dir)?;
    let fzf = dir.join("fzf");
    let sk = dir.join("sk");
    fs::write(&fzf, "#!/bin/sh\nexit 0\n")?;
    fs::write(&sk, "#!/bin/sh\nexit 0\n")?;
    fs::set_permissions(&fzf, fs::Permissions::from_mode(0o755))?;
    fs::set_permissions(&sk, fs::Permissions::from_mode(0o755))?;

    let original_path = std::env::var_os("PATH");
    unsafe {
      std::env::set_var("PATH", &dir);
    }
    let backend = detect_backend()?;
    match original_path {
      Some(path) => unsafe { std::env::set_var("PATH", path) },
      None => unsafe { std::env::remove_var("PATH") },
    }
    let _ = fs::remove_file(&fzf);
    let _ = fs::remove_file(&sk);
    let _ = fs::remove_dir(&dir);

    assert_eq!(backend, Backend::Fzf);
    Ok(())
  }
}
