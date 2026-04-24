# Issues

## Config and Vault Integration

Instruction for items in this section:
- Keep each checklist item scoped to one small workable chunk.
- Describe exact code, command, schema field, validation rule, or test to add/change.
- Do not combine multiple implementation steps into one checklist item if they can be merged separately.
- Prefer additive wording like "add", "replace", "update", "remove", "validate", "test".
- Avoid broad goals without concrete implementation detail.

### Phase 1: Schema and Migration Layer

- [x] Add `SecretBackend` enum
  - Implement enum values: `BuiltInPgp`, `Gpg`.
  - Stop using `gpg_key_id.is_some()` as backend switch.
  - Use backend enum in runtime, CLI, validation, and vault metadata.

- [x] Add `SecretSettings` schema type and use it in both `TaskRoot` and `TaskArgs`
  - Implement `SecretSettings` with exact fields: `backend`, `vault_location`, `keys_location`, `key_name`, `gpg_key_id`, `secrets_path`.
  - Replace root/task scalar secret fields with `secrets: SecretSettings`.
  - Keep task-level `secrets` as partial override merged on top of root `secrets`.

- [x] Keep old secret scalar fields readable during migration
  - Continue deserializing old root/task fields: `vault_location`, `keys_location`, `key_name`, `gpg_key_id`, `secrets_path`.
  - Map them into `SecretSettings` during load.
  - Emit validation warning when old fields are used.
  - Do not remove old fields until tests and docs for `secrets` block exist.

### Phase 2: Shared Resolution Path

- [x] Replace `SecretConfig::resolve` with source-aware resolver
  - Implement one resolver function that accepts CLI overrides, task-level `SecretSettings`, root-level `SecretSettings`, vault metadata, built-in defaults.
  - Return one resolved struct with final values plus backend.

- [x] Make task runtime use resolved `SecretSettings` only
  - Update `TaskContext` to carry resolved secret settings as one struct instead of separate fields.
  - Remove duplicated `secret_vault_location`, `secret_keys_location`, `secret_key_name`, `secret_gpg_key_id` state from `TaskContext`.
  - Update `${{ secrets.* }}` and `secrets_path` loading to use resolved settings only.

- [x] Expand `.vault-meta.toml` to full vault settings file
  - Store exact fields: `backend`, `gpg_key_id`, `key_name`, `keys_location`.
  - Read this metadata through typed struct, not ad-hoc optional lookup.
  - Keep reading old metadata files that only contain `gpg_key_id`.

### Phase 3: CLI Integration

- [x] Allow `mk secrets` to run without `tasks.yaml`
  - Add `Command::Secrets(_)` to config-less command allow list in CLI bootstrap.
  - Keep current config loading behavior when `-c/--config` is explicitly passed.
  - Add integration test for `mk secrets vault list --vault-location <path>` in directory without config file.
  - If no config file exists, run secrets commands with CLI flags, vault metadata, and built-in defaults only.
  - If config file exists and defines vault settings, use those config values as CLI defaults for unified behavior with task runtime.

- [x] Make `mk secrets` CLI load config file and use same resolver as runtime
  - When user runs `mk secrets ...`, load active config file using same resolution rules as main CLI.
  - Apply precedence in exact order: command flags, task/root config `secrets`, vault metadata, built-in defaults.
  - Remove fallback-only behavior from `src/cli/bin/secrets/context.rs`.

- [x] Add `mk secrets doctor`
  - Print active config file path.
  - Print resolved backend.
  - Print resolved vault path.
  - Print resolved keys path.
  - Print resolved key name.
  - Print resolved gpg key id.
  - Print source for each resolved value.
  - Print whether vault metadata was used.

### Phase 4: Validation and Safety Rules

- [x] Validate secret config combinations with exact rules
  - Add validation error for `backend = "gpg"` without `gpg_key_id`.
  - Add validation error for `backend = "pgp"` without `key_name`.
  - Add validation error for `backend = "pgp"` with missing `keys_location` only if no default applies.
  - Add validation error for `backend = "gpg"` combined with incompatible PGP-only settings when explicit override conflicts.
  - Add validation error when old scalar fields and `secrets` block are both present with different values.

### Phase 5: Config Mutation Workflow

- [x] Add `mk secrets vault init --write-config`
  - Implement flag to update active config file after vault initialization.
  - Create vault if missing.
  - Write `.vault-meta.toml`.
  - Add or update root `secrets` block in config file.
  - Preserve unrelated config content.
  - Refuse write if config file format unsupported for mutation.

### Phase 6: Docs and Coverage

- [x] Update docs and examples to use `secrets:` block everywhere
  - Replace examples that show root scalar secret fields with `secrets:` block.
  - Add one example for root defaults.
  - Add one example for task override.
  - Add one example for GPG backend.
  - Add one example for `mk secrets vault init --write-config`.
  - Document exact precedence order.

- [x] Add integration tests for full config-vault merge behavior
  - Add test for root `secrets` used by task runtime.
  - Add test for task `secrets` overriding root `secrets`.
  - Add test for `mk secrets` commands reading config defaults.
  - Add test for vault metadata filling missing config values.
  - Add test for CLI flags overriding config and metadata.
  - Add test for old scalar fields still working.
  - Add test for `backend = "gpg"` and `backend = "pgp"` validation rules.
  - Add test for `mk secrets doctor` output.
  - Add test for `mk secrets vault init --write-config` config mutation flow.
