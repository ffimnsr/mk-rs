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
/// # use mk_lib::defaults::default_true;
/// let a = default_true();
/// assert!(a);
/// ```
pub fn default_true() -> bool {
  true
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
