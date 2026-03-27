param(
    [string]$SkillRoot = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = "Stop"

$requiredPaths = @(
    "SKILL.md",
    "agents/openai.yaml",
    "Cargo.toml",
    "docs/architecture.md",
    "docs/blueprint-schema.md",
    "docs/contract-mapping.md",
    "docs/readiness-states.md",
    "docs/ara-error-codes.md",
    "docs/ara-logging-schema.md",
    "docs/ara-logging-schema.json",
    "docs/ara-module-language-policy.md",
    "docs/ara-normalization-rules.md",
    "templates/authority-root.md",
    "templates/workflow-overview.md",
    "templates/stage-doc.md",
    "templates/module-language-policy.md",
    "templates/author-report.md",
    "defaults/blueprint-policy.toml",
    "defaults/normalization-policy.toml",
    "defaults/module-language-policy.toml",
    "defaults/project-contract.toml",
    "wrappers/ara.ps1",
    "wrappers/ara.sh",
    "wrappers/ara.bat",
    "scripts/run-self-check.ps1",
    "crates/ara-schemas/Cargo.toml",
    "crates/ara-core/Cargo.toml",
    "crates/ara-runtime/Cargo.toml",
    "crates/ara-cli/Cargo.toml",
    "crates/ara-host-api/Cargo.toml"
)

$missing = @()
foreach ($path in $requiredPaths) {
    $fullPath = Join-Path $SkillRoot $path
    if (-not (Test-Path $fullPath)) {
        $missing += $path
    }
}

if ($missing.Count -gt 0) {
    throw ("Missing required skill paths: " + ($missing -join ", "))
}

$loggingSchemaPath = Join-Path $SkillRoot "docs/ara-logging-schema.json"
$null = Get-Content -Raw $loggingSchemaPath | ConvertFrom-Json

$cargoToml = Get-Content -Raw (Join-Path $SkillRoot "Cargo.toml")
$requiredMembers = @(
    'crates/ara-schemas',
    'crates/ara-core',
    'crates/ara-runtime',
    'crates/ara-cli',
    'crates/ara-host-api'
)

$missingMembers = @()
foreach ($member in $requiredMembers) {
    if ($cargoToml -notmatch [regex]::Escape($member)) {
        $missingMembers += $member
    }
}

if ($missingMembers.Count -gt 0) {
    throw ("Cargo workspace is missing members: " + ($missingMembers -join ", "))
}

[pscustomobject]@{
    skill_root = $SkillRoot
    required_file_count = $requiredPaths.Count
    logging_schema_valid = $true
    workspace_members_valid = $true
    status = "ok"
} | ConvertTo-Json -Depth 5
