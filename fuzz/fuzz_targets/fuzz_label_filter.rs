#![no_main]

use hashbrown::HashMap;
use libfuzzer_sys::fuzz_target;
use mk_lib::label_filter::{
  matches_all,
  LabelFilter,
};

const MAX_INPUT_LEN: usize = 8 * 1024;
const MAX_FILTERS: usize = 16;
const MAX_LABELS: usize = 64;

fuzz_target!(|data: &[u8]| {
  if data.len() > MAX_INPUT_LEN {
    return;
  }

  let Ok(input) = std::str::from_utf8(data) else {
    return;
  };

  let mut filters = Vec::new();
  let mut labels = HashMap::new();

  for (index, line) in input.lines().enumerate() {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }

    if index < MAX_FILTERS {
      filters.push(LabelFilter::parse(line));
      continue;
    }

    if labels.len() >= MAX_LABELS {
      break;
    }

    if let Some((key, value)) = line.split_once('=') {
      labels.insert(key.to_string(), value.to_string());
    } else {
      labels.insert(line.to_string(), String::new());
    }
  }

  let _ = matches_all(&filters, &labels);
});
