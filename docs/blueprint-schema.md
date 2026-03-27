# Blueprint Schema

Every authored package must separate human-readable blueprint docs from machine-readable contract outputs.

## Authority docs

Place under `blueprint/authority/`.

Each authority doc should include:

- `Purpose`
- `Authority Scope`
- `Truth Rules`
- `Conflict Resolution`
- `Non-Goals`

## Workflow docs

Place under `blueprint/workflow/`.

The workflow overview should include:

- `Stage Order`
- `Entry Rule`
- `Exit Gate`
- `Cross-Stage Split Rule`
- `Stop Conditions`
- `Worktree Model`
- `Parallel Worktree Policy`
- `Shared Authority Paths`
- `Worktree Roles`
- `Worktree Sync Rule`
- `Worktree Merge Back Rule`
- `Worktree Cleanup Rule`

## Stage docs

Place under `blueprint/stages/`.

Each stage doc should include:

- `Stage`
- `Intent`
- `Allowed Scope`
- `Forbidden Scope`
- `Deliverables`
- `Required Verification`
- `Review Focus`
- `Advance Rule`
- `Repair Routing`

Structured imports may also define stage-specific detail through:

- `stages` arrays in `json` or `toml`
- stage source-file blocks under `blueprint/stages/`

When present, these stage-specific values override generic stage defaults for the matching stage only.

## Module docs and machine outputs

Place under `blueprint/modules/`.

Required machine-readable file:

- `00-module-catalog.json`

Each module entry must include:

- `module_id`
- `layer`
- `responsibility`
- `recommended_language`
- `allowed_languages`
- `forbidden_languages`
- `reason`
- `hot_path`
- `cross_platform_requirement`
- `boundary_type`
- `owned_artifacts`
- `preferred_worktree_role`
- `allowed_worktree_roles`

## Machine-readable outputs

Place under `.codex/auto-dev/`.

Required outputs:

- `project-contract.toml`
- `resolved-contract.json`
- `worktree-protocol.json`
- `blueprint-manifest.json`
- `semantic-ir.json`
- `normalization-report.json`
- `change-report.json`
- `patch-base.json`
- `patch-plan.json`
- `patch-execution-report.json`
- `decision-summary.json`
- `agent-brief.json`
- `readiness.json`
- `task-progress.json`
- `author-report.md`

Operational output when `migrate-workspace` runs:

- `migration-report.json`

`normalization-report.json` should include:

- `source_type`
- `source_files`
- `preserved_sections`
- `inferred_sections`
- `dropped_sections`
- `semantic_hints`
- `semantic_risks`
- `unresolved_ambiguities`
- `status`

`change-report.json` should include:

- `operation_count`
- `conflict_count`
- `operations`
- `patch_operation_count`
- `patch_operations`

`patch-plan.json` should include:

- `mode`
- `operation_count`
- `conflict_count`
- `base_fingerprint`
- `result_fingerprint`
- `operations`

Each patch operation should include:

- `scope`
- `target_id`
- `strategy`
- `affected_paths`
- `target_worktree_roles`
- `reverse_strategy`
- `value_lines`
- `previous_value_lines`
- `stage_name`
- `previous_stage_name`
- `strategy_metadata`

`strategy_metadata` should at least carry:

- `strategy_family`
- `replay_direction`
- `is_reversible`
- `risk_level`
- `review_required`
- `apply_mode`

`patch-base.json` should include:

- `mode`
- `artifact_status`
- `base_fingerprint`
- `sections`
- `stage_sections`

`patch-execution-report.json` should include:

- `mode`
- `operation_count`
- `applied_operation_count`
- `base_fingerprint`
- `expected_result_fingerprint`
- `replayed_result_fingerprint`
- `replay_status`
- `mismatch_count`
- `mismatches`
- `reverse_replayed_base_fingerprint`
- `reversibility_status`
- `reverse_mismatch_count`
- `reverse_mismatches`
- `scope_validation_status`
- `scope_mismatch_count`
- `scope_mismatches`

`semantic-ir.json` should include:

- `mode`
- `project_name`
- `source_type`
- `source_provenance`
- `derivation`
- `source_files`
- `source_fingerprint`
- `projection_fingerprint`
- `normalized_sections`
- `normalized_stage_sections`
- `normalized_section_origins`
- `normalized_stage_section_origins`
- `semantic_frames`
- `semantic_clusters`
- `sections`
- `stages`
- `preserved_sections`
- `inferred_sections`
- `semantic_hints`
- `semantic_risks`
- `semantic_conflicts`
- `unresolved_ambiguities`

`task-progress.json` should include:

- `total_stages`
- `completed_stages`
- `current_stage_id`
- `overall_progress_percent`
- `stages`

`decision-summary.json` should include:

- `mode`
- `readiness_state`
- `reason`
- `blocking`
- `review_required`
- `blocking_semantic_conflict_count`
- `review_required_semantic_conflict_count`
- `high_risk_patch_operation_count`
- `review_required_patch_operation_count`
- `gate_hold_count`
- `recommended_action_count`
- `blocking_kinds`
- `review_required_kinds`
- `blocking_kind_counts`
- `review_required_kind_counts`
- `primary_blocker_kind`
- `primary_blocker_scope`
- `primary_blocker_target_id`
- `primary_blocker_summary`
- `primary_recommended_action`
- `top_blockers`
- `top_review_items`
- `scoped_worktree_roles`
- `entries`
- `gate_holds`
- `recommended_actions`

`agent-brief.json` should include:

- `project_name`
- `mode`
- `readiness_state`
- `reason`
- `current_stage_id`
- `current_stage_name`
- `total_stages`
- `completed_stages`
- `overall_progress_percent`
- `blocking`
- `review_required`
- `primary_blocker_kind`
- `primary_blocker_scope`
- `primary_blocker_target_id`
- `primary_blocker_summary`
- `primary_recommended_action`
- `top_blockers`
- `top_review_items`
- `scoped_worktree_roles`
- `gate_holds`
- `next_actions`

Each decision summary entry should include:

- `kind`
- `scope`
- `target_id`
- `severity`
- `blocking`
- `review_required`
- `worktree_roles`
- `summary`
- `recommended_action`

`readiness.json` should include:

- `state`
- `reason`
- `fingerprint`
- `blocking_semantic_conflict_count`
- `review_required_semantic_conflict_count`
- `high_risk_patch_operation_count`
- `review_required_patch_operation_count`
- `gate_holds`
- `recommended_actions`

`worktree-protocol.json` should include:

- `schema_version`
- `model`
- `parallel_worktree_policy`
- `shared_authority_paths`
- `sync_rule`
- `merge_back_rule`
- `cleanup_rule`
- `roles`

When `shared_authority_paths` are present, `parallel_worktree_policy`, `sync_rule`, `merge_back_rule`, and `cleanup_rule` must explicitly mention how those shared authority or workflow paths are coordinated. Merely describing isolated stage/module work is not enough.

Each worktree role should include:

- `role_id`
- `branch_pattern`
- `stage_ids`
- `module_ids`
- `exclusive_paths`

Each worktree role must declare at least one deterministic ownership scope through `stage_ids`, `module_ids`, or `exclusive_paths`.
When `model = stage-isolated-worktree`, each role must bind to exactly one `stage_id`.
When `model = module-isolated-worktree`, each role must declare explicit `module_ids`.
The emitted default `parallel_worktree_policy`, `sync_rule`, `merge_back_rule`, and `cleanup_rule` are model-aware: stage-isolated workflows default to stage-role language, while module-isolated workflows default to module-role language.

Worktree validation must also reject:

- empty `parallel_worktree_policy`, `shared_authority_paths`, `sync_rule`, `merge_back_rule`, or `cleanup_rule` once worktree roles exist
- placeholder `parallel_worktree_policy`, `sync_rule`, `merge_back_rule`, or `cleanup_rule` values such as `todo`, `tbd`, or `pending`
- non-actionable `parallel_worktree_policy`, `sync_rule`, `merge_back_rule`, or `cleanup_rule` entries that never reference a worktree scope together with an actual policy or action
- worktree rule entries whose model language conflicts with the declared worktree model, such as stage-role wording under `module-isolated-worktree`
- invalid `branch_pattern` values that contain spaces, backslashes, ref-reserved tokens, or other non-portable branch syntax
- duplicate `branch_pattern` values across roles
- overlapping `branch_pattern` hierarchies across roles, such as `codex/foundation` and `codex/foundation/hardening`
- duplicate explicit `module_ids` ownership across roles
- exclusive paths that overlap each other across roles
- exclusive paths that overlap shared authority paths

These outputs are not only emitted; they are reloaded and revalidated as a consistency set by `validate-workspace`. `semantic-ir.json` is validated as a source-first normalized IR that must also project cleanly back onto the emitted docs. `worktree-protocol.json` is validated against workflow sections, `project-contract.toml`, and `resolved-contract.json` so worktree-aware development flow remains machine-verifiable instead of living only in prose. `patch-base.json` is validated against `patch-plan.json` so standalone patch apply always has a durable normalized base snapshot, and `patch-execution-report.json` must prove both forward replay and reverse replay back to that base. `decision-summary.json` is validated against `semantic_conflicts`, patch risk metadata, readiness, and worktree scope evidence so short-form gate truth cannot silently drift away from the underlying artifacts. `agent-brief.json` is then validated as a strictly derived handoff artifact built from `decision-summary.json`, `readiness.json`, and `task-progress.json`, so agent-facing short summaries cannot silently diverge from gate truth. Older workspaces that predate `patch-base.json`, `patch-plan.json`, `patch-execution-report.json`, `semantic-ir.json`, `decision-summary.json`, `agent-brief.json`, or `worktree-protocol.json` may still be reloaded because the runtime can derive fallback artifacts from current workspace state, `change-report.json`, `patch-plan.json`, emitted blueprint docs, and the embedded contract/workflow truth before validation.
`patch-plan.json` is also worktree-aware: every patch operation must declare deterministic `affected_paths` and any `target_worktree_roles` implied by the current worktree protocol. `patch-execution-report.json` must then prove that replay stayed inside those derived worktree boundaries.

Each semantic frame records:

- package or stage scope
- canonical local section
- raw source label
- source locator path
- origin kind such as heading, inline label, nested structured key, or normalized fallback
- mapping confidence
- normalized values

Inline markdown semantic labels may also absorb continuation lines and bullet items, so a label such as `Acceptance Criteria:` can yield multiple normalized values instead of only preserving the same-line suffix.
When stage-scoped indirect sources disagree, semantic risk and ambiguity output should preserve source-group and confidence detail instead of collapsing that conflict into a generic warning.
Structured semantic conflicts should preserve the conflicted scope, canonical section, conflict kind, source-group locators, source labels, merged values, confidence profile, severity, blocking status, and recommended action so downstream runtimes can reason over conflict evidence and basic next-step decisions without reparsing free-form text.
`readiness.json` must project blocking and review-required semantic conflicts plus high-risk and review-required patch operations into machine-readable gate summary fields instead of leaving that interpretation to downstream consumers. `decision-summary.json` then condenses that gate truth into a shorter machine-consumable artifact with scoped entries, blocker-kind classification, primary blocker fields, stable top blocker/review slices, and worktree-role context, so downstream consumers do not need to traverse every raw evidence file before choosing the next action. `agent-brief.json` is the shortest derived handoff view and should summarize the current stage, primary blocker, top blockers/review items, worktree role scope, and next actions without inventing any gate logic that is not already present in `decision-summary.json` or `readiness.json`.
Mapping confidence is intentionally tiered so direct exact mappings, aliases, inline heuristics, cross-source stage alias evidence, and path-alias stage evidence do not collapse into one generic confidence bucket.

Stage-scoped semantic evidence may also be reconstructed across source blocks when deterministic stage ids are known in one block and later markdown headings reuse stage aliases such as `Foundation` or `Hardening`.
Stage-scoped semantic evidence may also be reconstructed from stage-alias source file names such as `foundation-notes.md` when the file body contains recognized stage-local headings or inline semantic labels.
Stage-scoped semantic evidence may also be reconstructed from document-level metadata such as front matter `stage: foundation`, list metadata like `- Milestone: foundation`, or inline metadata like `Stage ID: stage-02` and `phase_id = "stage-02"`, so stage-local headings do not have to rely on file naming alone.

Each semantic cluster records:

- scope and canonical section
- merged source labels and locators
- origin kinds and confidence levels
- merged normalized values
- merge pattern such as `single-source`, `single-source-heuristic`, `multi-source`, or `multi-source-heuristic`

When multiple indirect stage-scoped sources disagree on conflict-sensitive sections such as `deliverables` or `required_verification`, normalization should preserve the merged evidence in `semantic-ir.json` but also emit semantic risks and unresolved ambiguities in `normalization-report.json`.
