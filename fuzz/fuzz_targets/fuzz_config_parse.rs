#![no_main]

use libfuzzer_sys::fuzz_target;
use mk_lib::schema::TaskRoot;

const MAX_INPUT_LEN: usize = 16 * 1024;
const MAX_PLANNED_TASKS: usize = 8;

fuzz_target!(|data: &[u8]| {
  if data.len() > MAX_INPUT_LEN {
    return;
  }

  let Ok(input) = std::str::from_utf8(data) else {
    return;
  };

  try_yaml(input);
  try_json(input);
  try_toml(input);
});

fn try_yaml(input: &str) {
  if let Ok(root) = serde_yaml::from_str::<TaskRoot>(input) {
    exercise_root(root);
  }
}

fn try_json(input: &str) {
  if let Ok(root) = serde_json::from_str::<TaskRoot>(input) {
    exercise_root(root);
  }
}

fn try_toml(input: &str) {
  if let Ok(root) = toml::from_str::<TaskRoot>(input) {
    exercise_root(root);
  }
}

fn exercise_root(root: TaskRoot) {
  let report = root.validate();
  let _ = serde_json::to_string(&report);

  for task_name in root.tasks.keys().take(MAX_PLANNED_TASKS) {
    let _ = root.plan_task(task_name);
  }
}
