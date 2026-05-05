#![no_main]

use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use mk_lib::schema::MatrixSelector;

const MAX_INPUT_LEN: usize = 4 * 1024;
const MAX_SELECTORS: usize = 64;

fuzz_target!(|data: &[u8]| {
  if data.len() > MAX_INPUT_LEN {
    return;
  }

  let Ok(input) = std::str::from_utf8(data) else {
    return;
  };

  for line in input.lines().take(MAX_SELECTORS) {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    let _ = MatrixSelector::from_str(line);
  }
});
