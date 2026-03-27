# ARA Error Codes

All machine-readable failures should carry a unique error code.

## ARA-1xxx Environment and runtime

- `ARA-1000`
  Generic runtime failure.
- `ARA-1001`
  Required path could not be resolved.
- `ARA-1002`
  Atomic write failed.
- `ARA-1003`
  Unsupported host platform behavior.

## ARA-2xxx Input, contract, and schema

- `ARA-2000`
  Input source is missing, unsupported by normalization policy, or cannot be reloaded from the required workspace bundle.
- `ARA-2001`
  Blueprint schema is incomplete.
- `ARA-2002`
  Contract TOML is invalid.
- `ARA-2003`
  Resolved contract JSON is inconsistent with source TOML, workflow stage order, workflow worktree protocol, stage documents, or emitted worktree protocol truth.
- `ARA-2004`
  Stage graph is invalid or cyclic.
- `ARA-2005`
  An emitted artifact schema version does not match the runtime-supported schema version. `validate-workspace` should fail with this code before `migrate-workspace` rewrites the package to the current schema.

## ARA-3xxx Policy and normalization

- `ARA-3000`
  Normalization or update would rewrite authority truth and must stop.
- `ARA-3001`
  Module language assignment or emitted module catalog violates embedded policy or the expected runtime module set.
- `ARA-3002`
  External blueprint is too ambiguous to normalize deterministically, including stage orders without explicit stage ids when the normalization policy forbids implicit stage creation.
- `ARA-3003`
  Required durable output is unavailable, including standalone patch apply requests that do not have an emitted `patch-base.json`.

## ARA-4xxx Lifecycle and readiness

- `ARA-4000`
  Readiness calculation failed.
- `ARA-4001`
  Package is not ready for blueprint gate handoff.
- `ARA-4002`
  Package fingerprint is stale relative to the current emitted package and inputs.

## ARA-5xxx Evidence and fingerprint

- `ARA-5000`
  Manifest generation failed.
- `ARA-5001`
  Normalization report is missing required evidence such as source files or status.
- `ARA-5002`
  Artifact fingerprint mismatch, including tampered manifest entries that no longer match on-disk package content.
- `ARA-5003`
  Change report, patch plan, patch scope metadata, or patch execution replay evidence is missing, internally inconsistent, or no longer aligned with runtime expectations.

## ARA-6xxx Wrapper and host integration

- `ARA-6000`
  Wrapper could not locate `ara-cli`.
- `ARA-6001`
  Wrapper invocation is unsupported for this host.
- `ARA-6002`
  Host API contract bridge failed.
