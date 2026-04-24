#!/usr/bin/env bash

set -euo pipefail

declare -Ag package_scope=()
declare -Ag crate_package_cache=()
declare -ag scope_args=()
needs_rust_checks=0
needs_workspace_checks=0

reset_check_scope() {
    package_scope=()
    scope_args=()
    needs_rust_checks=0
    needs_workspace_checks=0
}

package_name_for_crate_dir() {
    local crate_dir="$1"

    if [[ -n "${crate_package_cache[$crate_dir]:-}" ]]; then
        printf '%s\n' "${crate_package_cache[$crate_dir]}"
        return
    fi

    local manifest="crates/$crate_dir/Cargo.toml"
    if [[ ! -f "$manifest" ]]; then
        return
    fi

    local package_name
    package_name="$(sed -n 's/^name = "\(.*\)"/\1/p' "$manifest" | head -n 1)"
    if [[ -n "$package_name" ]]; then
        crate_package_cache["$crate_dir"]="$package_name"
        printf '%s\n' "$package_name"
    fi
}

collect_check_scope() {
    local file
    while IFS= read -r file; do
        [[ -z "$file" ]] && continue

        case "$file" in
            *.rs|Cargo.toml|Cargo.lock|build.rs|rustfmt.toml|clippy.toml|.cargo/*|src/*|tests/*|examples/*|fuzz/*|crates/*/Cargo.toml|crates/*/build.rs|crates/*/src/*|crates/*/tests/*|crates/*/examples/*|crates/*/benches/*|crates/*/fuzz/*)
                needs_rust_checks=1
                ;;
            *)
                ;;
        esac

        case "$file" in
            Cargo.toml|Cargo.lock|build.rs|rustfmt.toml|clippy.toml|.cargo/*)
                needs_workspace_checks=1
                ;;
            src/*|tests/*|examples/*|fuzz/*)
                package_scope["mk"]=1
                ;;
            crates/*)
                local crate_dir crate_package
                crate_dir="${file#crates/}"
                crate_dir="${crate_dir%%/*}"
                crate_package="$(package_name_for_crate_dir "$crate_dir")"
                if [[ -n "$crate_package" ]]; then
                    package_scope["$crate_package"]=1
                else
                    needs_workspace_checks=1
                fi
                ;;
            *.rs)
                needs_workspace_checks=1
                ;;
            *)
                ;;
        esac
    done
}

build_scope_args() {
    local force_full="${1:-0}"

    scope_args=()
    if [[ "$needs_workspace_checks" -eq 1 || "$force_full" == "1" ]]; then
        scope_args=(--workspace)
        return
    fi

    local packages=()
    local package
    for package in "${!package_scope[@]}"; do
        packages+=("$package")
    done

    if [[ "${#packages[@]}" -eq 0 ]]; then
        return
    fi

    IFS=$'\n' packages=($(printf '%s\n' "${packages[@]}" | sort))
    unset IFS

    for package in "${packages[@]}"; do
        scope_args+=(-p "$package")
    done
}
