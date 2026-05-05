#![no_main]

use libfuzzer_sys::fuzz_target;
use mk_lib::schema::{
  interpolate_template_string, interpolate_matrix_template_string, resolve_template_expression,
  is_shell_command, is_template_command, TaskContext,
};

use std::collections::BTreeMap;

const MAX_INPUT_LEN: usize = 8 * 1024;
const MAX_LINES: usize = 32;

fuzz_target!(|data: &[u8]| {
  if data.len() > MAX_INPUT_LEN {
    return;
  }

  let Ok(input) = std::str::from_utf8(data) else {
    return;
  };

  // Split input: first line is the template string, remaining lines populate
  // env_vars and matrix_vars as KEY=VALUE pairs.
  let mut lines = input.lines();
  let Some(template) = lines.next() else {
    return;
  };

  let mut context = TaskContext::empty();
  let mut matrix: BTreeMap<String, String> = BTreeMap::new();

  for line in lines.take(MAX_LINES) {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    if let Some((key, value)) = line.split_once('=') {
      let key = key.trim().to_string();
      let value = value.trim().to_string();
      if key.starts_with("matrix.") {
        let mkey = key.trim_start_matches("matrix.").to_string();
        context.matrix_vars.insert(mkey.clone(), value.clone());
        matrix.insert(mkey, value);
      } else {
        context.env_vars.insert(key, value);
      }
    }
  }

  // Exercise all template-related functions.
  let _ = is_shell_command(template);
  let _ = is_template_command(template);
  let _ = interpolate_template_string(template, &context);
  let _ = resolve_template_expression(template, &context);
  let _ = interpolate_matrix_template_string(template, &matrix);
});
