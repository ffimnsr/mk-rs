#!/usr/bin/env bash

set -euo pipefail

readonly PACKAGE_NAME="mk"
readonly REMOTE_NAME="origin"

usage() {
  cat <<'EOF'
Usage: scripts/release.sh [options] [<version>]

Bump the package version, verify the crate, publish it to crates.io,
create a git tag, and push the release commit and tag to origin.

Options:
  --major         Increment the major version and reset minor/patch to zero.
  --minor         Increment the minor version and reset patch to zero.
  --patch         Increment the patch version.
  --skip-publish  Skip `cargo publish` and only prepare/tag/push the release.
  --skip-push     Skip pushing the release commit and tag to origin.
  -h, --help      Show this help message.

Examples:
  scripts/release.sh 0.4.3
  scripts/release.sh --patch
  scripts/release.sh --minor --skip-push
  scripts/release.sh --skip-publish --skip-push 0.4.3
EOF
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

ensure_clean_worktree() {
  git diff --quiet --exit-code || die "working tree has unstaged changes"
  git diff --cached --quiet --exit-code || die "index has staged but uncommitted changes"
}

current_version() {
  awk '
    BEGIN { in_package = 0 }
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ && $0 != "[package]" && in_package { in_package = 0 }
    in_package && /^version = "/ {
      gsub(/^version = "/, "", $0)
      gsub(/"$/, "", $0)
      print
      exit
    }
  ' Cargo.toml
}

increment_version() {
  local current="$1"
  local bump_kind="$2"
  local major minor patch

  IFS='.' read -r major minor patch <<<"$current"

  case "$bump_kind" in
    major)
      ((major += 1))
      minor=0
      patch=0
      ;;
    minor)
      ((minor += 1))
      patch=0
      ;;
    patch)
      ((patch += 1))
      ;;
    *)
      die "unsupported bump kind: $bump_kind"
      ;;
  esac

  printf '%s.%s.%s\n' "$major" "$minor" "$patch"
}

update_manifest_version() {
  local version="$1"
  local tmp
  tmp="$(mktemp)"

  awk -v version="$version" '
    BEGIN { in_package = 0; replaced = 0 }
    /^\[package\]$/ { in_package = 1 }
    /^\[/ && $0 != "[package]" && in_package { in_package = 0 }
    in_package && /^version = "/ && !replaced {
      print "version = \"" version "\""
      replaced = 1
      next
    }
    { print }
    END {
      if (!replaced) {
        exit 1
      }
    }
  ' Cargo.toml >"$tmp" || {
    rm -f "$tmp"
    die "failed to update Cargo.toml version"
  }

  mv "$tmp" Cargo.toml
}

update_lockfile_version() {
  local version="$1"
  local tmp
  tmp="$(mktemp)"

  awk -v version="$version" -v package_name="$PACKAGE_NAME" '
    BEGIN { in_package = 0; target = 0; replaced = 0 }
    /^\[\[package\]\]$/ {
      in_package = 1
      target = 0
    }
    in_package && $0 == "name = \"" package_name "\"" {
      target = 1
    }
    target && /^version = "/ && !replaced {
      print "version = \"" version "\""
      replaced = 1
      target = 0
      next
    }
    { print }
    END {
      if (!replaced) {
        exit 1
      }
    }
  ' Cargo.lock >"$tmp" || {
    rm -f "$tmp"
    die "failed to update Cargo.lock version"
  }

  mv "$tmp" Cargo.lock
}

main() {
  local run_publish=1
  local run_push=1
  local version=""
  local bump_kind=""

  while (($# > 0)); do
    case "$1" in
      --major|--minor|--patch)
        [[ -z "$bump_kind" ]] || die "only one of --major, --minor, or --patch may be used"
        bump_kind="${1#--}"
        shift
        ;;
      --skip-publish)
        run_publish=0
        shift
        ;;
      --skip-push)
        run_push=0
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      -*)
        die "unknown option: $1"
        ;;
      *)
        if [[ -n "$version" ]]; then
          die "version may only be provided once"
        fi
        version="$1"
        shift
        ;;
    esac
  done

  if [[ -z "$version" && -z "$bump_kind" ]]; then
    usage
    exit 1
  fi

  [[ -z "$version" || -z "$bump_kind" ]] || die "pass either an explicit version or one bump flag"

  need_cmd awk
  need_cmd cargo
  need_cmd git
  need_cmd mktemp

  local repo_root
  repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" || die "must be run inside a git repository"
  cd "$repo_root"

  ensure_clean_worktree

  git remote get-url "$REMOTE_NAME" >/dev/null 2>&1 || die "git remote '$REMOTE_NAME' is not configured"

  local old_version
  old_version="$(current_version)"
  [[ -n "$old_version" ]] || die "failed to read current package version"
  [[ "$old_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "current version must match x.y.z"

  if [[ -n "$bump_kind" ]]; then
    version="$(increment_version "$old_version" "$bump_kind")"
  fi

  [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "version must match x.y.z"
  [[ "$old_version" != "$version" ]] || die "version is already $version"

  git rev-parse --verify "refs/tags/$version" >/dev/null 2>&1 && die "tag '$version' already exists locally"
  git ls-remote --exit-code --tags "$REMOTE_NAME" "refs/tags/$version" >/dev/null 2>&1 && die "tag '$version' already exists on '$REMOTE_NAME'"

  update_manifest_version "$version"
  update_lockfile_version "$version"

  cargo fmt
  cargo test
  cargo clippy --all-targets --all-features -- -D warnings

  git add Cargo.toml Cargo.lock
  git commit -m "release: $version"

  cargo publish --dry-run

  if ((run_publish)); then
    cargo publish
  fi

  git tag -a "$version" -m "release: $version"

  if ((run_push)); then
    git push "$REMOTE_NAME" HEAD
    git push "$REMOTE_NAME" "$version"
  fi

  printf 'Released %s -> %s\n' "$old_version" "$version"
}

main "$@"
