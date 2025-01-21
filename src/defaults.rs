/// Default value for `shell` field
///
/// ```
/// # use mk_lib::defaults::default_shell;
/// let a = default_shell();
/// assert_eq!(a, "sh");
/// ```
pub fn default_shell() -> String {
  "sh".to_string()
}

/// Default value for `verbose` field
///
/// ```
/// # use mk_lib::defaults::default_verbose;
/// let a = default_verbose();
/// assert!(a);
/// ```
pub fn default_verbose() -> bool {
  true
}

/// Default value for `verbose` field
///
/// ```
/// # use mk_lib::defaults::default_ignore_errors;
/// let a = default_ignore_errors();
/// assert!(!a);
/// ```
pub fn default_ignore_errors() -> bool {
  false
}

/// Default value for `use_npm` -> `package_manager` field
///
/// ```
/// # use mk_lib::defaults::default_node_package_manager;
/// let a = default_node_package_manager();
/// assert_eq!(a, "npm");
/// ```
pub fn default_node_package_manager() -> String {
  "npm".to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_default_ignore_errors() {
    let result = default_ignore_errors();
    assert!(!result);
  }

  #[test]
  fn test_default_node_package_manager() {
    let result = default_node_package_manager();
    assert_eq!(result, "npm");
    assert_eq!(result.len(), 3);
    assert!(result.is_ascii());
  }

  #[test]
  fn test_default_shell() {
    let result = default_shell();
    assert_eq!(result, "sh");
    assert_eq!(result.len(), 2);
    assert!(result.is_ascii());
  }

  #[test]
  fn test_default_verbose() {
    let result = default_verbose();
    assert!(result);
  }
}
