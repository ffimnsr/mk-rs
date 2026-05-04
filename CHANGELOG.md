# Changelog

## 0.7.17 - 2026-05-04

### Features

- implement GNU Make config support milestone 1 and 2 parity (`9a8e0e3`)

### Fixes

- resolve macOS CI test failures and update markdown task naming spec (`171713e`)
- normalize macOS path in makefile integration test (`95eb189`)

### Documentation

- add on roadmap to support multiple interpreter (`f0cd838`)
- update wording so shell is now interpreter so can run depending on interpreter (`1964797`)

## 0.7.16 - 2026-05-04

### Features

- update ISSUES.md for make support and workflow updates (`8ffa351`)

## 0.7.15 - 2026-05-04

### Features

- add nerdctl runtime support (`1bfdaef`)
- add fuzzy task selector (`9f39d53`)
- expand init templates and cache coverage (`27f437e`)
- update tasks and coverage report (`e471407`)
- add ssh_run, json_extract, write_output commands and mk doctor (`885ce8a`)
- watch mode with trailing args and man page generation (`3fd346e`)
- add release workflow for macos aarch64 (`3ee2354`)
- update workflow to have cache (`a417a76`)
- add zeroize for secrets to be remove from mem (`fd95670`)

### Fixes

- align fuzzy selector descriptions (`4140274`)
- guard test_run_fzf_reports_missing_backend with cfg(unix) (`702e34a`)

### Documentation

- add wiki submodule docs (`4ef5c41`)

### Maintenance

- update the formatter issue nightly vs stable (`1bd2fa2`)

### Other Changes

- 0.7.14 (`f3b567e`)

## 0.7.14 - 2026-05-04

### Features

- add nerdctl runtime support (`1bfdaef`)
- add fuzzy task selector (`9f39d53`)
- expand init templates and cache coverage (`27f437e`)
- update tasks and coverage report (`e471407`)
- add ssh_run, json_extract, write_output commands and mk doctor (`885ce8a`)
- watch mode with trailing args and man page generation (`3fd346e`)
- add release workflow for macos aarch64 (`3ee2354`)
- update workflow to have cache (`a417a76`)
- add zeroize for secrets to be remove from mem (`fd95670`)

### Fixes

- align fuzzy selector descriptions (`4140274`)
- guard test_run_fzf_reports_missing_backend with cfg(unix) (`702e34a`)

### Documentation

- add wiki submodule docs (`4ef5c41`)

### Maintenance

- update the formatter issue nightly vs stable (`1bd2fa2`)

## 0.7.13 - 2026-04-26

### Features

- add fuzz targets and refresh deps (`2cc8815`)
- update containerfile for failed run (`4a76e72`)
- improve task completion and release maintenance (`5e8033a`)

### Tests

- update snapshot (`2744a3a`)

See github release page: https://github.com/ffimnsr/mk-rs/releases
