#![no_main]

use libfuzzer_sys::fuzz_target;
use mk_lib::cache::CacheStore;

const MAX_INPUT_LEN: usize = 32 * 1024;

fuzz_target!(|data: &[u8]| {
  if data.len() > MAX_INPUT_LEN {
    return;
  }

  let Ok(input) = std::str::from_utf8(data) else {
    return;
  };

  // Exercise JSON deserialization of the on-disk cache format.
  if let Ok(store) = serde_json::from_str::<CacheStore>(input) {
    // Touch each entry to ensure access paths don't panic.
    for (name, entry) in &store.tasks {
      let _ = (name.len(), entry.fingerprint.len(), entry.outputs.len(), entry.updated_at.len());
    }
  }
});
