# ARA Logging Schema

Structured logs should be optimized for auditability and automation, not for chain-of-thought capture.

## Required fields

- `schema_version`
- `event_name`
- `level`
- `correlation_id`
- `workspace`
- `stage`
- `loop_id`
- `command`
- `decision_code`
- `error_code`
- `report_path`
- `fingerprint`
- `message`
- `timestamp_utc`

## Rules

- use UTF-8 JSON output
- do not log hidden reasoning or chain-of-thought
- keep `error_code` empty for success events
- keep `decision_code` concrete for major state transitions
- use repo-relative paths with `/` separators when possible
