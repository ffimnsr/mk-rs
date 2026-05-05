#![no_main]

use libfuzzer_sys::fuzz_target;
use mk_lib::make::parse_make_database_rule_records;

const MAX_INPUT_LEN: usize = 64 * 1024;

fuzz_target!(|data: &[u8]| {
  if data.len() > MAX_INPUT_LEN {
    return;
  }

  let Ok(input) = std::str::from_utf8(data) else {
    return;
  };

  let _ = parse_make_database_rule_records(input);
});
