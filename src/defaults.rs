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
