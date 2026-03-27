# Readiness States

The authoring runtime should emit one readiness state at a time.

## States

- `draft`
  Initial inputs exist but the package is incomplete.
- `normalized`
  Source material has been normalized into the local schema.
- `contract-valid`
  Contract compilation and structural validation passed.
- `candidate-for-blueprint-gate`
The package is ready for `aegiscore-rust-runtime` to run blueprint gate.
- `stale`
  A previously emitted package no longer matches current inputs or policy.

## Rules

- `candidate-for-blueprint-gate` is the highest state this skill may emit.
- This skill must never emit `clean`, `pass`, or any stage-advance verdict.
- Any authority-changing edit should downgrade readiness until validation is rerun.
- `readiness.json` must also carry machine-readable gate summary fields for semantic conflicts and patch risk:
- `blocking_semantic_conflict_count`
- `review_required_semantic_conflict_count`
- `high_risk_patch_operation_count`
- `review_required_patch_operation_count`
- `gate_holds`
- `recommended_actions`
- Blocking semantic conflicts and high-risk patch operations do not automatically change the readiness state, but they must be projected into readiness so downstream runtimes can hold automation safely at gate time.
