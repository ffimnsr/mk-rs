use schemars::JsonSchema;
use serde::Deserialize;

use crate::schema::{interpolate_template_string, TaskContext};

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub struct JsonExtract {
  /// The name of a previously saved output to parse as JSON
  pub extract_json_from: String,

  /// Dot-separated path into the JSON value (e.g. "items.0.name")
  pub json_path: String,

  /// The output name under which the extracted value is saved
  pub save_as: String,
}

impl JsonExtract {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    let source_name = interpolate_template_string(&self.extract_json_from, context)?;
    let path = interpolate_template_string(&self.json_path, context)?;
    let save_as = interpolate_template_string(&self.save_as, context)?;

    let raw = context
      .get_task_output(&source_name)?
      .ok_or_else(|| anyhow::anyhow!("Task output '{}' is not available", source_name))?;

    let json: serde_json::Value = serde_json::from_str(&raw)
      .map_err(|e| anyhow::anyhow!("Failed to parse '{}' as JSON: {}", source_name, e))?;

    let extracted = resolve_dot_path(&json, &path)
      .ok_or_else(|| anyhow::anyhow!("JSON path '{}' not found in output '{}'", path, source_name))?;

    let value = json_value_to_string(extracted)?;
    context.insert_task_output(save_as, value)?;
    Ok(())
  }
}

/// Walk a dot-separated path through a JSON value.
/// Array indices are supported as numeric segments (e.g. "items.0.name").
fn resolve_dot_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
  let mut current = value;
  for segment in path.split('.') {
    current = match current {
      serde_json::Value::Object(map) => map.get(segment)?,
      serde_json::Value::Array(arr) => {
        let index: usize = segment.parse().ok()?;
        arr.get(index)?
      },
      _ => return None,
    };
  }
  Some(current)
}

/// Convert a leaf JSON value to a plain string.
/// Objects and arrays are serialised back to compact JSON strings.
fn json_value_to_string(value: &serde_json::Value) -> anyhow::Result<String> {
  Ok(match value {
    serde_json::Value::String(s) => s.clone(),
    serde_json::Value::Number(n) => n.to_string(),
    serde_json::Value::Bool(b) => b.to_string(),
    serde_json::Value::Null => "null".to_string(),
    other => serde_json::to_string(other)?,
  })
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use super::*;
  use crate::schema::{TaskContext, TaskRoot};

  fn ctx_with_output(name: &str, value: &str) -> TaskContext {
    let context = TaskContext::empty_with_root(Arc::new(TaskRoot::default()));
    context.insert_task_output(name, value).unwrap();
    context
  }

  #[test]
  fn test_extract_string_field() -> anyhow::Result<()> {
    let context = ctx_with_output("data", r#"{"name":"alice","age":30}"#);
    let cmd = JsonExtract {
      extract_json_from: "data".into(),
      json_path: "name".into(),
      save_as: "result".into(),
    };
    cmd.execute(&context)?;
    assert_eq!(context.get_task_output("result")?, Some("alice".to_string()));
    Ok(())
  }

  #[test]
  fn test_extract_nested_path() -> anyhow::Result<()> {
    let context = ctx_with_output("data", r#"{"user":{"id":42}}"#);
    let cmd = JsonExtract {
      extract_json_from: "data".into(),
      json_path: "user.id".into(),
      save_as: "uid".into(),
    };
    cmd.execute(&context)?;
    assert_eq!(context.get_task_output("uid")?, Some("42".to_string()));
    Ok(())
  }

  #[test]
  fn test_extract_array_index() -> anyhow::Result<()> {
    let context = ctx_with_output("data", r#"{"items":["a","b","c"]}"#);
    let cmd = JsonExtract {
      extract_json_from: "data".into(),
      json_path: "items.1".into(),
      save_as: "second".into(),
    };
    cmd.execute(&context)?;
    assert_eq!(context.get_task_output("second")?, Some("b".to_string()));
    Ok(())
  }

  #[test]
  fn test_invalid_json_returns_error() {
    let context = ctx_with_output("data", "not json");
    let cmd = JsonExtract {
      extract_json_from: "data".into(),
      json_path: "key".into(),
      save_as: "result".into(),
    };
    assert!(cmd.execute(&context).is_err());
  }

  #[test]
  fn test_missing_path_returns_error() {
    let context = ctx_with_output("data", r#"{"key":"value"}"#);
    let cmd = JsonExtract {
      extract_json_from: "data".into(),
      json_path: "missing".into(),
      save_as: "result".into(),
    };
    assert!(cmd.execute(&context).is_err());
  }

  #[test]
  fn test_missing_source_output_returns_error() {
    let context = TaskContext::empty_with_root(Arc::new(TaskRoot::default()));
    let cmd = JsonExtract {
      extract_json_from: "nonexistent".into(),
      json_path: "key".into(),
      save_as: "result".into(),
    };
    assert!(cmd.execute(&context).is_err());
  }
}
