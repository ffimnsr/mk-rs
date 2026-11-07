use serde::Serialize;
use serde_json::Value;

/// Print a serializable value as canonical, pretty-printed JSON.
pub(crate) fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
  print_json_value(serde_json::to_value(value)?)
}

/// Print a JSON value as canonical, pretty-printed JSON.
pub(crate) fn print_json_value(value: Value) -> anyhow::Result<()> {
  let value = canonicalize_json(value);
  println!("{}", serde_json::to_string_pretty(&value)?);
  Ok(())
}

/// Sort object keys recursively so JSON output stays deterministic.
fn canonicalize_json(value: Value) -> Value {
  match value {
    Value::Array(items) => Value::Array(items.into_iter().map(canonicalize_json).collect()),
    Value::Object(map) => {
      let mut entries = map.into_iter().collect::<Vec<_>>();
      entries.sort_by(|left, right| left.0.cmp(&right.0));

      let mut canonical = serde_json::Map::new();
      for (key, value) in entries {
        canonical.insert(key, canonicalize_json(value));
      }
      Value::Object(canonical)
    },
    other => other,
  }
}
