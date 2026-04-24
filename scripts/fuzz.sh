#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

target="${1:-fuzz_config_parse}"
time_limit="${FUZZ_TIME:-60}"

if [[ "$target" == "--help" || "$target" == "-h" ]]; then
  cat <<'USAGE'
Usage:
  scripts/fuzz.sh [target|all|--list] [libFuzzer args...]

Examples:
  scripts/fuzz.sh --list
  scripts/fuzz.sh fuzz_config_parse
  FUZZ_TIME=300 scripts/fuzz.sh all
  scripts/fuzz.sh fuzz_label_filter -runs=10000
USAGE
  exit 0
fi

if ! command -v cargo-fuzz >/dev/null 2>&1; then
  cargo install cargo-fuzz
fi

if [[ "$target" == "--list" ]]; then
  cargo fuzz list
  exit 0
fi

shift || true

if [[ "$target" == "all" ]]; then
  while IFS= read -r fuzz_target; do
    cargo fuzz run "$fuzz_target" -- -max_total_time="$time_limit" "$@"
  done < <(cargo fuzz list)
else
  cargo fuzz run "$target" -- -max_total_time="$time_limit" "$@"
fi
