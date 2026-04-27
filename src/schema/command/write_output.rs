use schemars::JsonSchema;
use serde::Deserialize;

use crate::file::ToUtf8 as _;
use crate::schema::{
  interpolate_template_string,
  TaskContext,
};

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub struct WriteOutput {
  /// The name of a previously saved output to write to disk
  pub write_output: String,

  /// The file path to write the output to
  pub to_file: String,

  /// Create parent directories when they do not exist
  #[serde(default)]
  pub create_parents: Option<bool>,
}

impl WriteOutput {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    let source_name = interpolate_template_string(&self.write_output, context)?;
    let dest_path_str = interpolate_template_string(&self.to_file, context)?;
    let dest_path = context.resolve_from_config(&dest_path_str);

    let value = context
      .get_task_output(&source_name)?
      .ok_or_else(|| anyhow::anyhow!("Task output '{}' is not available", source_name))?;

    if dest_path.exists() {
      let dest_path = dest_path.to_utf8()?;
      anyhow::bail!(
        "Output file already exists: {}. Remove it before writing.",
        dest_path
      );
    }

    if self.create_parents.unwrap_or(false) {
      if let Some(parent) = dest_path.parent() {
        if !parent.as_os_str().is_empty() {
          let dest_path = dest_path.to_utf8()?.to_string();
          std::fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("Failed to create parent directories for '{}': {}", dest_path, e))?;
        }
      }
    }

    let dest_path_utf8 = dest_path.to_utf8()?.to_string();
    std::fs::write(&dest_path, value.as_bytes())
      .map_err(|e| anyhow::anyhow!("Failed to write output to '{}': {}", dest_path_utf8, e))?;

    Ok(())
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use tempfile::TempDir;

  use super::*;
  use crate::schema::{
    TaskContext,
    TaskRoot,
  };

  fn ctx_with_output(name: &str, value: &str) -> (TaskContext, Arc<TaskRoot>) {
    let root = Arc::new(TaskRoot::default());
    let context = TaskContext::empty_with_root(root.clone());
    context.insert_task_output(name, value).unwrap();
    (context, root)
  }

  #[test]
  fn test_write_output_to_new_file() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let dest = tmp.path().join("out.txt");
    let (context, _) = ctx_with_output("data", "hello world");
    let cmd = WriteOutput {
      write_output: "data".into(),
      to_file: dest.to_string_lossy().into_owned(),
      create_parents: None,
    };
    cmd.execute(&context)?;
    assert_eq!(std::fs::read_to_string(&dest)?, "hello world");
    Ok(())
  }

  #[test]
  fn test_write_output_fails_on_existing_file() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let dest = tmp.path().join("out.txt");
    std::fs::write(&dest, "existing")?;
    let (context, _) = ctx_with_output("data", "new content");
    let cmd = WriteOutput {
      write_output: "data".into(),
      to_file: dest.to_string_lossy().into_owned(),
      create_parents: None,
    };
    assert!(cmd.execute(&context).is_err());
    Ok(())
  }

  #[test]
  fn test_write_output_with_create_parents() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let dest = tmp.path().join("nested").join("dir").join("out.txt");
    let (context, _) = ctx_with_output("data", "content");
    let cmd = WriteOutput {
      write_output: "data".into(),
      to_file: dest.to_string_lossy().into_owned(),
      create_parents: Some(true),
    };
    cmd.execute(&context)?;
    assert_eq!(std::fs::read_to_string(&dest)?, "content");
    Ok(())
  }

  #[test]
  fn test_write_output_fails_without_create_parents() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let dest = tmp.path().join("nonexistent").join("out.txt");
    let (context, _) = ctx_with_output("data", "content");
    let cmd = WriteOutput {
      write_output: "data".into(),
      to_file: dest.to_string_lossy().into_owned(),
      create_parents: None,
    };
    assert!(cmd.execute(&context).is_err());
    Ok(())
  }

  #[test]
  fn test_write_output_missing_source_returns_error() {
    let tmp = TempDir::new().unwrap();
    let dest = tmp.path().join("out.txt");
    let context = TaskContext::empty_with_root(Arc::new(TaskRoot::default()));
    let cmd = WriteOutput {
      write_output: "missing".into(),
      to_file: dest.to_string_lossy().into_owned(),
      create_parents: None,
    };
    assert!(cmd.execute(&context).is_err());
  }
}
