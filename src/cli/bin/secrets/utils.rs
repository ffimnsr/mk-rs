use regex::Regex;

#[allow(dead_code)]
pub fn is_path_safe(path: &str) -> anyhow::Result<bool> {
  let re = Regex::new(r"^[a-zA-Z0-9_\-\/]+$")?;
  Ok(re.is_match(path))
}
