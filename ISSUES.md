# Issues

Instruction for all the items in this file:
- Keep each checklist item scoped to one small workable chunk.
- Describe exact code, command, schema field, validation rule, or test to add/change.
- Do not combine multiple implementation steps into one checklist item if they can be merged separately.
- Prefer additive wording like "add", "replace", "update", "remove", "validate", "test".
- Avoid broad goals without concrete implementation detail.

## Watch Mode

Instruction for items in this section:
- Keep watch behavior deterministic and explicit.
- Reuse existing task resolution, validation, and cache semantics where possible.
- Prefer additive CLI flags over implicit background behavior.

### Phase 1: CLI Surface

- [x] Add `mk watch <task>` command
  - Add `watch` subcommand to CLI help and docs.
  - Require a task name or label filter input using same selection rules as `mk run`.
  - Reuse current config file discovery and task resolution.
  - Add help snapshot coverage for `mk watch --help`.

- [x] Add watch path and debounce flags
  - Add repeatable `--path <PATH>` flags to override watched inputs.
  - Add `--debounce <DURATION>` flag for filesystem event coalescing.
  - Validate duration parsing and reject zero debounce values.
  - Add CLI parsing tests for repeated paths and invalid durations.

### Phase 2: Execution Behavior

- [x] Add filesystem watch loop for local task reruns
  - Watch explicit `--path` values when provided.
  - Re-run selected task after debounce when matching changes arrive.
  - Print a clear rerun reason before each execution.
  - Add integration test covering one file change and one rerun.

- [x] Reuse task `inputs` as default watch paths
  - Use declared task `inputs` when `mk watch` runs without `--path`.
  - Skip unresolved glob patterns without panicking.
  - Return a user-facing error when neither `--path` nor task `inputs` exist.
  - Add integration tests for inferred paths and empty watch target errors.

### Phase 3: Quality of Life

- [x] Add `--clear` flag to `mk watch`
  - Clear terminal before each rerun when enabled.
  - Keep default output append-only when flag is absent.
  - Add integration test for flag parsing and screen-clear branch selection.

- [x] Add `.mkignore` support to `mk watch` (use ignore crate of ripgrep)
  - Load ignore rules from `.mkignore` at config root using gitignore-style pattern semantics.
  - Apply ignore filtering to watched descendant paths for both explicit `--path` roots and inferred task `inputs`.
  - Keep ignore handling scoped to watch behavior and do not change cache `inputs` resolution semantics.
  - Allow negated patterns so users can re-include specific paths under ignored directories.
  - Add integration test coverage for ignored paths and negated re-includes.

- [x] Document watch workflows
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

- [x] Add stderr capture support for local commands
  - Add command field to save stderr under explicit output name.
  - Keep stdout capture behavior unchanged.
  - Add integration tests for stdout-only, stderr-only, and combined command output.

- [x] Add exit code capture support for local commands
  - Add command field to save process exit status for later interpolation.
  - Preserve current failure semantics when command exits non-zero.
  - Add integration tests for successful and failing commands with saved exit codes.

### Phase 2: Structured Extraction

- [x] Add JSON field extraction from saved command output
  - Add command field to parse saved stdout as JSON and extract a dot-path into named outputs.
  - Return validation or runtime errors for invalid JSON and missing paths.
  - Add integration tests for valid extraction, invalid JSON, and missing path cases.

### Phase 3: Artifact Writes

- [x] Add command field to write saved output to a file
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

## Markdown Task Files

Instruction for items in this section:
- Keep Markdown task files as small command-only wrapper over existing local shell task execution.
- Parse explicit Markdown structure only; do not infer YAML-like config from prose.
- Preserve current execution semantics for plain shell command strings.

### Phase 1: Loader

- [ ] Add markdown config file extension support
  - Add `.md` and `.markdown` handling to task root file loader.
  - Keep existing YAML, TOML, JSON, and Lua behavior unchanged.
  - Return unsupported-format errors for other extensions as today.
  - Add unit tests for supported markdown extensions.

- [ ] Add Markdown heading-based task extraction
  - Treat each level-2 heading `## Task Name` as one task definition.
  - Use heading text as task name without additional slug generation.
  - Keep task order deterministic based on file order.
  - Add unit tests for one-task and multi-task markdown parsing.

- [ ] Add markdown description extraction
  - Use first non-empty paragraph after each task heading as task description.
  - Keep description optional when paragraph text is absent.
  - Stop description capture before first fenced code block under same heading.
  - Add unit tests for present and missing descriptions.

- [ ] Add fenced code block command extraction
  - Convert fenced code blocks under a task heading into ordered local command entries.
  - Accept unlabeled fences and shell-style info strings like `sh`, `bash`, `shell`, `zsh`, `pwsh`, and `powershell`.
  - Preserve command text exactly as written inside each fence.
  - Add unit tests for single-command and multi-command task sections.

- [ ] Ignore and reject markdown tasks with no command blocks
  - Return a user-facing error when a task heading has no fenced code block commands.
  - Report the task heading name in the error output.
  - Keep non-task Markdown content ignored outside task sections.
  - Add unit tests for empty task section failures.

### Phase 2: Execution Mapping

- [ ] Map markdown tasks into existing shell command task structs
  - Build parsed markdown tasks into existing task structs instead of introducing a new execution path.
  - Map each fenced code block to existing plain shell command execution entries.
  - Map extracted paragraph text to task description field.
  - Add unit tests for task struct conversion.

- [ ] Keep markdown task files command-only
  - Reject Markdown-only extensions for dependencies, env, cache, labels, secrets, containers, and other YAML task fields.
  - Keep markdown parsing limited to task name, description, and command blocks.
  - Return actionable errors when unsupported markdown task metadata is introduced.
  - Add validation tests for unsupported markdown features.

### Phase 3: CLI Discovery And Init

- [ ] Add markdown files to default config discovery
  - Add `tasks.md`, `tasks.markdown`, `.mk/tasks.md`, and `.mk/tasks.markdown` to default config candidates.
  - Update missing-config help text to mention markdown fallback files.
  - Keep explicit `--config` behavior unchanged.
  - Add CLI tests for markdown candidate discovery.

- [ ] Add `mk init` markdown output support
  - Allow `mk init` output paths ending in `.md` and `.markdown`.
  - Render sample markdown config using `##` task headings, optional paragraph description, and fenced shell code blocks.
  - Keep existing `.yaml`, `.yml`, `.toml`, `.json`, and `.lua` output behavior unchanged.
  - Add tests for supported markdown init output paths.

### Phase 4: Documentation

- [ ] Document markdown task file workflow
  - Add README examples for `tasks.md` and `mk -c tasks.md run <task>`.
  - Document `##` heading task syntax, optional paragraph descriptions, and fenced command blocks.
  - Clarify that markdown task files support command execution only and do not expose full YAML task schema.
  - Add doc coverage for accepted fenced code block info strings.

## Makefile Support

Instruction for items in this section:
- Prefer delegated GNU Make integration before deeper parity work.
- Reuse existing config discovery, TaskRoot loading, task selection, and CLI plumbing where practical.
- Keep milestone 1 focused on lowest-risk path: discovery, import, list, completion, run.
- Do not add native Make parser in milestone 1 or milestone 2.

### Milestone 1: Foundation And Delegated Run

#### Phase 1: Config Discovery

- [ ] Add Makefile config candidates to default config lookup
  - Add `Makefile`, `makefile`, and `GNUmakefile` to default config candidates.
  - Apply same candidate order in regular config resolution and completion config resolution.
  - Keep existing structured config candidates ahead of Makefile candidates to avoid surprising current users.
  - Add tests for fallback discovery order and explicit `-c Makefile` behavior.

- [ ] Replace extension-only config detection with path-name aware format detection
  - Replace extension-only branching in task root loader with helper that recognizes `Makefile`, `makefile`, and `GNUmakefile` by file name.
  - Keep `.yaml`, `.yml`, `.toml`, `.json`, and `.lua` behavior unchanged.
  - Remove current hard error for Makefiles once named-file detection exists.
  - Add unit tests for named Makefile detection and unsupported file names.

#### Phase 2: Target Discovery

- [ ] Add reusable GNU Make target discovery helper
  - Add helper module that executes `make` against chosen config path and returns discovered target names in deterministic sorted order.
  - Resolve targets from configured Makefile path without changing current working directory semantics used by `mk`.
  - Return actionable errors when `make` binary is missing or target discovery command fails.
  - Add unit or integration tests for successful discovery and missing `make` binary failure.

- [ ] Filter discovered targets to user-runnable targets only
  - Exclude internal pattern rules, suffix rules, and special built-in targets from discovery results.
  - Keep explicit `.PHONY` targets included.
  - Preserve deterministic sorted output for list, completion, and selector behavior.
  - Add tests covering normal targets, `.PHONY` targets, and filtered special targets.

- [ ] Add Make-backed `TaskRoot` adapter with minimal task metadata
  - Convert discovered Make targets into synthesized `TaskRoot.tasks` entries before existing CLI task flows consume them.
  - Populate task name and fallback description only; do not infer labels, watch inputs, or cache metadata in milestone 1.
  - Set `source_path` and config base dir consistently with other config formats.
  - Add tests that imported targets appear in `TaskRoot` with stable names and source path.

#### Phase 3: CLI Parity Baseline

- [ ] Load Make-backed configs through existing CLI entry path
  - Reuse current CLI load paths so `mk`, `mk run`, `mk list`, and completion work with imported Make targets.
  - Keep existing YAML, TOML, JSON, and Lua flows unchanged.
  - Add integration tests for `mk -c Makefile list` and implicit `Makefile` discovery.

- [ ] Add `mk list` support for Make-backed targets
  - Show imported target names in plain, table, and JSON list output using existing task listing flow.
  - Use fallback description text when Make metadata is absent.
  - Keep label filtering unavailable for imported Make targets instead of inventing synthetic labels.
  - Add integration tests for plain and JSON list output from Makefile config.

- [ ] Add dynamic completion support for Make-backed targets
  - Reuse completion config resolution so shell completion returns imported Make targets.
  - Keep current Bash, Zsh, and Fish dynamic completion behavior unchanged for structured configs.
  - Add integration test covering completion prefix filtering against Makefile config.
  - Add integration test covering implicit config fallback to `Makefile` for completion.

- [ ] Add delegated `mk run <target>` execution for Make-backed configs
  - Run selected imported target by invoking `make` against active Makefile path instead of synthesizing command bodies.
  - Preserve current exit-code handling and user-facing task selection semantics.
  - Reject `mk` forwarded trailing args for Make-backed configs with explicit error until argument mapping is designed.
  - Add integration tests for successful target run, failing target run, and forwarded-args rejection.

#### Phase 4: Milestone 1 Guardrails

- [ ] Add explicit unsupported-operation errors for milestone 1 gaps
  - Return clear user-facing errors when Make-backed configs use `mk plan`, `mk watch`, or label filters in milestone 1.
  - Keep errors specific to Make-backed configs so structured config behavior does not change.
  - Add integration tests for each unsupported command or flag path.
  - Document milestone 1 support matrix in `README.md`.

- [ ] Update docs for Makefile milestone 1 support
  - Add README examples for `mk -c Makefile list` and `mk -c Makefile run <target>`.
  - Document supported config names: `Makefile`, `makefile`, `GNUmakefile`.
  - Document milestone 1 limitations: no labels, no watch, no plan, no forwarded args.
  - Update format summary in library docs so docs stop implying YAML-only support.

### Milestone 2: Deeper Parity

#### Phase 1: Imported Metadata

- [ ] Add target description extraction for imported Make targets
  - Parse conventional inline target comments such as `target: ## description` when present.
  - Keep fallback description when comment metadata is absent.
  - Do not infer descriptions from recipe lines.
  - Add tests for comment-derived descriptions and fallback descriptions.

- [ ] Import explicit prerequisites into synthesized task dependencies where safe
  - Convert plain target prerequisites into `depends_on` entries when prerequisite also resolves to imported runnable target.
  - Skip file prerequisites and pattern prerequisites instead of treating them as task dependencies.
  - Preserve deterministic dependency ordering.
  - Add tests for target-to-target prerequisites and mixed file prerequisites.

#### Phase 2: Plan And Validate

- [ ] Add `mk plan` support for Make-backed configs
  - Produce plan output for imported Make targets using synthesized dependency graph plus delegated execution summary.
  - Show final execution step as `make <target>` against active Makefile path instead of fake recipe expansion.
  - Keep JSON and text plan output stable and explicit about delegated Make execution.
  - Add integration tests for text and JSON plan output.

- [ ] Add `mk validate` support for Make-backed configs
  - Validate Makefile path existence, `make` binary availability, imported target discovery success, and selected target existence.
  - Report unsupported Make-backed features as warnings or explicit notes instead of silent success.
  - Keep structured config validation rules unchanged.
  - Add integration tests for valid config, missing `make`, and discovery failure cases.

#### Phase 3: CLI Quality

- [ ] Add doctor output for Make-backed configs
  - Update doctor reporting to show detected config format as Makefile.
  - Report located `make` binary or missing-binary failure in doctor output.
  - Keep container runtime checks unchanged.
  - Add integration test or snapshot for doctor output with Make-backed config.

- [ ] Add task selector support for imported Make targets
  - Reuse existing selector candidate flow for Make-backed targets with description fallback or imported comment description.
  - Keep label-based filtering unsupported unless Make labels are explicitly designed later.
  - Add integration test for fuzzy selector candidate generation from imported Make targets.

- [ ] Add clear watch limitation messaging for Make-backed configs
  - Detect Make-backed config early in `mk watch` path and return explicit unsupported message until watch input semantics are designed.
  - Do not attempt to infer watch paths from Make prerequisites in milestone 2.
  - Add integration test for Make-backed watch rejection.
  - Document watch limitation in README support matrix.

#### Phase 4: Docs And Compatibility Matrix

- [ ] Add Makefile capability matrix to README
  - Document exact parity level for `list`, `completion`, `run`, `plan`, `validate`, `doctor`, `watch`, and labels.
  - Separate milestone 1 baseline from milestone 2 parity in changelog or roadmap wording.
  - Clarify GNU Make first-class support and BSD make non-goal for now.
  - Add examples for Makefile target descriptions and prerequisite-based planning.

- [ ] Add regression coverage for mixed-format discovery
  - Add integration tests proving `tasks.yaml` still wins over `Makefile` when both exist and no explicit config path is passed.
  - Add integration tests proving explicit `-c Makefile` bypasses structured config fallback.
  - Add integration tests proving unsupported arbitrary file names still fail with actionable error.
  - Keep current config discovery behavior stable for existing supported formats.
