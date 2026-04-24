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
