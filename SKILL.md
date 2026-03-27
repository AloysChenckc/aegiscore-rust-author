---
name: aegiscore-rust-author
description: Generate, normalize, and compile Rust-first blueprint packages, module language plans, and machine-readable contracts for staged projects that will later be executed by aegiscore-rust-runtime. Use when Codex must turn a product idea, an external AI-generated blueprint, or an existing repository into a normalized blueprint package and readiness bundle, but should not itself approve blueprint cleanliness or start coding.
---

# AegisCore Rust Author

Run blueprint authoring as a Rust-first, library-first runtime instead of as a free-form writing task.

## Quick Start

1. Resolve the target workspace and choose the mode:
   - `new-project`
   - `import-blueprint`
   - `update-blueprint`
   - `recompile-contract`
   - `migrate-workspace`
   - `apply-patch-plan`
   For `import-blueprint` and `update-blueprint`, prefer supplying an explicit source file instead of only a summary.
2. Read [docs/architecture.md](docs/architecture.md) and [docs/blueprint-schema.md](docs/blueprint-schema.md).
3. If importing a blueprint from another model, also read [docs/ara-normalization-rules.md](docs/ara-normalization-rules.md).
4. Read [docs/ara-module-language-policy.md](docs/ara-module-language-policy.md) before assigning implementation languages to modules.
5. Generate or normalize the blueprint package.
6. Compile the human TOML contract into machine-readable outputs.
7. Validate structure, module language assignments, readiness, and emitted workspace integrity.
8. Stop at `candidate-for-blueprint-gate` and hand the package to `aegiscore-rust-runtime`.

## Core Rules

- Keep the real runtime Rust-first:
  - `Rust` for schemas, core logic, runtime, CLI, and host API
  - `PowerShell`, `POSIX sh`, and `bat` only for thin wrappers
  - `Markdown`, `TOML`, `JSON`, and `YAML` only for assets and protocols
- Generate explicit module language assignments for every module.
- Normalize external blueprints into the local schema before compiling contracts.
- Enforce embedded `defaults/*.toml` policies at runtime instead of treating them as documentation-only hints.
- Prefer deterministic paths, stable bytes, UTF-8, and machine-readable outputs.
- Emit structured JSON errors with a stable error code when authoring fails.
- Emit structured JSON log events for major command start, success, and failure transitions.
- Do not mark a package `clean`; only emit `candidate-for-blueprint-gate`.
- Do not start coding the target project. Handoff to `aegiscore-rust-runtime` after authoring and validation.

## Modes

### `new-project`

Use when the user provides a new product idea, PRD, or architecture note and needs a full blueprint package from scratch.

### `import-blueprint`

Use when the user already has a blueprint written by another AI model or external system and needs it normalized into the local AegisCore schema.

Current runtime behavior:

- accepts `markdown`, `json`, and `toml` external blueprint sources when allowed by normalization policy
- parses structured `json` and `toml` sources into local blueprint sections before normalization
- maps richer external semantics such as acceptance criteria, constraints, assumptions, out-of-scope blocks, and phase-style stage collections into local sections
- recursively traverses nested `json` and `toml` containers and extracts inline markdown labels so semantic import is not limited to top-level headings
- absorbs multiline markdown semantic blocks after inline labels such as `Acceptance Criteria:` and `Validation Plan:` so external blueprint prose and bullet lists survive normalization as semantic evidence
- recognizes markdown phase-style stage blocks under headings such as `Phases` or `Milestones`, preserving stage order and stage-scoped details like deliverables and validation plans even when the source is not structured `json` or `toml`
- recognizes nested markdown stage headings such as `### stage-01: foundation` and stage-local headings like `#### Deliverables`, preserving those stage-scoped sections as real semantic evidence instead of flattening them into prose
- recognizes cross-source markdown stage aliases, so one source block can declare `stage-01: foundation` while later source blocks contribute `## Foundation` detail that is remapped back onto the same stage-scoped semantic truth
- recognizes stage-alias file stems such as `foundation-notes.md` or `hardening-validation-plan.md`, so stage-scoped headings and inline labels inside those files can be remapped back onto known stages even without repeating the stage heading in the file body
- recognizes document-level stage metadata such as front matter `stage: foundation`, list metadata like `- Milestone: foundation`, or inline headers like `Stage ID: stage-02` and `phase_id = "stage-02"`, so a whole source file can attach its semantic sections to a known stage without depending on file naming
- preserves stage-specific detail from structured `stages` arrays and stage source-file blocks
- emits `semantic-ir.json` as a source-first normalized IR with explicit semantic frames, semantic clusters, an explicit rendered projection, and origin labels for normalized sections so semantic truth carries both value and provenance class
- records finer semantic confidence such as `exact`, `alias`, `inline-heuristic`, `cross-source-exact`, and `path-alias-exact`, so imported evidence can be separated from inferred evidence
- detects stage-scoped semantic conflicts across indirect sources and records them as semantic risks, unresolved ambiguities, and structured `semantic_conflicts` evidence with source-group detail, confidence detail, severity, blocking state, and recommended action instead of silently union-merging conflicting stage truth
- projects semantic conflict severity, blocking state, patch risk, and recommended actions into `readiness.json`, so downstream runtimes can see gate holds without re-deriving them from raw conflict or patch evidence
- emits `decision-summary.json` as the shortest machine-consumable gate artifact, so downstream runtimes can consume conflict, patch, readiness, blocker-kind classification, primary blocker fields, stable top blocker/review slices, and scoped worktree decision truth without traversing every raw evidence file first
- records semantic hints and semantic risks in `normalization-report.json` instead of only listing dropped sections
- rejects unsupported source formats with `ARA-2000`
- rejects ambiguous stage orders without deterministic stage ids with `ARA-3002`

### `update-blueprint`

Use when an existing normalized package must be expanded, corrected, or re-layered without rebuilding everything from zero. Prefer providing `--workspace` so the runtime can read the existing blueprint package before applying update input.

Current runtime behavior:

- re-reads the existing `blueprint/authority`, `blueprint/workflow`, and `blueprint/stages` docs from the workspace
- derives workspace reread roots and default log destinations from embedded policy instead of fixed path assumptions
- merges list-like sections and stage order updates instead of rebuilding the whole package
- emits patch-level operations inside `change-report.json` and a dedicated `.codex/auto-dev/patch-plan.json` so update intent is tracked as an explicit merge plan, not only as a final merged result
- emits `.codex/auto-dev/patch-base.json` so update replay has a durable normalized base snapshot instead of relying only on live workspace rereads
- emits `.codex/auto-dev/patch-execution-report.json` so patch replay becomes a durable machine artifact instead of a transient validation step
- hydrates patch operations with replayable values at build time so standalone patch apply is not dependent on a later workspace reread step
- classifies patch strategies with explicit metadata such as `risk_level`, `review_required`, and `apply_mode` so update intent is easier to audit and gate
- captures reverse patch metadata and proves reverse replay back to the emitted patch base, so patch execution is validated as reversible instead of only forward-replayable
- replays patch strategies against the workspace blueprint source and rejects update flows whose patch plan cannot reproduce the merged result
- stops with `ARA-3000` if the update would rewrite authority truth such as `Purpose`
- keeps stage document paths deterministic and disambiguates collisions when multiple stage names would map to the same slug
- rejects unsupported external source formats and ambiguous stage orders when normalization policy forbids them

### `recompile-contract`

Use when the blueprint docs already exist and only the machine-readable contract bundle must be refreshed. Prefer providing `--workspace` so the runtime can rebuild from the existing `blueprint/` docs instead of falling back to loose summaries.

Current runtime behavior:

- prefers workspace blueprint docs over loose summaries
- rebuilds stage order from the existing workflow and stage docs
- preserves all discovered stages when regenerating `resolved-contract.json`
- falls back to stage document metadata when workflow stage order is missing

### `migrate-workspace`

Use when an existing workspace package still parses but carries drifted machine artifact schema versions that must be rewritten to the current runtime schema.

Current runtime behavior:

- reloads the existing workspace bundle without first trusting its schema versions
- rewrites supported machine artifacts to the current runtime schema versions
- emits `.codex/auto-dev/migration-report.json` with per-artifact migration status
- re-emits and revalidates the workspace package after migration before reporting success

## Workspace Validation

Use `validate-workspace` when a package has already been emitted and must be reloaded from disk before handoff or audit.

Current runtime behavior:

- reloads authority, workflow, stage, module, manifest, normalization, readiness, and contract artifacts from the workspace
- reloads and revalidates `worktree-protocol.json` as part of the workspace consistency set
- rejects invalid, duplicate, or hierarchically overlapping worktree branch patterns, overlapping exclusive paths, empty scoped worktree roles, duplicate explicit stage/module owners, stage-isolated roles that span multiple stages, empty global/shared worktree rules, placeholder worktree rules like `todo` or `tbd`, and non-actionable worktree rules before a package can validate cleanly
- derives `patch-base.json` from current workspace state when validating an older workspace that predates the dedicated patch-base artifact
- derives `patch-plan.json` from `change-report.json` when validating an older workspace that predates the dedicated patch-plan artifact
- derives `patch-execution-report.json` from `patch-plan.json` when validating an older workspace that predates the dedicated replay report artifact
- derives `semantic-ir.json` from reconstructed normalized source plus emitted blueprint docs when validating an older workspace that predates the dedicated semantic IR artifact
- revalidates schema versions for emitted machine artifacts before trusting the workspace bundle
- revalidates `module-catalog.json` against the embedded runtime module set and `defaults/module-language-policy.toml`
- revalidates `patch-base.json` against `patch-plan.json` so stored patch base truth cannot silently drift
- revalidates `patch-plan.json` against `change-report.json` so tampered patch intent cannot silently pass workspace validation
- revalidates patch operation `affected_paths` and `target_worktree_roles` against the emitted worktree protocol and stage document paths
- revalidates `patch-execution-report.json` against `patch-plan.json` so tampered replay evidence cannot silently pass workspace validation
- revalidates `semantic-ir.json` against both reconstructed normalized source truth and the rendered blueprint docs
- revalidates `agent-brief.json` against `decision-summary.json`, `readiness.json`, and `task-progress.json` so short handoff truth cannot silently drift
- revalidates `project-contract.toml`, workflow stage order, workflow worktree protocol, stage docs, and `resolved-contract.json` as one consistency set
- revalidates manifest fingerprints against current on-disk content
- revalidates readiness fingerprints against the reconstructed package
- emits `ARA-2003` for contract drift, `ARA-2005` for schema version drift, `ARA-5003` for tampered patch/change artifacts, `ARA-5002` for tampered manifest artifacts, and `ARA-4002` for stale readiness fingerprints

## Standalone Patch Replay

Use `apply-patch-plan` when an emitted update package already contains a durable `.codex/auto-dev/patch-base.json` and `.codex/auto-dev/patch-plan.json`, and the runtime must independently replay patch intent without rebuilding the whole package from source inputs.

Current runtime behavior:

- loads the emitted `patch-base.json` as the standalone normalized base snapshot
- refuses legacy workspaces that do not contain an emitted patch base artifact, instead of silently replaying from the current result state
- replays `patch-plan.json` against that stored base snapshot
- requires patch replay scope to remain inside the emitted worktree protocol before standalone apply succeeds
- requires standalone replay evidence to remain reversible back to the same emitted patch base
- rewrites `.codex/auto-dev/patch-execution-report.json` and refreshes `readiness.json` from the replayed result
- rewrites `.codex/auto-dev/decision-summary.json` and `.codex/auto-dev/agent-brief.json` alongside `readiness.json` so standalone replay keeps both gate truth and short handoff truth in sync
- stops with `ARA-3003` if the workspace cannot provide a durable patch base for standalone apply

## Required Outputs

Authoring should converge on these durable artifacts in the target workspace:

- `blueprint/authority/`
- `blueprint/workflow/`
- `blueprint/stages/`
- `blueprint/modules/00-module-catalog.json`
- `.codex/auto-dev/project-contract.toml`
- `.codex/auto-dev/resolved-contract.json`
- `.codex/auto-dev/worktree-protocol.json`
- `.codex/auto-dev/blueprint-manifest.json`
- `.codex/auto-dev/semantic-ir.json`
- `.codex/auto-dev/normalization-report.json`
- `.codex/auto-dev/change-report.json`
- `.codex/auto-dev/patch-base.json`
- `.codex/auto-dev/patch-plan.json`
- `.codex/auto-dev/patch-execution-report.json`
- `.codex/auto-dev/decision-summary.json`
- `.codex/auto-dev/agent-brief.json`
- `.codex/auto-dev/readiness.json`
- `.codex/auto-dev/task-progress.json`
- `.codex/auto-dev/author-report.md`

When workflow worktree roles assign `module_ids`, the emitted `module-catalog.json` must also record deterministic `preferred_worktree_role` and `allowed_worktree_roles` bindings for those modules.
Worktree roles must declare at least one deterministic scope through `stage_ids`, `module_ids`, or `exclusive_paths`, explicit `module_ids` ownership cannot be duplicated across roles, `stage-isolated-worktree` roles must bind to exactly one stage, `module-isolated-worktree` roles must declare explicit module ownership, and worktree rule language (`parallel_worktree_policy`, `sync_rule`, `merge_back_rule`, `cleanup_rule`) must remain consistent with the declared worktree model.
If `shared_authority_paths` are declared, `parallel_worktree_policy`, `sync_rule`, `merge_back_rule`, and `cleanup_rule` must also explicitly describe how shared authority or workflow changes are coordinated, not just how isolated stage/module work proceeds.

Optional operational output when migration runs:

- `.codex/auto-dev/migration-report.json`

## Stop Conditions

Stop authoring immediately if any of these are true:

- the package cannot be normalized without changing authority truth
- a module language cannot be assigned without violating policy
- contract structure and blueprint structure disagree
- the stage graph is ambiguous or cyclic
- the external blueprint is too incomplete to normalize deterministically
- required machine-readable outputs cannot be emitted

When stopped, output:

1. blocker
2. affected file or module
3. why authoring cannot continue safely
4. smallest repair path

## Resource Map

- `docs/architecture.md`
  Canonical runtime architecture and crate map.
- `docs/blueprint-schema.md`
  Required blueprint sections and machine outputs.
- `docs/contract-mapping.md`
  How blueprint docs map into TOML and JSON contract fields.
- `docs/readiness-states.md`
  Readiness lifecycle and handoff boundary.
- `docs/ara-error-codes.md`
  Error code catalog.
- `docs/ara-logging-schema.md`
  Human-readable logging contract.
- `docs/ara-logging-schema.json`
  Machine-readable logging schema.
- `docs/ara-module-language-policy.md`
  Module language assignment rules.
- `docs/ara-normalization-rules.md`
  Import and normalization rules.
- `docs/final-capability-summary.md`
  Final strict summary of the added closure capabilities and what is runtime-enforced.
- `defaults/`
  Default TOML policies and contract templates.
- `templates/`
  Markdown and JSON templates for generated assets.
- `wrappers/`
  Optional `pwsh`, `sh`, and `bat` launchers.
- `scripts/validate-skill-package.ps1`
  Structural validator for the skill package itself.
- `scripts/run-self-check.ps1`
  End-to-end package validation helper, including workspace revalidation and tamper checks.
