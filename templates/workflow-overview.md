# Workflow Overview

## Stage Order

List every stage in deterministic order with a stable `stage_id`.

## Entry Rule

Define what must be true before the first stage can start.

## Exit Gate

Define the evidence bundle, review bundle, and readiness state required before stage advance.

## Cross-Stage Split Rule

Describe how to split work that spans multiple stages without allowing stage pollution.

## Stop Conditions

List the conditions that must block authoring or contract compilation.
