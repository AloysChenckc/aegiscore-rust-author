# ARA Module Language Policy

Module language planning is part of blueprint truth.

## Language assignments

### Core truth modules

Layers:

- `schemas`
- `core`
- `runtime`
- `validation`
- `contract`
- `host-api`

Rules:

- `recommended_language = rust`
- `allowed_languages = ["rust"]`
- `forbidden_languages = ["powershell", "sh", "bat", "python", "nodejs"]`
- emitted `module-catalog.json` must keep the embedded runtime module ids and per-layer language sets exactly aligned with `defaults/module-language-policy.toml`

### Wrapper modules

Layers:

- `wrapper`
- `launcher`

Rules:

- Windows wrapper: `powershell`
- Unix wrapper: `sh`
- Windows fallback wrapper: `bat`
- emitted wrapper module specs must remain aligned with the embedded wrapper layers and may not be rewritten into runtime-bearing modules

Wrappers must not own business truth.

### Human docs

Rules:

- use `markdown`

### Static defaults and policy

Rules:

- use `toml`

### Machine state and reports

Rules:

- use `json`
