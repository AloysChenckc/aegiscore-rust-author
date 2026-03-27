# Contract Mapping

This document explains how human-authored blueprint docs map into machine-readable contract outputs.

## Human truth

Human-edited truth should live in:

- `blueprint/authority/*.md`
- `blueprint/workflow/*.md`
- `blueprint/stages/*.md`
- `blueprint/modules/00-module-catalog.json`
- `.codex/auto-dev/project-contract.toml`

## Machine truth

Machine-consumable truth should live in:

- `.codex/auto-dev/resolved-contract.json`
- `.codex/auto-dev/worktree-protocol.json`
- `.codex/auto-dev/blueprint-manifest.json`
- `.codex/auto-dev/normalization-report.json`
- `.codex/auto-dev/readiness.json`

## Mapping rules

- authority selectors in the resolved contract must point to deterministic files beneath `blueprint/`
- stage order in the contract must agree with the workflow overview and every stage doc
- worktree protocol in the workflow overview must agree with `project-contract.toml`, `resolved-contract.json`, and `worktree-protocol.json`
- worktree roles should identify deterministic branch patterns, stage ownership, optional module ownership, and exclusive path scopes
- module language assignments must agree with `module-language-policy.toml`
- readiness must never claim `candidate-for-blueprint-gate` unless the TOML contract and resolved JSON contract agree on stage graph and output paths
- normalization reports should preserve import auditability by recording source files, inferred sections, dropped sections, and unresolved ambiguities

## Recommended split

- `project-contract.toml`
  Human-maintained defaults and project-specific settings
- `resolved-contract.json`
  Compiler output for downstream runtime consumption
- `worktree-protocol.json`
  Compiler output for worktree-aware execution planning and validation
