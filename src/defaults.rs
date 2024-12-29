/// Default value for `shell` field
///
/// ```
/// # use mk_lib::defaults::default_shell;
/// let a = default_shell();
/// ```
pub fn default_shell() -> String {
  "sh".to_string()
}

/// Default value for `verbose` field
///
/// ```
/// # use mk_lib::defaults::default_true;
/// let a = default_true();
/// ```
pub fn default_true() -> bool {
  true
}
