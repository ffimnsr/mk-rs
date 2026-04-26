# Issues

Instruction for all the items in this file:
- Keep each checklist item scoped to one small workable chunk.
- Describe exact code, command, schema field, validation rule, or test to add/change.
- Do not combine multiple implementation steps into one checklist item if they can be merged separately.
- Prefer additive wording like "add", "replace", "update", "remove", "validate", "test".
- Avoid broad goals without concrete implementation detail.

## Task Labels

Instruction for items in this section:
- Keep labels as task metadata until a command explicitly uses them.
- Use exact-match filtering first; avoid regex or expression syntax until needed.
- Preserve deterministic task ordering when labels select multiple tasks.

### Phase 1: Shared Label Matching

- [x] Add shared label filter parsing and matching
  - Add a reusable parser for `KEY` and `KEY=VALUE` label filters.
  - Treat multiple label filters as AND filters.
  - Match `KEY` by label existence.
  - Match `KEY=VALUE` by exact label value.
  - Add unit tests for existence, exact value, multiple filters, and no match.

### Phase 2: List Integration

- [x] Add label filters to `mk list`
  - Add repeatable `--label <KEY>` and `--label <KEY=VALUE>` flags to `mk list`.
  - Use the shared label matching helper.
  - Keep sorted task output order after filtering.
  - Add integration tests for text, plain, and JSON list output.

- [x] Include labels in `mk list --json`
  - Add a `labels` object to each task entry.
  - Use `{}` for string shorthand tasks and tasks without labels.
  - Keep JSON output sorted and stable.
  - Update `tests/snapshots/list-json.snap`.

### Phase 3: Run Integration

- [x] Add label filters to `mk run`
  - Add repeatable `--label <KEY>` and `--label <KEY=VALUE>` flags to `mk run`.
  - Use the shared label matching helper.
  - Require either a task name or at least one `--label` filter.
  - Run all matching tasks in deterministic sorted order.
  - Return an error when no task matches the label filter.
  - Add integration tests for one match, multiple matches, and no matches.

### Phase 4: Plan Integration

- [x] Add label filters to `mk plan`
  - Add repeatable `--label <KEY>` and `--label <KEY=VALUE>` flags to `mk plan`.
  - Use the shared label matching helper.
  - Print combined plans for all matching tasks in deterministic sorted order.
  - Preserve current single-task `mk plan <task>` behavior.
  - Add integration tests for text and JSON plan output.

### Phase 5: Validation

- [x] Add label validation rules
  - Warn on empty label keys.
  - Warn on empty label values.
  - Warn on labels using reserved `mk.` prefix.
  - Add validation tests for each warning.

### Phase 6: Docs and Examples

- [x] Document task label workflows
  - Add README examples for `mk list --label area=ci`.
  - Add README examples for `mk run --label kind=test`.
  - Document multiple label filters as AND.
  - Clarify task labels are separate from `container_build.labels`.

## Task Selector

- [x] Add fuzzy task selector to `mk run`
  - Add `mk run --fzf` and `mk run -F`.
  - Select one task with `fzf` first, then `sk`.
  - Keep command `interactive: true` behavior unchanged.
  - Allow `--label` filters to narrow selector candidates.
  - Add integration tests for selection, fallback, missing backend, and cancel flow.

## Watch Mode

Instruction for items in this section:
- Keep watch behavior deterministic and explicit.
- Reuse existing task resolution, validation, and cache semantics where possible.
- Prefer additive CLI flags over implicit background behavior.

### Phase 1: CLI Surface

- [ ] Add `mk watch <task>` command
  - Add `watch` subcommand to CLI help and docs.
  - Require a task name or label filter input using same selection rules as `mk run`.
  - Reuse current config file discovery and task resolution.
  - Add help snapshot coverage for `mk watch --help`.

- [ ] Add watch path and debounce flags
  - Add repeatable `--path <PATH>` flags to override watched inputs.
  - Add `--debounce <DURATION>` flag for filesystem event coalescing.
  - Validate duration parsing and reject zero debounce values.
  - Add CLI parsing tests for repeated paths and invalid durations.

### Phase 2: Execution Behavior

- [ ] Add filesystem watch loop for local task reruns
  - Watch explicit `--path` values when provided.
  - Re-run selected task after debounce when matching changes arrive.
  - Print a clear rerun reason before each execution.
  - Add integration test covering one file change and one rerun.

- [ ] Reuse task `inputs` as default watch paths
  - Use declared task `inputs` when `mk watch` runs without `--path`.
  - Skip unresolved glob patterns without panicking.
  - Return a user-facing error when neither `--path` nor task `inputs` exist.
  - Add integration tests for inferred paths and empty watch target errors.

### Phase 3: Quality of Life

- [ ] Add `--clear` flag to `mk watch`
  - Clear terminal before each rerun when enabled.
  - Keep default output append-only when flag is absent.
  - Add integration test for flag parsing and screen-clear branch selection.

- [ ] Add `.mkignore` support to `mk watch` (ignore crate of ripgrep)
  - Load ignore rules from `.mkignore` at config root using gitignore-style pattern semantics.
  - Apply ignore filtering to watched descendant paths for both explicit `--path` roots and inferred task `inputs`.
  - Keep ignore handling scoped to watch behavior and do not change cache `inputs` resolution semantics.
  - Allow negated patterns so users can re-include specific paths under ignored directories.
  - Add integration test coverage for ignored paths and negated re-includes.

- [ ] Document watch workflows
  - Add README examples for `mk watch test`.
  - Document default `inputs` reuse and explicit `--path` override behavior.
  - Document `.mkignore` usage, config-root lookup, and negated pattern behavior.
  - Clarify interaction with incremental cache hits during reruns.

## Matrix Tasks

Instruction for items in this section:
- Keep matrix expansion explicit in schema and plan output.
- Preserve deterministic expansion order.
- Avoid mixing matrix support with hidden shell interpolation rules.

### Phase 1: Schema

- [ ] Add task-level `matrix` schema field
  - Add `matrix` map field to task schema with string list values.
  - Reject empty matrix keys and empty value lists during validation.
  - Keep field optional for non-matrix tasks.
  - Add schema and validation tests for valid and invalid matrix definitions.

- [ ] Add `${{ matrix.KEY }}` interpolation
  - Resolve matrix values in task command strings, env values, and task names shown in plans.
  - Return validation errors for unknown matrix keys referenced in matrix-aware fields.
  - Keep existing `${{ outputs.NAME }}` interpolation behavior unchanged.
  - Add unit tests for interpolation success and unknown-key failures.

### Phase 2: Expansion

- [ ] Expand matrix tasks into deterministic execution variants
  - Generate one task variant per cartesian product combination.
  - Sort expanded variants by matrix key name and declared value order.
  - Keep non-matrix task execution behavior unchanged.
  - Add integration test covering two-key matrix expansion order.

- [ ] Show matrix variants in `mk plan`
  - Print expanded variant names and resolved command strings in text output.
  - Include matrix values in JSON plan output.
  - Update `tests/snapshots/plan-json.snap` or add dedicated matrix plan snapshot.
  - Add integration tests for text and JSON matrix plans.

### Phase 3: Targeted Execution

- [ ] Add `mk run --set KEY=VALUE` for matrix filtering
  - Allow repeatable `--set` flags to select a subset of matrix variants.
  - Return an error when `--set` references unknown matrix keys or values.
  - Apply same filter support to `mk plan`.
  - Add integration tests for one match, multiple matches, and invalid selectors.

## Parallel Execution

Instruction for items in this section:
- Only run tasks concurrently when dependency order remains correct.
- Keep output readable and failure behavior explicit.
- Reuse existing task graph validation before scheduling work.

### Phase 1: CLI Surface

- [ ] Add `--jobs <N>` to `mk run`
  - Parse `--jobs` as a positive non-zero integer.
  - Default to current serial execution when flag is absent.
  - Show flag in CLI help and update snapshots.
  - Add CLI parsing tests for valid and invalid job counts.

### Phase 2: Scheduler

- [ ] Add dependency-aware parallel task scheduling
  - Run independent selected tasks concurrently up to `--jobs`.
  - Preserve `depends_on` ordering and wait for dependencies before starting dependents.
  - Keep deterministic scheduling among ready tasks by sorted task name.
  - Add integration test for concurrent execution of independent tasks.

- [ ] Add fail-fast behavior for parallel runs
  - Stop scheduling new tasks after first task failure.
  - Allow already-running tasks to complete before process exit.
  - Return non-zero exit status when any task fails.
  - Add integration test covering one failure and blocked unscheduled work.

### Phase 3: Optional Continue-On-Error

- [ ] Add `mk run --keep-going` for parallel and serial runs
  - Continue scheduling remaining independent tasks after a failure when enabled.
  - Keep dependency-blocked tasks skipped when an upstream dependency fails.
  - Print summary counts for succeeded, failed, and skipped tasks.
  - Add integration tests for serial and parallel `--keep-going` behavior.

## Conditional Execution

Instruction for items in this section:
- Keep condition syntax declarative.
- Validate unsupported conditions early.
- Print explicit skip reasons during plan and run output.

### Phase 1: Schema

- [ ] Add task-level `when` schema field
  - Add optional `when` object to task schema.
  - Support `os`, `env`, `file_exists`, and `command_exists` keys first.
  - Reject unknown `when` keys during validation.
  - Add schema and validation tests for accepted and rejected keys.

### Phase 2: Evaluation

- [ ] Evaluate `when.os` before task execution
  - Match current target OS against exact allowed values.
  - Skip task when OS condition does not match.
  - Show skip reason in text and JSON event output.
  - Add integration tests for matching and non-matching OS conditions.

- [ ] Evaluate `when.env` before task execution
  - Support exact-match environment checks using `KEY=VALUE`.
  - Treat missing variables as condition failures without panicking.
  - Show skip reason in text and JSON event output.
  - Add integration tests for present, missing, and mismatched env values.

- [ ] Evaluate `when.file_exists` and `when.command_exists`
  - Check local filesystem paths for `file_exists`.
  - Check command availability on `PATH` for `command_exists`.
  - Skip task when any declared condition fails.
  - Add integration tests for existing and missing files and commands.

### Phase 3: Planning and Docs

- [ ] Show conditional skip state in `mk plan`
  - Mark tasks as skipped with condition reason when conditions fail at plan time.
  - Preserve current plan output for tasks without conditions.
  - Add text and JSON plan coverage for skipped tasks.

- [ ] Document conditional task workflows
  - Add README examples for OS-gated and env-gated tasks.
  - Clarify that failed conditions skip tasks instead of failing execution.

## Doctor Command

Instruction for items in this section:
- Prefer read-only diagnostics.
- Print actionable failures with exact missing path, binary, or setting.
- Reuse existing config loading and runtime detection helpers where possible.

### Phase 1: CLI Surface

- [ ] Add `mk doctor` command
  - Add `doctor` subcommand to CLI help and docs.
  - Exit zero when all checks pass and non-zero when any required check fails.
  - Add help snapshot coverage for `mk doctor --help`.

### Phase 2: Core Diagnostics

- [ ] Add config discovery diagnostics to `mk doctor`
  - Print resolved config path or explicit missing-config result.
  - Reuse current config file search order.
  - Include detected config format in output.
  - Add integration tests for found and missing config cases.

- [ ] Add container runtime diagnostics to `mk doctor`
  - Detect available `docker`, `podman`, and `nerdctl` binaries.
  - Show which runtime `auto` would select on current system.
  - Mark runtime checks as warnings when container features are unused.
  - Add integration tests with mocked runtime binaries on `PATH`.

- [ ] Add cache and secrets diagnostics to `mk doctor`
  - Print cache metadata directory path and existence status.
  - Print secrets backend availability without exposing secret values.
  - Surface missing key material or unreadable vault path as failures.
  - Add integration tests for healthy and missing secrets state.

### Phase 3: Documentation

- [ ] Document `mk doctor` troubleshooting workflows
  - Add README examples for diagnosing missing config and runtime setup.
  - Link to relevant wiki pages for secrets and configuration help.

## Extends Composition

Instruction for items in this section:
- Use `extends` as only supported composition entrypoint.
- Keep merge order and override semantics explicit.
- Prefer improving `extends` instead of reviving deprecated `include`.

### Phase 1: Current `extends` Semantics

- [ ] Document and test deterministic `extends` merge precedence
  - Define exact parent-child merge rules for `tasks`, `environment`, `env_file`, `secrets`, `use_npm`, `use_cargo`, and `container_runtime`.
  - Document which fields append and which fields override.
  - Add integration tests covering child task override, environment override, and `env_file` append order.

- [ ] Document and test `extends` path resolution and cycle failures
  - Clarify that relative `extends` paths resolve from current config file directory.
  - Keep cycle detection error output stable and actionable.
  - Add integration tests for relative paths, absolute paths, and circular `extends`.

- [ ] Remove stale `include` references from docs and examples
  - Replace deprecated `include` examples in wiki and docs with `extends` examples.
  - Keep schema/runtime note that `include` is rejected for legacy configs.
  - Add doc test or snapshot coverage where applicable.
  - Remove legacy `include` field

### Phase 2: Expand `extends` Beyond Single-Parent Local Files

- [ ] Add multi-parent `extends` support
  - Allow `extends` to accept more than one parent config in deterministic order.
  - Define later-parent versus earlier-parent precedence before child overrides apply.
  - Add integration tests for two-parent merges and conflict resolution.

- [ ] Add validation for duplicate parent entries in `extends`
  - Reject repeated parent paths in one `extends` chain.
  - Report the duplicated path in validation or load errors.
  - Add integration tests for duplicate and unique parent lists.

### Phase 3: Override Controls

- [ ] Add explicit override policy docs for composed tasks
  - Document current full-task replacement behavior when child and parent define same task name.
  - Clarify that task command lists do not deep-merge.
  - Add integration tests that lock current replacement behavior.

- [ ] Add optional strict mode for task override collisions
  - Add config or CLI validation mode that warns or errors when child overrides a parent task name.
  - Keep default behavior backward compatible unless task explicitly enables strict composition checks.
  - Add validation and integration tests for warning, error, and default modes.

## Task Variables

Instruction for items in this section:
- Keep variables separate from process environment.
- Resolve variables deterministically before command execution.
- Avoid adding expression language beyond direct interpolation.
- Keep phase 1 strict: string vars and `${{ vars.KEY }}` first.

### Phase 1: Schema

- [ ] Add root-level and task-level `vars` schema fields
  - Add optional `vars` map to root config and task schema.
  - Allow string values first.
  - Reject empty variable keys during validation.
  - Add schema and validation tests for valid and invalid variable definitions.

- [ ] Add variable key and reference validation rules
  - Define allowed variable key syntax and reject empty, whitespace-only, or invalid keys.
  - Return validation errors for unknown `${{ vars.KEY }}` references.
  - Add validation tests for valid keys, invalid keys, and unknown references.

### Phase 2: Resolution

- [ ] Add `${{ vars.KEY }}` interpolation
  - Resolve task-level vars before falling back to root-level vars.
  - Support interpolation in command strings, env values, and container image fields.
  - Return validation errors for unknown variable keys.
  - Add unit tests for precedence, interpolation success, and unknown-key failures.

- [ ] Add deterministic variable evaluation order
  - Resolve variable references in stable sorted key order instead of hash map iteration order.
  - Reuse already-resolved variable values instead of recomputing them per template occurrence.
  - Add unit tests that lock stable resolution and error ordering.

- [ ] Add variable cycle validation
  - Reject direct self-reference like `${{ vars.name }}` inside `vars.name`.
  - Reject multi-variable cycles across root and task `vars`.
  - Report full cycle path in validation output.
  - Add validation tests for self-cycle, two-node cycle, and longer cycle chains.

- [ ] Add strict variable scope validation
  - Reject root `vars` references to task-scoped `vars`.
  - Keep task `vars` allowed to reference root `vars`.
  - Document and test task-level override precedence over root-level values.

- [ ] Allow vars to reference saved command outputs
  - Resolve `${{ outputs.NAME }}` inside variable values after output-producing commands complete.
  - Reject forward references to outputs not yet available in task execution order.
  - Add integration tests for valid output-backed vars and invalid forward references.

- [ ] Reject output-backed vars in unsupported execution modes
  - Reject task `vars` references to `${{ outputs.NAME }}` when task execution mode is parallel.
  - Reject root `vars` references to `${{ outputs.NAME }}` because root scope has no task-local outputs.
  - Add validation tests for parallel-task and root-scope output references.

- [ ] Preserve current output publication semantics for output-backed vars
  - Keep `${{ outputs.NAME }}` unavailable when producing command fails, even with `ignore_errors: true`.
  - Keep nested task outputs isolated from parent task `vars` unless explicitly produced in same task scope.
  - Add integration tests for failed output production and nested-task output isolation through vars.

- [ ] Define multiline and trailing-newline semantics for output-backed vars
  - Reuse saved output behavior that preserves internal newlines and trims trailing newline characters.
  - Add integration tests for single-line and multi-line output-backed vars.

- [ ] Add cache fingerprint coverage for resolved vars
  - Include resolved variable values in task cache fingerprint inputs when vars affect command resolution.
  - Add integration test proving cache invalidates when root or task `vars` change.

- [ ] Define plan behavior for unresolved variable sources
  - Keep `mk plan` side-effect free when vars depend on runtime outputs.
  - Show unresolved or deferred var values explicitly instead of executing commands during planning.
  - Add text and JSON plan coverage for deferred variable values.

### Phase 3: Optional Shell-Computed Vars

- [ ] Add optional shell-computed variable schema
  - Add explicit object form for vars computed from shell commands instead of overloading plain strings.
  - Keep string-only vars supported and unchanged.
  - Add schema and validation tests for string and shell-computed forms.

- [ ] Define shell-computed var execution timing and reuse
  - Evaluate each shell-computed var at most once per task execution context.
  - Reuse computed values across multiple template references in same task.
  - Add integration tests proving single evaluation for repeated references.

- [ ] Add shell-computed var failure handling
  - Fail task when shell-computed var command exits non-zero unless explicit ignore behavior is designed.
  - Reject shell-computed vars from `mk plan` execution to preserve side-effect-free planning.
  - Add integration tests for non-zero exit, stderr output, and planning behavior.

- [ ] Add shell-computed var timeout and newline semantics
  - Reuse command timeout policy when available, or add explicit timeout field before enabling long-running shell vars.
  - Trim trailing newline characters and preserve internal newlines consistently with `save_output_as`.
  - Add integration tests for timeout, single-line, and multi-line command results.

- [ ] Add secret-safety rules for variables
  - Define whether `${{ secrets.PATH }}` is allowed inside `vars` values.
  - Redact secret-backed resolved var values from diagnostics, plan output, and cache metadata.
  - Add integration tests that prove secret values are not printed in error output.

### Phase 4: Docs

- [ ] Document variable workflows
  - Add README examples for root vars, task vars, and output-backed vars.
  - Clarify difference between `vars` interpolation and process `env`.
  - Document phase limitations for parallel tasks, planning, and shell-computed vars if enabled later.

## Output Plumbing

Instruction for items in this section:
- Extend existing `save_output_as` feature without breaking current syntax.
- Keep output capture explicit and predictable.
- Avoid hidden shell-specific parsing behavior.

### Phase 1: Capture Expansion

- [ ] Add stderr capture support for local commands
  - Add command field to save stderr under explicit output name.
  - Keep stdout capture behavior unchanged.
  - Add integration tests for stdout-only, stderr-only, and combined command output.

- [ ] Add exit code capture support for local commands
  - Add command field to save process exit status for later interpolation.
  - Preserve current failure semantics when command exits non-zero.
  - Add integration tests for successful and failing commands with saved exit codes.

### Phase 2: Structured Extraction

- [ ] Add JSON field extraction from saved command output
  - Add command field to parse saved stdout as JSON and extract a dot-path into named outputs.
  - Return validation or runtime errors for invalid JSON and missing paths.
  - Add integration tests for valid extraction, invalid JSON, and missing path cases.

### Phase 3: Artifact Writes

- [ ] Add command field to write saved output to a file
  - Allow writing a named saved output into explicit file path after command completion.
  - Create parent directories when safe and requested by config.
  - Add integration tests for successful writes and existing-path conflicts.

## Task Metadata Docs

Instruction for items in this section:
- Keep metadata descriptive only unless command explicitly uses it.
- Improve discovery without changing execution semantics.

### Phase 1: Schema

- [ ] Add task `description` and `group` schema fields
  - Add optional string fields to task schema.
  - Reject empty strings during validation.
  - Add schema and validation tests for accepted and rejected values.

### Phase 2: CLI Output

- [ ] Show task descriptions in `mk list`
  - Add formatted description output to default and plain list views.
  - Preserve stable task ordering.
  - Add integration tests for tasks with and without descriptions.

- [ ] Group task list output by `group`
  - Add grouped view for tasks that declare `group`.
  - Keep ungrouped tasks visible in separate deterministic section.
  - Add integration tests for grouped and ungrouped list output.

- [ ] Add `mk help <task>` task-specific help output
  - Print description, group, labels, dependencies, and resolved commands for one task.
  - Keep top-level Clap help behavior unchanged.
  - Add integration tests for existing and missing task names.

### Phase 3: Examples

- [ ] Add task `examples` metadata field
  - Add optional string list field to task schema.
  - Print examples in `mk help <task>`.
  - Add schema and integration tests for example rendering.

## Init Templates

Instruction for items in this section:
- Keep generated templates minimal and runnable.
- Match template format to requested output extension.
- Reuse shared sample task content where practical.

### Phase 1: Format Support

- [x] Add TOML template generation to `mk init`
  - Generate sample config when output path ends with `.toml`.
  - Keep YAML template behavior unchanged.
  - Add integration snapshots for TOML init stdout and file contents.

- [x] Add JSON template generation to `mk init`
  - Generate sample config when output path ends with `.json`.
  - Add integration snapshots for JSON init stdout and file contents.

- [x] Add Lua template generation to `mk init`
  - Generate sample config when output path ends with `.lua`.
  - Add integration snapshots for Lua init stdout and file contents.

### Phase 2: Validation

- [x] Validate `mk init` output extension against supported template formats
  - Reject unsupported extensions with actionable error text.
  - Update existing init tests for supported and unsupported paths.

## Cache Fingerprinting

Instruction for items in this section:
- Preserve cache correctness over cache hit rate.
- Prefer explicit invalidation inputs over hidden heuristics.
- Keep fingerprint serialization deterministic across runs and platforms.

### Phase 1: Output and Input Correctness

- [x] Invalidate cache when declared output content drifts
  - Hash current declared output file contents or equivalent stable metadata during cache-hit evaluation instead of checking existence only.
  - Treat missing or externally modified outputs as cache misses even when inputs and env are unchanged.
  - Add integration tests for external output edits and output deletions after a cached run.

- [x] Improve directory input fingerprinting
  - Recurse into directory inputs and hash child entry paths and file contents instead of relying on directory metadata only.
  - Keep deterministic traversal order across platforms.
  - Add integration tests for nested file edits, file additions, and file removals inside directory inputs.

### Phase 2: Fingerprint Stability

- [x] Replace debug-string task fingerprint serialization with explicit structured hashing
  - Stop relying on `Debug` formatting for `commands`, `preconditions`, and dependency data in cache fingerprints.
  - Serialize fingerprint-relevant task fields in a stable explicit order.
  - Add unit tests that lock fingerprint stability for equivalent task definitions.

- [x] Add cache fingerprint coverage tests for task configuration changes
  - Prove cache invalidates when command strings, outputs, env values, shell, execution mode, or secret path config changes.
  - Add targeted tests for one-field changes instead of one broad snapshot.

### Phase 3: Dynamic Input Visibility

- [x] Warn when cached tasks contain dynamic shell-derived command inputs without declared `inputs`
  - Detect shell substitutions or other runtime-derived command fragments in cacheable command fields where practical.
  - Emit validation warnings that cache invalidation may miss undeclared dynamic dependencies.
  - Add validation tests for warning and no-warning cases.

- [x] Document cache fingerprint blind spots and declaration rules
  - Clarify that undeclared runtime reads such as shell-derived values, git state, and external files do not invalidate cache automatically.
  - Add README guidance to declare such dependencies in `inputs` or `env_file`.

## Remote Cache

Instruction for items in this section:
- Keep local cache behavior default.
- Add remote cache as explicit opt-in.
- Preserve cache correctness over cache hit rate.

### Phase 1: Schema

- [ ] Add cache backend schema for local and remote modes
  - Add `cache.backend` field with `local` and `fs` values first.
  - Add backend-specific path field for shared filesystem cache root.
  - Reject unknown backend values during validation.
  - Add schema and validation tests for valid and invalid cache backend configs.

### Phase 2: Shared Filesystem Backend

- [ ] Add shared filesystem cache read support
  - Resolve cache entries from configured shared cache root before local miss execution.
  - Keep current local cache behavior when backend is `local`.
  - Add integration tests for remote hit and remote miss flows.

- [ ] Add shared filesystem cache write support
  - Write cache metadata and output artifacts into configured shared cache root after successful execution.
  - Avoid partial cache entry publication on failed tasks.
  - Add integration tests for successful writes and failed-task no-write behavior.

### Phase 3: Diagnostics and Docs

- [ ] Add remote cache diagnostics to `mk doctor`
  - Print configured cache backend and shared cache root reachability.
  - Mark unreadable shared cache roots as failures.
  - Add integration tests for reachable and unreachable remote cache roots.

- [ ] Document remote cache workflows
  - Add README examples for local and shared filesystem cache configuration.
  - Clarify cache consistency limits and recommended CI usage.
