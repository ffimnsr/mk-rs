/// A single label filter: either key-existence (`KEY`) or exact-value (`KEY=VALUE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelFilter {
  /// Match tasks that have the key, regardless of value.
  Exists(String),
  /// Match tasks whose label key equals the given value exactly.
  Equals(String, String),
}

impl LabelFilter {
  /// Parse a filter string. `"KEY"` becomes `Exists`, `"KEY=VALUE"` becomes `Equals`.
  pub fn parse(s: &str) -> Self {
    if let Some((key, value)) = s.split_once('=') {
      Self::Equals(key.to_owned(), value.to_owned())
    } else {
      Self::Exists(s.to_owned())
    }
  }

  /// Returns true when this filter matches `labels`.
  pub fn matches(&self, labels: &hashbrown::HashMap<String, String>) -> bool {
    match self {
      Self::Exists(key) => labels.contains_key(key),
      Self::Equals(key, value) => labels.get(key).map(|v| v == value).unwrap_or(false),
    }
  }
}

/// Returns true when all `filters` match `labels` (AND semantics).
/// An empty filter list always returns true.
pub fn matches_all(filters: &[LabelFilter], labels: &hashbrown::HashMap<String, String>) -> bool {
  filters.iter().all(|f| f.matches(labels))
}

#[cfg(test)]
mod tests {
  use super::*;
  use hashbrown::HashMap;

  fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect()
  }

  #[test]
  fn parse_key_only() {
    assert_eq!(LabelFilter::parse("area"), LabelFilter::Exists("area".into()));
  }

  #[test]
  fn parse_key_value() {
    assert_eq!(
      LabelFilter::parse("area=ci"),
      LabelFilter::Equals("area".into(), "ci".into())
    );
  }

  #[test]
  fn parse_key_value_with_equals_in_value() {
    // only split on the first '='
    assert_eq!(
      LabelFilter::parse("key=a=b"),
      LabelFilter::Equals("key".into(), "a=b".into())
    );
  }

  #[test]
  fn exists_filter_matches_present_key() {
    let labels = map(&[("area", "ci")]);
    assert!(LabelFilter::Exists("area".into()).matches(&labels));
  }

  #[test]
  fn exists_filter_rejects_missing_key() {
    let labels = map(&[("other", "x")]);
    assert!(!LabelFilter::Exists("area".into()).matches(&labels));
  }

  #[test]
  fn equals_filter_matches_exact_value() {
    let labels = map(&[("area", "ci")]);
    assert!(LabelFilter::Equals("area".into(), "ci".into()).matches(&labels));
  }

  #[test]
  fn equals_filter_rejects_wrong_value() {
    let labels = map(&[("area", "build")]);
    assert!(!LabelFilter::Equals("area".into(), "ci".into()).matches(&labels));
  }

  #[test]
  fn equals_filter_rejects_missing_key() {
    let labels = map(&[]);
    assert!(!LabelFilter::Equals("area".into(), "ci".into()).matches(&labels));
  }

  #[test]
  fn matches_all_empty_filters() {
    let labels = map(&[]);
    assert!(matches_all(&[], &labels));
  }

  #[test]
  fn matches_all_multiple_filters_all_match() {
    let labels = map(&[("area", "ci"), ("kind", "test")]);
    let filters = vec![
      LabelFilter::Equals("area".into(), "ci".into()),
      LabelFilter::Exists("kind".into()),
    ];
    assert!(matches_all(&filters, &labels));
  }

  #[test]
  fn matches_all_multiple_filters_one_fails() {
    let labels = map(&[("area", "ci")]);
    let filters = vec![
      LabelFilter::Equals("area".into(), "ci".into()),
      LabelFilter::Exists("kind".into()),
    ];
    assert!(!matches_all(&filters, &labels));
  }

  #[test]
  fn matches_all_no_match() {
    let labels = map(&[("area", "build")]);
    let filters = vec![LabelFilter::Equals("area".into(), "ci".into())];
    assert!(!matches_all(&filters, &labels));
  }
}
