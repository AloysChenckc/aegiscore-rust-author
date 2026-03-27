# Authority Root

## Purpose

Describe the product truth that all downstream workflow and stage docs must obey.

## Authority Scope

- Product intent
- Hard constraints
- Non-goals
- Governance boundaries

## Truth Rules

- This document outranks workflow and stage docs when intent conflicts exist.
- Downstream docs may refine execution details but must not rewrite authority truth.
- Contract compilation must preserve the canonical identifiers declared here.

## Conflict Resolution

If a workflow or stage document contradicts this file, stop authoring and record a blocker in `author-report.md`.

## Non-Goals

List capabilities, modules, or timelines that are explicitly out of scope.
