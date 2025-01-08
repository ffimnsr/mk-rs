use regex::Regex;

#[allow(dead_code)]
pub fn is_path_safe(path: &str) -> bool {
  let re = Regex::new(r"^[a-zA-Z0-9_\-\/]+$").unwrap();
  re.is_match(path)
}
