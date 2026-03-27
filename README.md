# aegiscore-rust-author

A Rust-first blueprint authoring runtime for AegisCore.

`aegiscore-rust-author` is an open-source, Rust-first authoring runtime that turns product ideas, external AI-generated blueprints, and existing project context into validated, machine-readable blueprint packages.

Instead of stopping at prose docs, it compiles blueprint truth into a durable artifact set that can be reloaded, audited, migrated, replayed, and handed off to downstream execution systems such as `aegiscore-rust-runtime`.

---

## Why

Many AI-assisted development workflows can generate plans, specs, or notes, but they often break down when those outputs need to become stable engineering inputs.

Typical problems include:

- blueprint docs drifting away from actual project truth
- unclear stage boundaries
- missing module ownership and language constraints
- inconsistent formats across different AI-generated blueprints
- weak handoff quality for downstream runtimes or agents

`aegiscore-rust-author` addresses that gap by moving blueprint authoring from "helpful text generation" to "validated engineering protocol generation".

---

## What It Does

`aegiscore-rust-author` can:

- generate staged blueprint packages from ideas, PRDs, and architecture notes
- import and normalize external blueprints from `markdown`, `json`, and `toml`
- plan modules and assign implementation languages explicitly
- compile human-facing blueprint docs into machine-readable contracts
- build source-first semantic IR for normalization and validation
- emit patch plans, patch bases, replay reports, and decision artifacts
- model worktree-aware development flow as machine-readable protocol
- reload emitted workspace bundles and revalidate them against drift or tampering
- produce short machine-consumable handoff artifacts for downstream runtimes and agents

---

## Core Positioning

This project is not a freeform "AI planner" and not a universal document interpreter.

Its strength is:

- strong governance inside a defined domain
- high-trust normalization
- machine-readable truth
- deterministic validation and replay
- strict worktree-aware staged workflow modeling

It is especially well suited for:

- blueprint-driven development
- staged AI agent execution
- contract-first engineering workflows
- module-aware and worktree-aware project planning
- systems that need a strict separation between authoring and execution

---

## Runtime Boundary

`aegiscore-rust-author` owns:

- blueprint generation
- blueprint normalization
- semantic extraction
- module planning
- implementation language planning
- contract compilation
- readiness and decision artifacts
- workspace revalidation
- schema migration

It does **not** own:

- blueprint gate approval
- coding execution
- finalize or stage-advance execution
- `git worktree` lifecycle management

Those runtime-execution responsibilities belong to `aegiscore-rust-runtime`.

---

## Installation

### Requirements

- Rust toolchain
- Cargo
- Windows, macOS, or Linux

Recommended:

- a dedicated workspace directory for emitted blueprint bundles
- a downstream runtime such as `aegiscore-rust-runtime`

### Build from source

```powershell
git clone https://github.com/AloysChenckc/aegiscore-rust-author.git
cd aegiscore-rust-author
cargo check --workspace
```

### Validate the package itself

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\validate-skill-package.ps1
```

### Run the end-to-end self-check

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-self-check.ps1
```

---

## Quick Start

### 1. Choose a workspace

Create or select a target workspace where the blueprint package will be emitted.

Example:

```powershell
mkdir my-agent-project
cd my-agent-project
```

### 2. Prepare input

You can start from:

- a product idea
- a PRD or architecture note
- an external blueprint in `markdown`, `json`, or `toml`
- an existing workspace that already contains `blueprint/` docs

### 3. Run the authoring runtime

Typical modes are:

- `new-project`
- `import-blueprint`
- `update-blueprint`
- `recompile-contract`
- `validate-workspace`
- `migrate-workspace`

Example: import an external blueprint

```powershell
cargo run -p ara-cli -- emit `
  --workspace C:\Path\To\Workspace `
  --project-name my-agent-project `
  --source-summary "AI agent project for structured local control" `
  --source-file C:\Path\To\external-blueprint.md `
  --mode import-blueprint
```

### 4. Revalidate the emitted package

Always revalidate the workspace after emission:

```powershell
cargo run -p ara-cli -- validate-workspace `
  --workspace C:\Path\To\Workspace
```

### 5. Inspect the generated artifacts

Important outputs will be under:

- `blueprint/`
- `.codex/auto-dev/`

Especially:

- `resolved-contract.json`
- `semantic-ir.json`
- `normalization-report.json`
- `worktree-protocol.json`
- `readiness.json`
- `decision-summary.json`
- `agent-brief.json`

### 6. Handoff to runtime

Once the package validates successfully and reaches `candidate-for-blueprint-gate`, hand it off to `aegiscore-rust-runtime`.

---

## CLI Commands

The main command surface lives in `ara-cli`.

### Emit a package

```powershell
cargo run -p ara-cli -- emit --workspace C:\Path\To\Workspace --project-name my-project --source-summary "Project summary" --mode new-project
```

### Import an external blueprint

```powershell
cargo run -p ara-cli -- emit --workspace C:\Path\To\Workspace --project-name my-project --source-summary "Imported blueprint" --source-file C:\Path\To\blueprint.md --mode import-blueprint
```

### Update an existing package

```powershell
cargo run -p ara-cli -- update --workspace C:\Path\To\Workspace --project-name my-project --source-summary "Update summary" --source-file C:\Path\To\update.md
```

### Recompile the contract

```powershell
cargo run -p ara-cli -- recompile --workspace C:\Path\To\Workspace --project-name my-project --source-summary "Recompile contract"
```

### Validate an emitted workspace

```powershell
cargo run -p ara-cli -- validate-workspace --workspace C:\Path\To\Workspace
```

### Apply a patch plan

```powershell
cargo run -p ara-cli -- apply-patch-plan --workspace C:\Path\To\Workspace
```

### Migrate an older workspace

```powershell
cargo run -p ara-cli -- migrate-workspace --workspace C:\Path\To\Workspace
```

---

## Example Workflow

Here is the recommended flow when using `aegiscore-rust-author` as the authoring layer in a staged AI-development system:

1. Prepare a blueprint source.
   This can be an idea note, an external AI blueprint, or architecture guidance.

2. Emit a normalized package.
   Use `emit` with `new-project` or `import-blueprint`.

3. Validate the package from disk.
   Run `validate-workspace` so emitted truth is reloaded and rechecked.

4. Inspect the gate artifacts.
   Review:
   - `readiness.json`
   - `decision-summary.json`
   - `agent-brief.json`

5. Hand off to execution.
   Pass the validated package to `aegiscore-rust-runtime`.

6. If the blueprint changes later:
   - run `update`
   - inspect `patch-plan.json` and `patch-execution-report.json`
   - revalidate the workspace

7. If schema drift occurs:
   - run `migrate-workspace`
   - revalidate after migration

---

## Project Structure

```text
aegiscore-rust-author/
|- crates/
|  |- ara-schemas/
|  |- ara-core/
|  |- ara-runtime/
|  |- ara-cli/
|  `- ara-host-api/
|- defaults/
|- docs/
|- exports/
|- scripts/
|- templates/
|- wrappers/
|- SKILL.md
`- Cargo.toml
```

### Key directories

- `crates/ara-schemas/`
  Schema definitions, error codes, machine-readable structs, and schema versions.

- `crates/ara-core/`
  Core authoring logic: normalization, semantic IR, module planning, contract compilation, patch planning, worktree protocol, readiness, and validation.

- `crates/ara-runtime/`
  Filesystem, fingerprinting, path normalization, stable writes, and runtime helpers.

- `crates/ara-cli/`
  Command-line entry point for authoring, validation, migration, and patch replay.

- `crates/ara-host-api/`
  Library-first API for embedding the authoring runtime into other Rust hosts.

- `defaults/`
  Embedded TOML policies and default contract settings enforced by the runtime.

- `docs/`
  Architecture, schema, normalization rules, language policy, readiness model, and capability docs.

- `scripts/`
  Skill validation and end-to-end self-check scripts.

- `templates/`
  Blueprint and machine-output templates used by the runtime.

- `wrappers/`
  Thin launchers for PowerShell, POSIX shell, and batch environments.

### Emitted workspace structure

A generated workspace typically looks like this:

```text
<workspace>/
|- blueprint/
|  |- authority/
|  |- workflow/
|  |- stages/
|  `- modules/
`- .codex/
   `- auto-dev/
      |- project-contract.toml
      |- resolved-contract.json
      |- semantic-ir.json
      |- normalization-report.json
      |- patch-plan.json
      |- worktree-protocol.json
      |- readiness.json
      |- decision-summary.json
      `- agent-brief.json
```

---

## Main Capabilities

### Blueprint generation

Generate staged blueprint packages from ideas, PRDs, and architecture notes.

### External blueprint normalization

Import `markdown`, `json`, and `toml` blueprints and normalize them into the local schema.

### Semantic IR

Build a source-first semantic IR that preserves:

- normalized source truth
- rendered projection
- semantic frames
- semantic clusters
- semantic conflicts
- ambiguity and risk signals

### Module planning

Generate module-level truth including:

- responsibility
- layer
- language assignment
- artifact ownership
- worktree-role binding

### Patch and update runtime

Support:

- patch-base
- patch-plan
- patch replay
- reverse replay proof
- patch execution reporting

### Worktree-aware protocol

Support:

- worktree model
- role ownership
- branch patterns
- exclusive paths
- sync / merge / cleanup rules
- worktree-aware patch scope validation

### Workspace revalidation

Reload emitted workspaces and validate:

- schema version drift
- contract inconsistency
- manifest tampering
- readiness drift
- semantic drift
- patch drift
- worktree protocol drift

---

## Output Artifacts

`aegiscore-rust-author` does not stop at Markdown docs. It emits a durable artifact set such as:

- `blueprint/authority/`
- `blueprint/workflow/`
- `blueprint/stages/`
- `blueprint/modules/00-module-catalog.json`
- `.codex/auto-dev/project-contract.toml`
- `.codex/auto-dev/resolved-contract.json`
- `.codex/auto-dev/semantic-ir.json`
- `.codex/auto-dev/normalization-report.json`
- `.codex/auto-dev/change-report.json`
- `.codex/auto-dev/patch-base.json`
- `.codex/auto-dev/patch-plan.json`
- `.codex/auto-dev/patch-execution-report.json`
- `.codex/auto-dev/worktree-protocol.json`
- `.codex/auto-dev/readiness.json`
- `.codex/auto-dev/decision-summary.json`
- `.codex/auto-dev/agent-brief.json`
- `.codex/auto-dev/task-progress.json`

Together, these artifacts form a machine-readable truth chain that downstream runtimes and agents can consume safely.

---

## Why Rust

This project uses Rust because it carries truth-layer logic, not just scripting glue.

Rust is used here for:

- strong typed schema and contract modeling
- stable machine artifact generation
- reliable validation, replay, and migration logic
- better cross-platform behavior
- a clean library-first plus CLI-first runtime shape

Language split:

- Rust: schemas, core, runtime, CLI, host API
- PowerShell / sh / bat: thin wrappers only
- Markdown / TOML / JSON / YAML: content and protocol assets

---

## Who This Is For

This project is a good fit if you are building:

- blueprint-driven development systems
- staged AI-agent workflows
- contract-first engineering pipelines
- worktree-aware planning flows
- authoring layers that must feed reliable inputs to downstream execution runtimes

---

## What This Project Is Not

`aegiscore-rust-author` is **not**:

- a general-purpose freeform AI planner
- a universal compiler for arbitrary document styles
- a `git worktree` manager
- a coding runtime
- a replacement for downstream execution engines

It is best understood as:

**a high-trust authoring runtime that turns blueprint intent into engineering protocol.**

---

## Design Philosophy

This project values:

- determinism over improvisation
- machine truth over prose-only output
- replayability over hidden merge behavior
- validation over optimistic guessing
- explicit protocol over loose interpretation

In short, it prefers systems that are explainable, replayable, and hard to silently drift.

---

## Open Source Value

The open-source value of this project is not simply "another AI tool".

It provides a concrete path for moving AI-assisted development from:

- prompt-first

to:

- blueprint-first
- contract-first
- machine-verifiable authoring

If you are exploring:

- AI software architecture authoring
- blueprint-driven development
- machine-verifiable planning
- worktree-aware agent workflows
- governance-first agent engineering

then `aegiscore-rust-author` can serve as a strong reference point.

---

## Status

The project already operates as a complete authoring runtime with:

- blueprint generation
- normalization
- contract compilation
- semantic IR
- patch and replay evidence
- worktree protocol
- readiness, decision, and handoff artifacts
- workspace validation and migration

Its current position is clear:

**a high-trust AegisCore authoring runtime, not an infinitely general platform.**

---

## License

This project is released under the MIT License.

See [LICENSE](LICENSE) for details.

---

## Related Project

- `aegiscore-rust-runtime`
  The downstream execution runtime that consumes blueprint packages and machine-readable contracts emitted by `aegiscore-rust-author`.

---

## One-Line Summary

`aegiscore-rust-author` turns ideas, external blueprints, and project context into validated, auditable, machine-readable blueprint packages for staged AI-driven development.
