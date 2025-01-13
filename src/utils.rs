use std::fmt;

use hashbrown::HashMap;
use serde::de::{
  self,
  MapAccess,
  Visitor,
};
use serde::{
  Deserialize,
  Deserializer,
};
use serde_json::Value as JsonValue;

#[allow(dead_code)]
#[derive(Debug)]
enum AnyValue {
  String(String),
  Number(serde_json::Number),
  Bool(bool),
}

impl fmt::Display for AnyValue {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      AnyValue::String(s) => write!(f, "{}", s),
      AnyValue::Number(n) => write!(f, "{}", n),
      AnyValue::Bool(b) => write!(f, "{}", b),
    }
  }
}

impl<'de> Deserialize<'de> for AnyValue {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value: JsonValue = Deserialize::deserialize(deserializer)?;
    match value {
      JsonValue::String(s) => Ok(AnyValue::String(s)),
      JsonValue::Number(n) => Ok(AnyValue::Number(n)),
      JsonValue::Bool(b) => Ok(AnyValue::Bool(b)),
      _ => Err(de::Error::custom("expected a string, number, or boolean")),
    }
  }
}

pub(crate) fn deserialize_environment<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
  D: Deserializer<'de>,
{
  struct EnvironmentVisitor;

  impl<'de> Visitor<'de> for EnvironmentVisitor {
    type Value = HashMap<String, String>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      formatter.write_str("a map of strings to any value (string, int, or bool)")
    }

    fn visit_map<M>(self, mut access: M) -> Result<HashMap<String, String>, M::Error>
    where
      M: MapAccess<'de>,
    {
      let mut map = HashMap::new();
      while let Some((key, value)) = access.next_entry::<String, AnyValue>()? {
        map.insert(key, value.to_string());
      }
      Ok(map)
    }
  }

  deserializer.deserialize_map(EnvironmentVisitor)
}
