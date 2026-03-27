# AegisCore Rust Author Architecture

## Runtime identity

`aegiscore-rust-author` is a Rust-first, cross-platform, library-first blueprint authoring runtime.

The runtime owns:

- blueprint generation
- external blueprint normalization
- semantic hint and semantic risk reporting during normalization
- workspace blueprint re-ingestion for update and recompile flows
- emitted workspace package revalidation
- supported machine-artifact schema migration for existing workspace packages
- worktree-aware blueprint compilation and validation for staged workspace isolation
- enforcement of embedded blueprint and normalization policy defaults
- module language planning
- contract compilation
- readiness evaluation

The runtime does not own:

- blueprint gate approval
- coding execution
- finalize or stage advance

Those responsibilities belong to `aegiscore-rust-runtime`.

## Runtime layers

### `ara-schemas`

Owns:

- schema version constants
- error code types
- readiness states
- module language enums
- machine-readable response envelopes

Language: `Rust`

### `ara-core`

Owns:

- input classification
- blueprint IR
- authority, workflow, and stage planning
- worktree protocol planning across stage roles, branch patterns, and exclusive path scopes
- module catalog generation
- module language planning
- source-first semantic IR generation with normalized-section origin labeling, explicit semantic frames, semantic clusters, a rendered projection view, and semantic IR reload fallback
- recursive semantic extraction from nested `json`/`toml` containers and inline markdown labels before normalization
- change-report generation for update and normalization audit
- patch-level update operation generation
- dedicated patch-base snapshot generation and legacy fallback handling
- dedicated patch-plan generation, reload fallback, and anti-tamper validation
- strategy-based patch-plan replay against stored patch-base snapshots with reverse replay proof back to the same base
- dedicated patch-execution-report generation, reload fallback, and anti-tamper validation
- standalone `apply-patch-plan` execution for emitted update packages
- worktree-aware patch scope derivation so patch operations declare affected repo paths and any role-scoped ownership implied by the current worktree protocol
- contract compilation
- emitted package reload and anti-tamper validation
- readiness evaluation

Language: `Rust`

### `ara-runtime`

Owns:

- filesystem helpers
- repo-relative path normalization
- stable UTF-8 writes
- atomic writes
- hash and fingerprint utilities
- platform-neutral path behavior

Language: `Rust`

### `ara-cli`

Owns:

- command-line interface
- subcommand parsing
- machine-readable stdout
- consistent error handling

Language: `Rust`

### `ara-host-api`

Owns:

- embeddable Rust API surface for other Rust platforms
- host integration without shell wrappers
- emitted workspace package loading and validation

Language: `Rust`

## Non-core layers

### Wrapper layer

Files:

- `wrappers/ara.ps1`
- `wrappers/ara.sh`
- `wrappers/ara.bat`

Languages:

- `PowerShell`
- `POSIX sh`
- `Batch`

Wrapper rules:

- do not parse blueprint truth
- do not implement business logic
- do not modify contract semantics
- only forward arguments and process exit status

### Content and protocol layer

Formats:

- `Markdown`
- `TOML`
- `JSON`
- `YAML`

Responsibilities:

- human-readable blueprint docs
- defaults and author-maintained policy
- machine-readable reports and state
- skill metadata

## Validation strategy

Validation should happen in this order:

1. skill package structure
2. TOML and JSON parseability
3. Rust workspace compilation
4. contract shape and module language consistency
5. emitted workspace package reload and anti-tamper validation
6. cross-file contract alignment across TOML, workflow, stages, worktree protocol, and resolved JSON
7. schema migration and post-migration workspace revalidation
8. readiness handoff integrity

`scripts/run-self-check.ps1` reports two terminal states:

- `ok`
  Compilation, runtime CLI checks, workspace revalidation, and tamper checks all completed.
- `blocked`
  The host allowed structural and compile validation, but blocked runtime CLI execution even after runtime-mirror fallback. This is an environment block, not automatically a Rust logic failure.
