#![no_main]

use libfuzzer_sys::fuzz_target;
use mk_lib::secrets::VaultMeta;

const MAX_INPUT_LEN: usize = 8 * 1024;

fuzz_target!(|data: &[u8]| {
  if data.len() > MAX_INPUT_LEN {
    return;
  }

  let Ok(input) = std::str::from_utf8(data) else {
    return;
  };

  // Exercise TOML deserialization of the vault metadata format.
  if let Ok(meta) = toml::from_str::<VaultMeta>(input) {
    let _ = (meta.backend, meta.gpg_key_id, meta.key_name, meta.keys_location);
  }

  // Also exercise JSON deserialization of the same struct.
  if let Ok(meta) = serde_json::from_str::<VaultMeta>(input) {
    let _ = (meta.backend, meta.gpg_key_id, meta.key_name, meta.keys_location);
  }
});
