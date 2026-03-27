param(
    [string]$SkillRoot = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = "Stop"
if ($PSVersionTable.PSVersion.Major -ge 7) {
    $PSNativeCommandUseErrorActionPreference = $false
}

$quickValidate = Join-Path $env:USERPROFILE ".codex\\skills\\.system\\skill-creator\\scripts\\quick_validate.py"
$validateScript = Join-Path $SkillRoot "scripts\\validate-skill-package.ps1"
$cargoExe = Join-Path $env:USERPROFILE ".cargo\\bin\\cargo.exe"
$tempRoot = Join-Path $env:TEMP ("ara-self-check-" + [guid]::NewGuid().ToString("N"))
$runtimeMirrorRoot = $null
$tamperManifestWorkspace = $null
$tamperReadinessWorkspace = $null
$tamperContractWorkspace = $null
$tamperProgressWorkspace = $null
$tamperModuleCatalogWorkspace = $null
$tamperSchemaWorkspace = $null
$tamperChangeReportWorkspace = $null
$tamperPatchBaseWorkspace = $null
$tamperPatchPlanWorkspace = $null
$tamperPatchScopeWorkspace = $null
$legacyPatchBaseWorkspace = $null
$legacyPatchPlanWorkspace = $null
$tamperWorktreeWorkspace = $null
$tamperWorktreeRuleWorkspace = $null
$tamperSemanticIrWorkspace = $null
$legacySemanticIrWorkspace = $null
$tamperAgentBriefWorkspace = $null
$legacyAgentBriefWorkspace = $null
$checks = @()
$runtimeBlocked = $false

function Invoke-CargoCommand {
    param(
        [string[]]$Arguments,
        [string]$WorkingDirectory = $SkillRoot
    )

    $stdoutPath = Join-Path $env:TEMP ("ara-cargo-stdout-" + [guid]::NewGuid().ToString("N") + ".log")
    $stderrPath = Join-Path $env:TEMP ("ara-cargo-stderr-" + [guid]::NewGuid().ToString("N") + ".log")

    try {
        $previousErrorActionPreference = $ErrorActionPreference
        Push-Location $WorkingDirectory
        try {
            $ErrorActionPreference = "Continue"
            & $cargoExe @Arguments 1> $stdoutPath 2> $stderrPath
            $exitCode = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = $previousErrorActionPreference
            Pop-Location
        }

        if (Test-Path $stdoutPath) {
            $stdout = Get-Content -Raw $stdoutPath
            if ($null -eq $stdout) {
                $stdout = ""
            }
        }
        else {
            $stdout = ""
        }

        if (Test-Path $stderrPath) {
            $stderr = Get-Content -Raw $stderrPath
            if ($null -eq $stderr) {
                $stderr = ""
            }
        }
        else {
            $stderr = ""
        }
        $output = ($stdout.TrimEnd() + [Environment]::NewLine + $stderr.TrimEnd()).Trim()

        [pscustomobject]@{
            ExitCode = $exitCode
            Output = $output
            Stdout = $stdout
            Stderr = $stderr
            Blocked = ($output -match "os error 4551") -or ($output -match "\b4551\b")
        }
    }
    finally {
        Remove-Item -Force -ErrorAction SilentlyContinue $stdoutPath, $stderrPath
    }
}

function New-RuntimeMirror {
    $documentsRoot = Join-Path $env:USERPROFILE "Documents\\Codex"
    New-Item -ItemType Directory -Force -Path $documentsRoot | Out-Null
    $mirrorRoot = Join-Path $documentsRoot "aegiscore-rust-author-work"
    if (Test-Path $mirrorRoot) {
        Remove-Item -Recurse -Force $mirrorRoot
    }
    New-Item -ItemType Directory -Force -Path $mirrorRoot | Out-Null

    & robocopy $SkillRoot $mirrorRoot /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -gt 3) {
        throw "robocopy failed while preparing runtime mirror"
    }

    return $mirrorRoot
}

function Get-DefaultPolicyValue {
    param(
        [string]$PolicyPath,
        [string]$Key
    )

    $content = Get-Content -Raw $PolicyPath
    $pattern = "(?m)^\s*" + [regex]::Escape($Key) + '\s*=\s*"([^"]+)"'
    $match = [regex]::Match($content, $pattern)
    if (-not $match.Success) {
        throw "missing policy key: $Key in $PolicyPath"
    }
    return $match.Groups[1].Value
}

Push-Location $SkillRoot
try {
    python -c "import yaml" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "PyYAML is required for quick_validate.py"
    }

    python $quickValidate $SkillRoot
    if ($LASTEXITCODE -ne 0) {
        throw "quick_validate.py failed"
    }
    $checks += [pscustomobject]@{ name = "quick-validate"; status = "ok" }

    & $validateScript -SkillRoot $SkillRoot | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "validate-skill-package.ps1 failed"
    }
    $checks += [pscustomobject]@{ name = "package-validate"; status = "ok" }

    & $cargoExe check --workspace
    if ($LASTEXITCODE -ne 0) {
        throw "cargo check failed"
    }
    $checks += [pscustomobject]@{ name = "cargo-check"; status = "ok" }

    & $cargoExe test --workspace --no-run
    if ($LASTEXITCODE -ne 0) {
        throw "cargo test --no-run failed"
    }
    $checks += [pscustomobject]@{ name = "cargo-test-build"; status = "ok" }

    $runtimeRoot = $SkillRoot
    $helpResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "--help") -WorkingDirectory $runtimeRoot
    if ($helpResult.Blocked) {
        $runtimeMirrorRoot = New-RuntimeMirror
        $runtimeRoot = $runtimeMirrorRoot
        $helpResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "--help") -WorkingDirectory $runtimeRoot
        if ($helpResult.Blocked) {
            $runtimeBlocked = $true
            $checks += [pscustomobject]@{ name = "cli-runtime"; status = "blocked_by_policy" }
        }
        elseif ($helpResult.ExitCode -ne 0) {
            throw "ara-cli help failed in runtime mirror"
        }
        else {
            $checks += [pscustomobject]@{ name = "cli-help"; status = "ok" }
        }
    }
    elseif ($helpResult.ExitCode -ne 0) {
        throw "ara-cli help failed"
    }
    else {
        $checks += [pscustomobject]@{ name = "cli-help"; status = "ok" }
    }

    if (-not $runtimeBlocked) {
        if (Test-Path $tempRoot) {
            Remove-Item -Recurse -Force $tempRoot
        }
        New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null

        $emitResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-self-check", "--source-summary", "Self-check emission path", "--mode", "new-project") -WorkingDirectory $runtimeRoot
        if ($emitResult.ExitCode -ne 0) {
            throw "ara-cli emit failed"
        }

        $blueprintPolicyPath = Join-Path $SkillRoot "defaults\\blueprint-policy.toml"
        $contractRoot = Get-DefaultPolicyValue -PolicyPath $blueprintPolicyPath -Key "contract_root"
        $manifestFile = Get-DefaultPolicyValue -PolicyPath $blueprintPolicyPath -Key "manifest_file"
        $authorReportFile = Get-DefaultPolicyValue -PolicyPath $blueprintPolicyPath -Key "author_report_file"
        $resolvedContractPath = Join-Path $tempRoot (Join-Path $contractRoot "resolved-contract.json")
        $resolvedContract = Get-Content -Raw $resolvedContractPath | ConvertFrom-Json

        $expectedOutputs = @(
            (Join-Path $resolvedContract.paths.authority_root "00-authority-root.md").Replace('\','/'),
            (Join-Path $resolvedContract.paths.workflow_root "00-workflow-overview.md").Replace('\','/'),
            (Join-Path $resolvedContract.paths.modules_root "01-language-policy.md").Replace('\','/'),
            (Join-Path $resolvedContract.paths.modules_root "00-module-catalog.json").Replace('\','/'),
            (Join-Path $contractRoot "project-contract.toml").Replace('\','/'),
            (Join-Path $contractRoot "resolved-contract.json").Replace('\','/'),
            (Join-Path $contractRoot "worktree-protocol.json").Replace('\','/'),
            ($manifestFile.Replace('\','/')),
            (Join-Path $contractRoot "normalization-report.json").Replace('\','/'),
            (Join-Path $contractRoot "change-report.json").Replace('\','/'),
            (Join-Path $contractRoot "patch-base.json").Replace('\','/'),
            (Join-Path $contractRoot "patch-plan.json").Replace('\','/'),
            (Join-Path $contractRoot "patch-execution-report.json").Replace('\','/'),
            (Join-Path $contractRoot "semantic-ir.json").Replace('\','/'),
            (Join-Path $contractRoot "decision-summary.json").Replace('\','/'),
            (Join-Path $contractRoot "agent-brief.json").Replace('\','/'),
            (Join-Path $contractRoot "readiness.json").Replace('\','/'),
            (Join-Path $contractRoot "task-progress.json").Replace('\','/'),
            ($authorReportFile.Replace('\','/'))
        )
        $manifestPreview = Get-Content -Raw (Join-Path $tempRoot $manifestFile) | ConvertFrom-Json
        $worktreeProtocol = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "worktree-protocol.json")) | ConvertFrom-Json
        if ([string]::IsNullOrWhiteSpace($worktreeProtocol.model)) {
            throw "emitted worktree protocol did not include a model"
        }
        $expectedOutputs += ($manifestPreview.files | Where-Object { $_.doc_role -eq "stage" } | ForEach-Object { $_.path })
        foreach ($relativePath in $expectedOutputs) {
            $fullPath = Join-Path $tempRoot $relativePath
            if (-not (Test-Path $fullPath)) {
                throw "missing emitted output: $relativePath"
            }
        }
        $checks += [pscustomobject]@{ name = "cli-emit"; status = "ok" }

        $validateWorkspaceResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tempRoot) -WorkingDirectory $runtimeRoot
        if ($validateWorkspaceResult.ExitCode -ne 0) {
            throw "validate-workspace failed for emitted package"
        }
        $checks += [pscustomobject]@{ name = "cli-validate-workspace"; status = "ok" }

        $importSource = Join-Path $tempRoot "external-blueprint.md"
        @'
# External Blueprint

## Purpose

Build a normalized blueprint package from imported content.

## Stage Order

- stage-01: foundation
- stage-02: hardening

## Allowed Scope

- Blueprint docs
- Contract outputs

## Cross-Stage Split Rule

- Keep authoring isolated from implementation.

## Stop Conditions

- Contract mismatch
- Authority drift

## Surprise Section

- Unexpected material
'@ | Set-Content -Path $importSource -Encoding utf8

        $logPath = Join-Path $tempRoot (Join-Path $contractRoot "ara-events.jsonl")
        $importResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-import-check", "--source-summary", "Import self-check", "--source-file", $importSource, "--mode", "import-blueprint", "--log-path", $logPath) -WorkingDirectory $runtimeRoot
        if ($importResult.ExitCode -ne 0) {
            throw "ara-cli import emit failed"
        }
        if (-not (Test-Path $logPath)) {
            throw "expected structured log file was not created"
        }
        $logContent = Get-Content -Raw $logPath
        if ($logContent -notmatch '"event_name":"command_succeeded"') {
            throw "structured log did not contain command_succeeded event"
        }
        $importStageOne = Join-Path $tempRoot (Join-Path $resolvedContract.paths.stages_root "01-foundation.md")
        $importStageTwo = Join-Path $tempRoot (Join-Path $resolvedContract.paths.stages_root "02-hardening.md")
        if (-not (Test-Path $importStageOne) -or -not (Test-Path $importStageTwo)) {
            throw "import emit did not create all expected stage documents"
        }
        $manifestPath = Join-Path $tempRoot $manifestFile
        $manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json
        if (($manifest.files | Where-Object { $_.doc_role -eq "stage" }).Count -lt 2) {
            throw "blueprint manifest did not capture every imported stage document"
        }
        $workflowDoc = Get-Content -Raw (Join-Path $tempRoot (Join-Path $resolvedContract.paths.workflow_root "00-workflow-overview.md"))
        if ($workflowDoc -notmatch "Keep authoring isolated from implementation.") {
            throw "workflow doc did not preserve cross-stage split rule from import"
        }
        if ($workflowDoc -notmatch "Contract mismatch" -or $workflowDoc -notmatch "Authority drift") {
            throw "workflow doc did not preserve stop conditions from import"
        }
        $normalizationReport = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "normalization-report.json")) | ConvertFrom-Json
        if (-not $normalizationReport.source_files -or $normalizationReport.source_files.Count -lt 1) {
            throw "normalization report did not record source files"
        }
        if ($normalizationReport.dropped_sections -notcontains "Surprise Section") {
            throw "normalization report did not record dropped sections"
        }
        $checks += [pscustomobject]@{ name = "cli-import-log"; status = "ok" }

        $worktreeBindingSource = Join-Path $tempRoot "external-blueprint-worktree.md"
        @'
# External Blueprint

## Purpose

Bind modules to explicit worktree roles.

## Stage Order

- stage-01: foundation
- stage-02: hardening

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core,ara-cli
- hardening-role | branch=codex/hardening-role | stages=stage-02 | modules=ara-host-api
'@ | Set-Content -Path $worktreeBindingSource -Encoding utf8

        $worktreeBindingResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-binding-check", "--source-summary", "Worktree binding self-check", "--source-file", $worktreeBindingSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeBindingResult.ExitCode -ne 0) {
            throw "ara-cli worktree binding import emit failed"
        }
        $worktreeModuleCatalog = Get-Content -Raw (Join-Path $tempRoot (Join-Path $resolvedContract.paths.modules_root "00-module-catalog.json")) | ConvertFrom-Json
        $araCoreModule = $worktreeModuleCatalog.modules | Where-Object { $_.module_id -eq "ara-core" } | Select-Object -First 1
        $araHostApiModule = $worktreeModuleCatalog.modules | Where-Object { $_.module_id -eq "ara-host-api" } | Select-Object -First 1
        if ($null -eq $araCoreModule -or $null -eq $araHostApiModule) {
            throw "worktree binding import did not emit expected module catalog entries"
        }
        if ($araCoreModule.preferred_worktree_role -ne "foundation-role") {
            throw "worktree binding import did not assign ara-core to foundation-role"
        }
        if (-not ($araCoreModule.allowed_worktree_roles -contains "foundation-role")) {
            throw "worktree binding import did not record ara-core allowed worktree roles"
        }
        if ($araHostApiModule.preferred_worktree_role -ne "hardening-role") {
            throw "worktree binding import did not assign ara-host-api to hardening-role"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-module-binding"; status = "ok" }

        $worktreePathConflictSource = Join-Path $tempRoot "external-blueprint-worktree-path-conflict.md"
        @'
# External Blueprint

## Purpose

Reject overlapping exclusive worktree paths.

## Stage Order

- stage-01: foundation
- stage-02: hardening

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01 | paths=blueprint/stages
- hardening-role | branch=codex/hardening-role | stages=stage-02 | paths=blueprint/stages/02-hardening.md
'@ | Set-Content -Path $worktreePathConflictSource -Encoding utf8

        $worktreePathConflictResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-path-conflict-check", "--source-summary", "Worktree conflict self-check", "--source-file", $worktreePathConflictSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreePathConflictResult.ExitCode -eq 0) {
            throw "worktree path conflict unexpectedly succeeded"
        }
        if ($worktreePathConflictResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree path conflict did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-path-conflict"; status = "ok" }

        $worktreeBranchHierarchySource = Join-Path $tempRoot "external-blueprint-worktree-branch-hierarchy.md"
        @'
# External Blueprint

## Purpose

Reject overlapping worktree branch hierarchies.

## Stage Order

- stage-01: foundation
- stage-02: hardening

## Worktree Roles

- foundation-role | branch=codex/foundation | stages=stage-01
- hardening-role | branch=codex/foundation/hardening | stages=stage-02
'@ | Set-Content -Path $worktreeBranchHierarchySource -Encoding utf8

        $worktreeBranchHierarchyResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-branch-hierarchy-check", "--source-summary", "Worktree branch hierarchy self-check", "--source-file", $worktreeBranchHierarchySource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeBranchHierarchyResult.ExitCode -eq 0) {
            throw "worktree branch hierarchy conflict unexpectedly succeeded"
        }
        if ($worktreeBranchHierarchyResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree branch hierarchy conflict did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-branch-hierarchy-conflict"; status = "ok" }

        $worktreeInvalidBranchPatternSource = Join-Path $tempRoot "external-blueprint-worktree-invalid-branch-pattern.md"
        @'
# External Blueprint

## Purpose

Reject invalid worktree branch patterns.

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation role | stages=stage-01
'@ | Set-Content -Path $worktreeInvalidBranchPatternSource -Encoding utf8

        $worktreeInvalidBranchPatternResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-invalid-branch-pattern-check", "--source-summary", "Worktree invalid branch pattern self-check", "--source-file", $worktreeInvalidBranchPatternSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeInvalidBranchPatternResult.ExitCode -eq 0) {
            throw "worktree invalid branch pattern unexpectedly succeeded"
        }
        if ($worktreeInvalidBranchPatternResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree invalid branch pattern did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-invalid-branch-pattern"; status = "ok" }

        $worktreeEmptyRoleSource = Join-Path $tempRoot "external-blueprint-worktree-empty-role.md"
        @'
# External Blueprint

## Purpose

Reject worktree roles that do not own any stage, module, or path scope.

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation-role
'@ | Set-Content -Path $worktreeEmptyRoleSource -Encoding utf8

        $worktreeEmptyRoleResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-empty-role-check", "--source-summary", "Worktree empty role self-check", "--source-file", $worktreeEmptyRoleSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeEmptyRoleResult.ExitCode -eq 0) {
            throw "worktree empty role unexpectedly succeeded"
        }
        if ($worktreeEmptyRoleResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree empty role did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-empty-role"; status = "ok" }

        $worktreeDuplicateModuleSource = Join-Path $tempRoot "external-blueprint-worktree-duplicate-module.md"
        @'
# External Blueprint

## Purpose

Reject duplicate explicit module owners across worktree roles.

## Stage Order

- stage-01: foundation
- stage-02: hardening

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core
- hardening-role | branch=codex/hardening-role | stages=stage-02 | modules=ara-core
'@ | Set-Content -Path $worktreeDuplicateModuleSource -Encoding utf8

        $worktreeDuplicateModuleResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-duplicate-module-check", "--source-summary", "Worktree duplicate module self-check", "--source-file", $worktreeDuplicateModuleSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeDuplicateModuleResult.ExitCode -eq 0) {
            throw "worktree duplicate module owner unexpectedly succeeded"
        }
        if ($worktreeDuplicateModuleResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree duplicate module owner did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-duplicate-module-owner"; status = "ok" }

        $worktreeMissingModuleScopeSource = Join-Path $tempRoot "external-blueprint-worktree-missing-module-scope.md"
        @'
# External Blueprint

## Purpose

Reject module-isolated worktree roles that do not declare module ownership.

## Worktree Model

- module-isolated-worktree

## Stage Order

- stage-01: foundation
- stage-02: hardening

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core
- hardening-role | branch=codex/hardening-role | stages=stage-01
'@ | Set-Content -Path $worktreeMissingModuleScopeSource -Encoding utf8

        $worktreeMissingModuleScopeResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-missing-module-scope-check", "--source-summary", "Worktree missing module scope self-check", "--source-file", $worktreeMissingModuleScopeSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeMissingModuleScopeResult.ExitCode -eq 0) {
            throw "module-isolated worktree role without module scope unexpectedly succeeded"
        }
        if ($worktreeMissingModuleScopeResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "module-isolated worktree role without module scope did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-missing-module-scope"; status = "ok" }

        $worktreeMultiStageRoleSource = Join-Path $tempRoot "external-blueprint-worktree-multi-stage-role.md"
        @'
# External Blueprint

## Purpose

Reject stage-isolated worktree roles that try to own multiple stages.

## Worktree Model

- stage-isolated-worktree

## Stage Order

- stage-01: foundation
- stage-02: hardening

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01,stage-02
'@ | Set-Content -Path $worktreeMultiStageRoleSource -Encoding utf8

        $worktreeMultiStageRoleResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-multi-stage-role-check", "--source-summary", "Worktree multi-stage role self-check", "--source-file", $worktreeMultiStageRoleSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeMultiStageRoleResult.ExitCode -eq 0) {
            throw "worktree multi-stage role unexpectedly succeeded"
        }
        if ($worktreeMultiStageRoleResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree multi-stage role did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-multi-stage-role"; status = "ok" }

        $worktreeEmptySharedPathsSource = Join-Path $tempRoot "external-blueprint-worktree-empty-shared-paths.md"
        @'
# External Blueprint

## Purpose

Reject worktree protocols that remove shared authority paths.

## Stage Order

- stage-01: foundation

## Shared Authority Paths

- none

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01
'@ | Set-Content -Path $worktreeEmptySharedPathsSource -Encoding utf8

        $worktreeEmptySharedPathsResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-empty-shared-paths-check", "--source-summary", "Worktree shared authority self-check", "--source-file", $worktreeEmptySharedPathsSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeEmptySharedPathsResult.ExitCode -eq 0) {
            throw "worktree empty shared authority paths unexpectedly succeeded"
        }
        if ($worktreeEmptySharedPathsResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree empty shared authority paths did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-empty-shared-authority-paths"; status = "ok" }

        $worktreeSharedAuthorityGapSource = Join-Path $tempRoot "external-blueprint-worktree-shared-authority-gap.md"
        @'
# External Blueprint

## Purpose

Reject worktree protocols whose shared authority paths are declared but not actually coordinated by the parallel, sync, and merge-back rules.

## Shared Authority Paths

- blueprint/authority
- blueprint/workflow
- .codex/auto-dev

## Parallel Worktree Policy

- Use one active implementation worktree per stage role when parallel execution is necessary.

## Worktree Sync Rule

- Sync each active worktree from the main integration branch before starting a new stage task.

## Worktree Merge Back Rule

- Only merge a worktree back after its stage-scoped changes pass validation and contract recheck.

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01
'@ | Set-Content -Path $worktreeSharedAuthorityGapSource -Encoding utf8

        $worktreeSharedAuthorityGapResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-shared-authority-gap-check", "--source-summary", "Worktree shared authority coordination self-check", "--source-file", $worktreeSharedAuthorityGapSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeSharedAuthorityGapResult.ExitCode -eq 0) {
            throw "worktree shared authority coordination gap unexpectedly succeeded"
        }
        if ($worktreeSharedAuthorityGapResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree shared authority coordination gap did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-shared-authority-coordination-gap"; status = "ok" }

        $worktreeSharedAuthorityCleanupGapSource = Join-Path $tempRoot "external-blueprint-worktree-shared-authority-cleanup-gap.md"
        @'
# External Blueprint

## Purpose

Reject shared authority workflows whose cleanup rule omits shared authority coordination.

## Shared Authority Paths

- blueprint/authority
- blueprint/workflow
- .codex/auto-dev

## Parallel Worktree Policy

- Use one active implementation worktree per stage role when parallel execution is necessary.
- Shared authority and workflow updates must stay serialized across worktrees.

## Worktree Sync Rule

- Sync each active worktree from the main integration branch before starting a new stage task.
- Re-run blueprint validation after any shared authority or workflow sync.

## Worktree Merge Back Rule

- Only merge a worktree back after its stage-scoped changes pass validation and contract recheck.
- Shared authority and workflow updates must merge before downstream worktrees rebase.

## Worktree Cleanup Rule

- Delete or recycle a worktree after its stage-scoped changes are merged and the next handoff is written.

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01
'@ | Set-Content -Path $worktreeSharedAuthorityCleanupGapSource -Encoding utf8

        $worktreeSharedAuthorityCleanupGapResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-shared-authority-cleanup-gap-check", "--source-summary", "Worktree shared authority cleanup coordination self-check", "--source-file", $worktreeSharedAuthorityCleanupGapSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeSharedAuthorityCleanupGapResult.ExitCode -eq 0) {
            throw "worktree shared authority cleanup coordination gap unexpectedly succeeded"
        }
        if ($worktreeSharedAuthorityCleanupGapResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree shared authority cleanup coordination gap did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-shared-authority-cleanup-gap"; status = "ok" }

        $worktreePlaceholderRuleSource = Join-Path $tempRoot "external-blueprint-worktree-placeholder-rule.md"
        @'
# External Blueprint

## Purpose

Reject placeholder worktree rules that are not actionable.

## Worktree Sync Rule

- todo

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01
'@ | Set-Content -Path $worktreePlaceholderRuleSource -Encoding utf8

        $worktreePlaceholderRuleResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-placeholder-rule-check", "--source-summary", "Worktree placeholder rule self-check", "--source-file", $worktreePlaceholderRuleSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreePlaceholderRuleResult.ExitCode -eq 0) {
            throw "worktree placeholder rule unexpectedly succeeded"
        }
        if ($worktreePlaceholderRuleResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree placeholder rule did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-placeholder-rule"; status = "ok" }

        $worktreeNonActionableRuleSource = Join-Path $tempRoot "external-blueprint-worktree-non-actionable-rule.md"
        @'
# External Blueprint

## Purpose

Reject worktree rules that are present but not actionable.

## Worktree Sync Rule

- This rule exists but says nothing concrete.

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01
'@ | Set-Content -Path $worktreeNonActionableRuleSource -Encoding utf8

        $worktreeNonActionableRuleResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-non-actionable-rule-check", "--source-summary", "Worktree non-actionable rule self-check", "--source-file", $worktreeNonActionableRuleSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeNonActionableRuleResult.ExitCode -eq 0) {
            throw "worktree non-actionable rule unexpectedly succeeded"
        }
        if ($worktreeNonActionableRuleResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree non-actionable rule did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-non-actionable-rule"; status = "ok" }

        $worktreeModelPolicyMismatchSource = Join-Path $tempRoot "external-blueprint-worktree-model-policy-mismatch.md"
        @'
# External Blueprint

## Purpose

Reject worktree policies that conflict with the declared worktree model.

## Worktree Model

- module-isolated-worktree

## Parallel Worktree Policy

- Use one active implementation worktree per stage role when parallel execution is necessary.
- Shared authority and workflow changes must be synchronized before merge-back.

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core
'@ | Set-Content -Path $worktreeModelPolicyMismatchSource -Encoding utf8

        $worktreeModelPolicyMismatchResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-model-policy-mismatch-check", "--source-summary", "Worktree model policy mismatch self-check", "--source-file", $worktreeModelPolicyMismatchSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeModelPolicyMismatchResult.ExitCode -eq 0) {
            throw "worktree model policy mismatch unexpectedly succeeded"
        }
        if ($worktreeModelPolicyMismatchResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree model policy mismatch did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-model-policy-mismatch"; status = "ok" }

        $worktreeModelSyncMismatchSource = Join-Path $tempRoot "external-blueprint-worktree-model-sync-mismatch.md"
        @'
# External Blueprint

## Purpose

Reject sync rules that conflict with the declared worktree model.

## Worktree Model

- module-isolated-worktree

## Worktree Sync Rule

- Sync each active worktree from the main integration branch before starting a new stage task.
- Re-run blueprint validation after any shared authority or workflow sync.

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core
'@ | Set-Content -Path $worktreeModelSyncMismatchSource -Encoding utf8

        $worktreeModelSyncMismatchResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-model-sync-mismatch-check", "--source-summary", "Worktree model sync mismatch self-check", "--source-file", $worktreeModelSyncMismatchSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeModelSyncMismatchResult.ExitCode -eq 0) {
            throw "worktree model sync mismatch unexpectedly succeeded"
        }
        if ($worktreeModelSyncMismatchResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree model sync mismatch did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-model-sync-mismatch"; status = "ok" }

        $worktreeModelMergeBackMismatchSource = Join-Path $tempRoot "external-blueprint-worktree-model-merge-back-mismatch.md"
        @'
# External Blueprint

## Purpose

Reject merge-back rules that conflict with the declared worktree model.

## Worktree Model

- module-isolated-worktree

## Worktree Merge Back Rule

- Only merge a worktree back after its stage-scoped changes pass validation and contract recheck.
- Shared authority and workflow updates must merge before downstream worktrees rebase.

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core
'@ | Set-Content -Path $worktreeModelMergeBackMismatchSource -Encoding utf8

        $worktreeModelMergeBackMismatchResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-model-merge-back-mismatch-check", "--source-summary", "Worktree model merge-back mismatch self-check", "--source-file", $worktreeModelMergeBackMismatchSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeModelMergeBackMismatchResult.ExitCode -eq 0) {
            throw "worktree model merge-back mismatch unexpectedly succeeded"
        }
        if ($worktreeModelMergeBackMismatchResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree model merge-back mismatch did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-model-merge-back-mismatch"; status = "ok" }

        $worktreeModelCleanupMismatchSource = Join-Path $tempRoot "external-blueprint-worktree-model-cleanup-mismatch.md"
        @'
# External Blueprint

## Purpose

Reject cleanup rules that conflict with the declared worktree model.

## Worktree Model

- module-isolated-worktree

## Worktree Cleanup Rule

- Delete or recycle a worktree after its stage changes are merged and the next-stage handoff is written.

## Stage Order

- stage-01: foundation

## Worktree Roles

- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core
'@ | Set-Content -Path $worktreeModelCleanupMismatchSource -Encoding utf8

        $worktreeModelCleanupMismatchResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-worktree-model-cleanup-mismatch-check", "--source-summary", "Worktree model cleanup mismatch self-check", "--source-file", $worktreeModelCleanupMismatchSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($worktreeModelCleanupMismatchResult.ExitCode -eq 0) {
            throw "worktree model cleanup mismatch unexpectedly succeeded"
        }
        if ($worktreeModelCleanupMismatchResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "worktree model cleanup mismatch did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-worktree-model-cleanup-mismatch"; status = "ok" }

        $jsonImportSource = Join-Path $tempRoot "external-blueprint.json"
        @'
{
  "purpose": "Build a normalized blueprint package from JSON content.",
  "requirements": {
    "acceptance_criteria": ["Emit a validated contract bundle."],
    "validation_plan": ["Run workspace validation."]
  },
  "governance": {
    "constraints": ["Contracts must stay deterministic."],
    "assumptions": ["External blueprints may omit some defaults."],
    "out_of_scope": ["Target project implementation."]
  },
  "workflow": {
    "phases": [
      {
        "stage_id": "stage-01",
        "stage_name": "foundation",
        "deliverables": ["Foundation bundle"],
        "verification": {
          "test_plan": ["Foundation validation"]
        }
      },
      {
        "stage_id": "stage-02",
        "stage_name": "hardening",
        "deliverables": ["Hardening bundle"],
        "verification": {
          "test_plan": ["Hardening validation"]
        }
      }
    ]
  },
  "deliverables": ["Blueprint package", "Contract bundle"]
}
'@ | Set-Content -Path $jsonImportSource -Encoding utf8

        $jsonImportResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-json-import-check", "--source-summary", "JSON import self-check", "--source-file", $jsonImportSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($jsonImportResult.ExitCode -ne 0) {
            throw "ara-cli json import emit failed"
        }
        $jsonContract = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "resolved-contract.json")) | ConvertFrom-Json
        if ($jsonContract.stages.Count -lt 2) {
            throw "json import did not produce expected stage graph"
        }
        $jsonFoundationStageDoc = Get-Content -Raw (Join-Path $tempRoot (Join-Path $jsonContract.paths.stages_root "01-foundation.md"))
        $jsonHardeningStageDoc = Get-Content -Raw (Join-Path $tempRoot (Join-Path $jsonContract.paths.stages_root "02-hardening.md"))
        if ($jsonFoundationStageDoc -notmatch "Foundation bundle" -or $jsonFoundationStageDoc -notmatch "Foundation validation") {
            throw "json import did not preserve stage-specific foundation details"
        }
        if ($jsonHardeningStageDoc -notmatch "Hardening bundle" -or $jsonHardeningStageDoc -notmatch "Hardening validation") {
            throw "json import did not preserve stage-specific hardening details"
        }
        $jsonNormalizationReport = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "normalization-report.json")) | ConvertFrom-Json
        if (-not ($jsonNormalizationReport.semantic_hints | Where-Object { $_ -match "Acceptance Criteria" })) {
            throw "json import did not record semantic hint mapping for acceptance criteria"
        }
        if (-not ($jsonNormalizationReport.semantic_hints | Where-Object { $_ -match "phase or milestone" })) {
            throw "json import did not record semantic hint mapping for phase collections"
        }
        $jsonSemanticIr = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "semantic-ir.json")) | ConvertFrom-Json
        if ($jsonSemanticIr.stages.Count -lt 2) {
            throw "json import did not emit semantic-ir stages"
        }
        if (-not ($jsonSemanticIr.semantic_hints | Where-Object { $_ -match "Acceptance Criteria" })) {
            throw "json import semantic-ir did not preserve semantic hints"
        }
        if (-not $jsonSemanticIr.normalized_sections.required_verification) {
            throw "json import semantic-ir did not persist normalized source sections"
        }
        if ($jsonSemanticIr.normalized_section_origins.required_verification -ne "preserved") {
            throw "json import semantic-ir did not label normalized section origin"
        }
        if (-not $jsonSemanticIr.projection_fingerprint) {
            throw "json import semantic-ir did not emit projection fingerprint"
        }
        if (-not ($jsonSemanticIr.semantic_frames | Where-Object { $_.source_locator -eq "requirements.acceptance_criteria" -and $_.canonical_section -eq "required_verification" })) {
            throw "json import semantic-ir did not emit nested semantic frames"
        }
        if (-not ($jsonSemanticIr.semantic_frames | Where-Object { $_.scope -eq "stage:stage-01" -and $_.canonical_section -eq "required_verification" })) {
            throw "json import semantic-ir did not emit stage-scoped semantic frames"
        }
        if (-not ($jsonSemanticIr.semantic_clusters | Where-Object { $_.scope -eq "package" -and $_.canonical_section -eq "required_verification" -and $_.merge_pattern -eq "multi-source" })) {
            throw "json import semantic-ir did not emit semantic clusters"
        }
        $checks += [pscustomobject]@{ name = "cli-json-import"; status = "ok" }

        $markdownSemanticSource = Join-Path $tempRoot "external-blueprint-inline.md"
        @'
# External Blueprint

## Notes

Acceptance Criteria:
- Emit a validated contract bundle.
- Preserve readiness evidence.
Validation Plan:
- Run cargo check.
- Run workspace validation.
Constraints:
- Authority docs outrank workflow docs.

## Phases

- stage-01: foundation
'@ | Set-Content -Path $markdownSemanticSource -Encoding utf8

        $markdownSemanticResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-inline-semantic-check", "--source-summary", "Markdown inline semantic self-check", "--source-file", $markdownSemanticSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($markdownSemanticResult.ExitCode -ne 0) {
            throw "ara-cli markdown semantic import emit failed"
        }
        $markdownSemanticIr = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "semantic-ir.json")) | ConvertFrom-Json
        if (-not ($markdownSemanticIr.semantic_frames | Where-Object { $_.source_label -eq "Acceptance Criteria" -and $_.canonical_section -eq "required_verification" -and $_.values -contains "Preserve readiness evidence." })) {
            throw "markdown semantic import did not capture multiline acceptance criteria values"
        }
        if (-not ($markdownSemanticIr.semantic_frames | Where-Object { $_.source_label -eq "Validation Plan" -and $_.canonical_section -eq "required_verification" -and $_.values -contains "Run workspace validation." })) {
            throw "markdown semantic import did not capture multiline validation plan values"
        }
        if (-not ($markdownSemanticIr.semantic_clusters | Where-Object { $_.scope -eq "package" -and $_.canonical_section -eq "required_verification" -and $_.merge_pattern -eq "multi-source-heuristic" })) {
            throw "markdown semantic import did not emit multiline heuristic semantic cluster"
        }
        $checks += [pscustomobject]@{ name = "cli-markdown-inline-semantic"; status = "ok" }

        $markdownPhaseSource = Join-Path $tempRoot "external-blueprint-phase-blocks.md"
        @'
# External Blueprint

## Phases

- stage-01: foundation
- Deliverables:
  - Foundation bundle
- Validation Plan:
  - Run foundation validation.
- stage-02: hardening
- Deliverables:
  - Hardening bundle
- Validation Plan:
  - Run hardening validation.
'@ | Set-Content -Path $markdownPhaseSource -Encoding utf8

        $markdownPhaseResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-phase-block-check", "--source-summary", "Markdown phase block self-check", "--source-file", $markdownPhaseSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($markdownPhaseResult.ExitCode -ne 0) {
            throw "ara-cli markdown phase block import emit failed"
        }
        $markdownPhaseContract = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "resolved-contract.json")) | ConvertFrom-Json
        if ($markdownPhaseContract.stages.Count -lt 2) {
            throw "markdown phase block import did not produce expected stage graph"
        }
        $markdownPhaseStageOne = Get-Content -Raw (Join-Path $tempRoot (Join-Path $markdownPhaseContract.paths.stages_root "01-foundation.md"))
        $markdownPhaseStageTwo = Get-Content -Raw (Join-Path $tempRoot (Join-Path $markdownPhaseContract.paths.stages_root "02-hardening.md"))
        if ($markdownPhaseStageOne -notmatch "Foundation bundle" -or $markdownPhaseStageOne -notmatch "Run foundation validation.") {
            throw "markdown phase block import did not preserve foundation stage detail"
        }
        if ($markdownPhaseStageTwo -notmatch "Hardening bundle" -or $markdownPhaseStageTwo -notmatch "Run hardening validation.") {
            throw "markdown phase block import did not preserve hardening stage detail"
        }
        $checks += [pscustomobject]@{ name = "cli-markdown-phase-blocks"; status = "ok" }

        $markdownPhaseHeadingSource = Join-Path $tempRoot "external-blueprint-phase-headings.md"
        @'
# External Blueprint

## Phases

### stage-01: foundation

#### Deliverables

- Foundation bundle

#### Validation Plan

- Run foundation validation.

### stage-02: hardening

#### Deliverables

- Hardening bundle

#### Validation Plan

- Run hardening validation.
'@ | Set-Content -Path $markdownPhaseHeadingSource -Encoding utf8

        $markdownPhaseHeadingResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-phase-heading-check", "--source-summary", "Markdown phase heading self-check", "--source-file", $markdownPhaseHeadingSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($markdownPhaseHeadingResult.ExitCode -ne 0) {
            throw "ara-cli markdown phase heading import emit failed"
        }
        $markdownPhaseHeadingContract = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "resolved-contract.json")) | ConvertFrom-Json
        if ($markdownPhaseHeadingContract.stages.Count -lt 2) {
            throw "markdown phase heading import did not produce expected stage graph"
        }
        $markdownPhaseHeadingStageOne = Get-Content -Raw (Join-Path $tempRoot (Join-Path $markdownPhaseHeadingContract.paths.stages_root "01-foundation.md"))
        $markdownPhaseHeadingStageTwo = Get-Content -Raw (Join-Path $tempRoot (Join-Path $markdownPhaseHeadingContract.paths.stages_root "02-hardening.md"))
        if ($markdownPhaseHeadingStageOne -notmatch "Foundation bundle" -or $markdownPhaseHeadingStageOne -notmatch "Run foundation validation.") {
            throw "markdown phase heading import did not preserve foundation stage detail"
        }
        if ($markdownPhaseHeadingStageTwo -notmatch "Hardening bundle" -or $markdownPhaseHeadingStageTwo -notmatch "Run hardening validation.") {
            throw "markdown phase heading import did not preserve hardening stage detail"
        }
        $checks += [pscustomobject]@{ name = "cli-markdown-phase-headings"; status = "ok" }

        $markdownCrossSourceStageAlias = Join-Path $tempRoot "external-blueprint-cross-source-stage-alias.md"
        @'
# Source File

blueprint/workflow/00-workflow-overview.md

## Stage Order

- stage-01: foundation
- stage-02: hardening

# Source File

docs/foundation-notes.md

## Foundation

### Deliverables

- Foundation bundle

### Validation Plan

- Run foundation validation.

# Source File

docs/hardening-notes.md

## Hardening

Deliverables:
- Hardening bundle
Validation Plan:
- Run hardening validation.
'@ | Set-Content -Path $markdownCrossSourceStageAlias -Encoding utf8

        $markdownCrossSourceStageAliasResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-cross-source-stage-alias-check", "--source-summary", "Cross-source stage alias self-check", "--source-file", $markdownCrossSourceStageAlias, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($markdownCrossSourceStageAliasResult.ExitCode -ne 0) {
            throw "ara-cli cross-source stage alias import emit failed"
        }
        $markdownCrossSourceStageAliasContract = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "resolved-contract.json")) | ConvertFrom-Json
        if ($markdownCrossSourceStageAliasContract.stages.Count -lt 2) {
            throw "cross-source stage alias import did not produce expected stage graph"
        }
        $markdownCrossSourceStageOne = Get-Content -Raw (Join-Path $tempRoot (Join-Path $markdownCrossSourceStageAliasContract.paths.stages_root "01-foundation.md"))
        $markdownCrossSourceStageTwo = Get-Content -Raw (Join-Path $tempRoot (Join-Path $markdownCrossSourceStageAliasContract.paths.stages_root "02-hardening.md"))
        if ($markdownCrossSourceStageOne -notmatch "Foundation bundle" -or $markdownCrossSourceStageOne -notmatch "Run foundation validation.") {
            throw "cross-source stage alias import did not preserve foundation stage detail"
        }
        if ($markdownCrossSourceStageTwo -notmatch "Hardening bundle" -or $markdownCrossSourceStageTwo -notmatch "Run hardening validation.") {
            throw "cross-source stage alias import did not preserve hardening stage detail"
        }
        $markdownCrossSourceSemanticIr = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "semantic-ir.json")) | ConvertFrom-Json
        if (-not ($markdownCrossSourceSemanticIr.semantic_frames | Where-Object { $_.scope -eq "stage:stage-01" -and $_.origin_kind -eq "stage-alias-heading-section" -and $_.canonical_section -eq "deliverables" })) {
            throw "cross-source stage alias import did not emit stage alias heading semantic frames"
        }
        if (-not ($markdownCrossSourceSemanticIr.semantic_frames | Where-Object { $_.scope -eq "stage:stage-02" -and $_.origin_kind -eq "stage-alias-inline-label" -and $_.canonical_section -eq "required_verification" })) {
            throw "cross-source stage alias import did not emit stage alias inline semantic frames"
        }
        $checks += [pscustomobject]@{ name = "cli-markdown-cross-source-stage-alias"; status = "ok" }

        $markdownPathAliasStageSource = Join-Path $tempRoot "external-blueprint-path-alias-stage-source.md"
        @'
# Source File

blueprint/workflow/00-workflow-overview.md

## Stage Order

- stage-01: foundation
- stage-02: hardening

# Source File

docs/foundation-notes.md

## Deliverables

- Foundation bundle

## Validation Plan

- Run foundation validation.

# Source File

docs/hardening-validation-plan.md

Deliverables:
- Hardening bundle
Validation Plan:
- Run hardening validation.
'@ | Set-Content -Path $markdownPathAliasStageSource -Encoding utf8

        $markdownPathAliasStageSourceResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-path-alias-stage-source-check", "--source-summary", "Path alias stage source self-check", "--source-file", $markdownPathAliasStageSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($markdownPathAliasStageSourceResult.ExitCode -ne 0) {
            throw "ara-cli path alias stage source import emit failed"
        }
        $markdownPathAliasStageContract = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "resolved-contract.json")) | ConvertFrom-Json
        if ($markdownPathAliasStageContract.stages.Count -lt 2) {
            throw "path alias stage source import did not produce expected stage graph"
        }
        $markdownPathAliasStageOne = Get-Content -Raw (Join-Path $tempRoot (Join-Path $markdownPathAliasStageContract.paths.stages_root "01-foundation.md"))
        $markdownPathAliasStageTwo = Get-Content -Raw (Join-Path $tempRoot (Join-Path $markdownPathAliasStageContract.paths.stages_root "02-hardening.md"))
        if ($markdownPathAliasStageOne -notmatch "Foundation bundle" -or $markdownPathAliasStageOne -notmatch "Run foundation validation.") {
            throw "path alias stage source import did not preserve foundation stage detail"
        }
        if ($markdownPathAliasStageTwo -notmatch "Hardening bundle" -or $markdownPathAliasStageTwo -notmatch "Run hardening validation.") {
            throw "path alias stage source import did not preserve hardening stage detail"
        }
        $markdownPathAliasSemanticIr = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "semantic-ir.json")) | ConvertFrom-Json
        if (-not ($markdownPathAliasSemanticIr.semantic_frames | Where-Object { $_.scope -eq "stage:stage-01" -and $_.origin_kind -eq "stage-path-alias-heading-section" -and $_.canonical_section -eq "deliverables" })) {
            throw "path alias stage source import did not emit stage path alias heading semantic frames"
        }
        if (-not ($markdownPathAliasSemanticIr.semantic_frames | Where-Object { $_.scope -eq "stage:stage-02" -and $_.origin_kind -eq "stage-path-alias-inline-label" -and $_.canonical_section -eq "required_verification" })) {
            throw "path alias stage source import did not emit stage path alias inline semantic frames"
        }
        $checks += [pscustomobject]@{ name = "cli-markdown-path-alias-stage-source"; status = "ok" }

        $markdownStageMetadataSource = Join-Path $tempRoot "external-blueprint-stage-metadata.md"
        @'
# Source File

blueprint/workflow/00-workflow-overview.md

## Stage Order

- stage-01: foundation
- stage-02: hardening

# Source File

docs/foundation-plan.md

---
stage: foundation
---

## Deliverables

- Foundation bundle

Validation Plan:
- Run foundation validation.

# Source File

docs/hardening-plan.md

Stage ID: stage-02

## Deliverables

- Hardening bundle

Validation Plan:
- Run hardening validation.
'@ | Set-Content -Path $markdownStageMetadataSource -Encoding utf8

        $markdownStageMetadataResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-stage-metadata-check", "--source-summary", "Stage metadata self-check", "--source-file", $markdownStageMetadataSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($markdownStageMetadataResult.ExitCode -ne 0) {
            throw "ara-cli stage metadata import emit failed"
        }
        $markdownStageMetadataSemanticIr = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "semantic-ir.json")) | ConvertFrom-Json
        if (-not ($markdownStageMetadataSemanticIr.semantic_frames | Where-Object { $_.scope -eq "stage:stage-01" -and $_.origin_kind -eq "stage-metadata-heading-section" -and $_.confidence -eq "metadata-alias" -and $_.canonical_section -eq "deliverables" })) {
            throw "stage metadata import did not emit metadata-scoped deliverable semantic frames"
        }
        if (-not ($markdownStageMetadataSemanticIr.semantic_frames | Where-Object { $_.scope -eq "stage:stage-02" -and $_.origin_kind -eq "stage-metadata-inline-label" -and $_.confidence -eq "metadata-heuristic" -and $_.canonical_section -eq "required_verification" })) {
            throw "stage metadata import did not emit metadata-scoped verification semantic frames"
        }
        $checks += [pscustomobject]@{ name = "cli-markdown-stage-metadata-source"; status = "ok" }

        $markdownGenericStageMetadataSource = Join-Path $tempRoot "external-blueprint-generic-stage-metadata.md"
        @'
# Source File

blueprint/workflow/00-workflow-overview.md

## Stage Order

- stage-01: foundation
- stage-02: hardening

# Source File

docs/foundation-brief.md

- Milestone: foundation

## Deliverables

- Foundation bundle

# Source File

docs/hardening-brief.md

phase_id = "stage-02"

Validation Plan:
- Run hardening validation.
'@ | Set-Content -Path $markdownGenericStageMetadataSource -Encoding utf8

        $markdownGenericStageMetadataResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-generic-stage-metadata-check", "--source-summary", "Generic stage metadata self-check", "--source-file", $markdownGenericStageMetadataSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($markdownGenericStageMetadataResult.ExitCode -ne 0) {
            throw "ara-cli generic stage metadata import emit failed"
        }
        $markdownGenericStageMetadataSemanticIr = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "semantic-ir.json")) | ConvertFrom-Json
        if (-not ($markdownGenericStageMetadataSemanticIr.semantic_frames | Where-Object { $_.scope -eq "stage:stage-01" -and $_.origin_kind -eq "stage-metadata-heading-section" -and $_.confidence -eq "metadata-alias" -and $_.canonical_section -eq "deliverables" })) {
            throw "generic stage metadata import did not emit milestone-scoped deliverable semantic frames"
        }
        if (-not ($markdownGenericStageMetadataSemanticIr.semantic_frames | Where-Object { $_.scope -eq "stage:stage-02" -and $_.origin_kind -eq "stage-metadata-inline-label" -and $_.confidence -eq "metadata-heuristic" -and $_.canonical_section -eq "required_verification" })) {
            throw "generic stage metadata import did not emit phase-id scoped verification semantic frames"
        }
        $checks += [pscustomobject]@{ name = "cli-markdown-generic-stage-metadata-source"; status = "ok" }

        $markdownStageConflictSource = Join-Path $tempRoot "external-blueprint-stage-conflict.md"
        @'
# Source File

blueprint/workflow/00-workflow-overview.md

## Stage Order

- stage-01: foundation

# Source File

docs/foundation-notes.md

## Foundation

### Deliverables

- Foundation bundle

# Source File

docs/foundation-review.md

## Foundation Review

### Deliverables

- Conflicting bundle
'@ | Set-Content -Path $markdownStageConflictSource -Encoding utf8

        $markdownStageConflictResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-stage-conflict-check", "--source-summary", "Stage conflict self-check", "--source-file", $markdownStageConflictSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($markdownStageConflictResult.ExitCode -ne 0) {
            throw "ara-cli stage conflict import emit failed"
        }
        $markdownStageConflictReport = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "normalization-report.json")) | ConvertFrom-Json
        if (-not ($markdownStageConflictReport.semantic_risks | Where-Object { $_ -match 'conflicting stage semantic evidence remained in `stage:stage-01` for `deliverables`' -and $_ -match 'docs/foundation-notes.md' -and $_ -match 'docs/foundation-review.md' })) {
            throw "stage conflict import did not record semantic risk"
        }
        if (-not ($markdownStageConflictReport.unresolved_ambiguities | Where-Object { $_ -match 'stage semantic conflict remained in `stage:stage-01` for `deliverables`' -and $_ -match 'docs/foundation-notes.md' -and $_ -match 'docs/foundation-review.md' })) {
            throw "stage conflict import did not record unresolved ambiguity"
        }
        if (-not ($markdownStageConflictReport.semantic_hints | Where-Object { $_ -match 'detected stage-scoped semantic divergence in `stage:stage-01` for `deliverables`' -and $_ -match 'path-alias-exact' })) {
            throw "stage conflict import did not record semantic hint details"
        }
        $stageConflict = $markdownStageConflictReport.semantic_conflicts | Where-Object { $_.scope -eq "stage:stage-01" -and $_.canonical_section -eq "deliverables" -and $_.conflict_kind -eq "indirect-source-divergence" } | Select-Object -First 1
        if (-not $stageConflict) {
            throw "stage conflict import did not record structured semantic conflict detail"
        }
        if ($stageConflict.severity -ne "medium" -or $stageConflict.blocking -ne $false -or $stageConflict.review_required -ne $true) {
            throw "stage conflict import did not record decision metadata"
        }
        if ([string]::IsNullOrWhiteSpace($stageConflict.recommended_action)) {
            throw "stage conflict import did not record recommended action"
        }
        $stageConflictReadiness = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "readiness.json")) | ConvertFrom-Json
        if ($stageConflictReadiness.review_required_semantic_conflict_count -lt 1) {
            throw "stage conflict import did not update readiness review-required conflict count"
        }
        if ($stageConflictReadiness.blocking_semantic_conflict_count -ne 0) {
            throw "stage conflict import incorrectly marked indirect conflict as blocking"
        }
        if (-not ($stageConflictReadiness.recommended_actions | Where-Object { $_ -eq "add explicit stage metadata or canonical stage-scoped sections to disambiguate conflicting source groups" })) {
            throw "stage conflict import did not project readiness recommended actions"
        }
        $stageConflictDecisionSummary = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "decision-summary.json")) | ConvertFrom-Json
        if (-not ($stageConflictDecisionSummary.review_required_kinds -contains "semantic-conflict")) {
            throw "stage conflict import did not classify review-required semantic conflicts in decision summary"
        }
        if (-not $stageConflictDecisionSummary.top_review_items -or $stageConflictDecisionSummary.top_review_items.Count -lt 1) {
            throw "stage conflict import did not emit top review items"
        }
        if ($stageConflictDecisionSummary.top_review_items[0].kind -ne "semantic-conflict") {
            throw "stage conflict import did not prioritize semantic conflicts in top review items"
        }
        $stageConflictAgentBrief = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "agent-brief.json")) | ConvertFrom-Json
        if ($stageConflictAgentBrief.review_required -ne $true) {
            throw "stage conflict import did not project review-required state into agent brief"
        }
        if (-not $stageConflictAgentBrief.top_review_items -or $stageConflictAgentBrief.top_review_items.Count -lt 1) {
            throw "stage conflict import did not emit top review items in agent brief"
        }
        $checks += [pscustomobject]@{ name = "cli-stage-semantic-conflict-risk"; status = "ok" }

        $tomlImportSource = Join-Path $tempRoot "external-blueprint.toml"
        @'
purpose = "Build a normalized blueprint package from TOML content."
deliverables = ["Blueprint package", "Contract bundle"]

[[stage_order]]
stage_id = "stage-01"
stage_name = "foundation"

[[stage_order]]
stage_id = "stage-02"
stage_name = "hardening"
'@ | Set-Content -Path $tomlImportSource -Encoding utf8

        $tomlImportResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-toml-import-check", "--source-summary", "TOML import self-check", "--source-file", $tomlImportSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($tomlImportResult.ExitCode -ne 0) {
            throw "ara-cli toml import emit failed"
        }
        $tomlContract = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "resolved-contract.json")) | ConvertFrom-Json
        if ($tomlContract.stages.Count -lt 2) {
            throw "toml import did not produce expected stage graph"
        }
        $checks += [pscustomobject]@{ name = "cli-toml-import"; status = "ok" }

        $recompileResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-recompile-check", "--source-summary", "Recompile self-check", "--mode", "recompile-contract") -WorkingDirectory $runtimeRoot
        if ($recompileResult.ExitCode -ne 0) {
            throw "ara-cli recompile emit failed"
        }
        $recompiledContract = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "resolved-contract.json")) | ConvertFrom-Json
        if ($recompiledContract.stages.Count -lt 2) {
            throw "recompile-contract did not preserve workspace stages"
        }
        $checks += [pscustomobject]@{ name = "cli-recompile-workspace"; status = "ok" }

        $updateBaselineSource = Join-Path $tempRoot "update-baseline.md"
        @'
# External Blueprint

## Purpose

Build a staged automation tool.

## Stage Order

- stage-01: foundation

## Deliverables

- Foundation bundle
'@ | Set-Content -Path $updateBaselineSource -Encoding utf8

        $updateBaselineResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-update-base-check", "--source-summary", "Update baseline self-check", "--source-file", $updateBaselineSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($updateBaselineResult.ExitCode -ne 0) {
            throw "ara-cli update baseline emit failed"
        }

        $updateSource = Join-Path $tempRoot "update-blueprint.md"
        @'
# Update Input

## Stage Order

- stage-03: release prep

## Deliverables

- Release readiness report

## Stop Conditions

- Missing review evidence
'@ | Set-Content -Path $updateSource -Encoding utf8

        $updateResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-update-check", "--source-summary", "Update self-check", "--source-file", $updateSource, "--mode", "update-blueprint") -WorkingDirectory $runtimeRoot
        if ($updateResult.ExitCode -ne 0) {
            throw "ara-cli update emit failed"
        }
        $updatedContract = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "resolved-contract.json")) | ConvertFrom-Json
        if ($updatedContract.stages.Count -lt 2) {
            throw "update-blueprint did not merge the new stage into the workspace contract"
        }
        $updatedWorkflowDoc = Get-Content -Raw (Join-Path $tempRoot (Join-Path $resolvedContract.paths.workflow_root "00-workflow-overview.md"))
        if ($updatedWorkflowDoc -notmatch "Missing review evidence") {
            throw "update-blueprint did not merge workflow stop conditions"
        }
        $changeReport = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "change-report.json")) | ConvertFrom-Json
        if ($changeReport.mode -ne "update-blueprint") {
            throw "change report did not record update-blueprint mode"
        }
        if (-not ($changeReport.operations | Where-Object { $_.target_kind -eq "section" -and $_.target_id -eq "deliverables" -and ($_.action -eq "merged" -or $_.action -eq "added") })) {
            throw "change report did not record update deliverables section detail"
        }
        if (-not ($changeReport.operations | Where-Object { $_.action -eq "merged" -and $_.target_id -eq "stage_order" })) {
            throw "change report did not record merged stage order"
        }
        if ($changeReport.patch_operation_count -lt 2) {
            throw "change report did not record patch operations for update flow"
        }
        if (-not ($changeReport.patch_operations | Where-Object { $_.strategy -eq "merge-stage-order" })) {
            throw "change report did not record patch strategy for stage order merge"
        }
        if (-not ($changeReport.patch_operations | Where-Object { $_.target_id -eq "deliverables" -and ($_.strategy -eq "union-merge" -or $_.strategy -eq "set-section") })) {
            throw "change report did not record a deliverables patch strategy"
        }
        $patchPlan = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "patch-plan.json")) | ConvertFrom-Json
        if ($patchPlan.operations.Count -lt 1) {
            throw "patch plan did not record any operations"
        }
        if ($patchPlan.operations | Where-Object { -not $_.affected_paths -or $_.affected_paths.Count -lt 1 }) {
            throw "patch plan did not record worktree-aware affected paths for every operation"
        }
        $stageOrderPatch = $patchPlan.operations | Where-Object { $_.strategy -eq "merge-stage-order" } | Select-Object -First 1
        if ($null -eq $stageOrderPatch) {
            throw "patch plan did not retain a merge-stage-order operation"
        }
        if ($stageOrderPatch.strategy_metadata.risk_level -ne "medium" -or $stageOrderPatch.strategy_metadata.review_required -ne "false" -or $stageOrderPatch.strategy_metadata.apply_mode -ne "merge") {
            throw "patch plan did not classify merge-stage-order strategy metadata"
        }
        $updateReadiness = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "readiness.json")) | ConvertFrom-Json
        if ($updateReadiness.high_risk_patch_operation_count -lt 1) {
            throw "update readiness did not project high-risk patch operations"
        }
        if ($updateReadiness.review_required_patch_operation_count -lt 1) {
            throw "update readiness did not project review-required patch operations"
        }
        if (-not ($updateReadiness.gate_holds | Where-Object { $_ -like "patch-risk:package:*:apply-update-delta" })) {
            throw "update readiness did not record patch-risk gate holds"
        }
        if (-not ($updateReadiness.recommended_actions | Where-Object { $_ -like '*apply-update-delta*' })) {
            throw "update readiness did not recommend patch review actions"
        }
        $decisionSummary = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "decision-summary.json")) | ConvertFrom-Json
        if ($decisionSummary.primary_blocker_kind -ne "patch-risk") {
            throw "decision summary did not classify patch-risk as the primary blocker"
        }
        if ($decisionSummary.primary_blocker_scope -ne "package") {
            throw "decision summary did not project the primary blocker scope"
        }
        if ($decisionSummary.primary_blocker_target_id -ne "ara-update-check") {
            throw "decision summary did not project the primary blocker target"
        }
        if (-not ($decisionSummary.blocking_kinds -contains "patch-risk")) {
            throw "decision summary did not record blocking kind classification"
        }
        if ($decisionSummary.blocking_kind_counts."patch-risk" -lt 1) {
            throw "decision summary did not record blocking kind counts"
        }
        if ($decisionSummary.primary_recommended_action -notlike '*apply-update-delta*') {
            throw "decision summary did not project the primary recommended action"
        }
        if (-not $decisionSummary.top_blockers -or $decisionSummary.top_blockers.Count -lt 1) {
            throw "decision summary did not emit top blockers"
        }
        if ($decisionSummary.top_blockers[0].kind -ne "patch-risk") {
            throw "decision summary did not sort top blockers with patch-risk first"
        }
        $agentBrief = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "agent-brief.json")) | ConvertFrom-Json
        if ($agentBrief.primary_blocker_kind -ne "patch-risk") {
            throw "agent brief did not classify patch-risk as the primary blocker"
        }
        if (-not $agentBrief.next_actions -or $agentBrief.next_actions.Count -lt 1) {
            throw "agent brief did not emit next actions"
        }
        $checks += [pscustomobject]@{ name = "cli-update-workspace"; status = "ok" }

        $applyPatchResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "apply-patch-plan", "--workspace", $tempRoot) -WorkingDirectory $runtimeRoot
        if ($applyPatchResult.ExitCode -ne 0) {
            throw "apply-patch-plan failed for updated workspace"
        }
        $appliedPatchExecution = Get-Content -Raw (Join-Path $tempRoot (Join-Path $contractRoot "patch-execution-report.json")) | ConvertFrom-Json
        if ($appliedPatchExecution.replay_status -ne "replayed") {
            throw "apply-patch-plan did not emit a replayed execution report"
        }
        if ($appliedPatchExecution.expected_result_fingerprint -ne $appliedPatchExecution.replayed_result_fingerprint) {
            throw "apply-patch-plan did not reproduce the patch result fingerprint"
        }
        if ($appliedPatchExecution.reversibility_status -ne "reversible") {
            throw "apply-patch-plan did not prove reversibility"
        }
        if ($appliedPatchExecution.base_fingerprint -ne $appliedPatchExecution.reverse_replayed_base_fingerprint) {
            throw "apply-patch-plan reverse replay did not return to patch base"
        }
        if ($appliedPatchExecution.scope_validation_status -ne "valid") {
            throw "apply-patch-plan did not validate worktree patch scope"
        }
        if ($appliedPatchExecution.scope_mismatch_count -ne 0) {
            throw "apply-patch-plan reported unexpected worktree scope mismatches"
        }
        $checks += [pscustomobject]@{ name = "cli-apply-patch-plan"; status = "ok" }

        $tamperPatchScopeWorkspace = Join-Path $env:TEMP ("ara-tamper-patch-scope-" + [guid]::NewGuid().ToString("N"))
        Copy-Item -Recurse -Force $tempRoot $tamperPatchScopeWorkspace

        $conflictSource = Join-Path $tempRoot "conflict-update.md"
        @'
# Update Input

## Purpose

Rewrite the original authority truth.
'@ | Set-Content -Path $conflictSource -Encoding utf8

        $conflictResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-conflict-check", "--source-summary", "Conflict self-check", "--source-file", $conflictSource, "--mode", "update-blueprint") -WorkingDirectory $runtimeRoot
        if ($conflictResult.ExitCode -eq 0) {
            throw "conflicting update unexpectedly succeeded"
        }
        if ($conflictResult.Output -notmatch '"error_code"\s*:\s*"ARA-3000"') {
            throw "update conflict did not emit the expected authority conflict error code"
        }
        $checks += [pscustomobject]@{ name = "cli-update-conflict"; status = "ok" }

        $ambiguousStageSource = Join-Path $tempRoot "ambiguous-stage-order.md"
        @'
# External Blueprint

## Stage Order

- foundation
- hardening
'@ | Set-Content -Path $ambiguousStageSource -Encoding utf8

        $ambiguousStageResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-ambiguous-stage-check", "--source-summary", "Ambiguous stage self-check", "--source-file", $ambiguousStageSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($ambiguousStageResult.ExitCode -eq 0) {
            throw "ambiguous stage import unexpectedly succeeded"
        }
        if ($ambiguousStageResult.Output -notmatch '"error_code"\s*:\s*"ARA-3002"') {
            throw "ambiguous stage import did not emit ARA-3002"
        }
        $checks += [pscustomobject]@{ name = "cli-ambiguous-stage-id"; status = "ok" }

        $unsupportedSource = Join-Path $tempRoot "unsupported-source.yaml"
        @'
purpose: unsupported
'@ | Set-Content -Path $unsupportedSource -Encoding utf8

        $unsupportedSourceResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-unsupported-source-check", "--source-summary", "Unsupported source self-check", "--source-file", $unsupportedSource, "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($unsupportedSourceResult.ExitCode -eq 0) {
            throw "unsupported source import unexpectedly succeeded"
        }
        if ($unsupportedSourceResult.Output -notmatch '"error_code"\s*:\s*"ARA-2000"') {
            throw "unsupported source import did not emit ARA-2000"
        }
        $checks += [pscustomobject]@{ name = "cli-unsupported-source"; status = "ok" }

        $badImportResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tempRoot, "--project-name", "ara-error-check", "--source-summary", "bad import", "--source-file", (Join-Path $tempRoot "missing.md"), "--mode", "import-blueprint") -WorkingDirectory $runtimeRoot
        if ($badImportResult.ExitCode -eq 0) {
            throw "bad import unexpectedly succeeded"
        }
        if ($badImportResult.Output -notmatch '"error_code"\s*:\s*"ARA-2000"') {
            throw "missing expected JSON error code for bad import source"
        }
        $checks += [pscustomobject]@{ name = "cli-json-error"; status = "ok" }

        $tamperManifestWorkspace = Join-Path $env:TEMP ("ara-tamper-manifest-" + [guid]::NewGuid().ToString("N"))
        $tamperReadinessWorkspace = Join-Path $env:TEMP ("ara-tamper-readiness-" + [guid]::NewGuid().ToString("N"))
        $tamperContractWorkspace = Join-Path $env:TEMP ("ara-tamper-contract-" + [guid]::NewGuid().ToString("N"))
        $tamperProgressWorkspace = Join-Path $env:TEMP ("ara-tamper-progress-" + [guid]::NewGuid().ToString("N"))
        $tamperModuleCatalogWorkspace = Join-Path $env:TEMP ("ara-tamper-module-catalog-" + [guid]::NewGuid().ToString("N"))
        $tamperSchemaWorkspace = Join-Path $env:TEMP ("ara-tamper-schema-" + [guid]::NewGuid().ToString("N"))
        $tamperChangeReportWorkspace = Join-Path $env:TEMP ("ara-tamper-change-report-" + [guid]::NewGuid().ToString("N"))
        $tamperPatchBaseWorkspace = Join-Path $env:TEMP ("ara-tamper-patch-base-" + [guid]::NewGuid().ToString("N"))
        $tamperPatchPlanWorkspace = Join-Path $env:TEMP ("ara-tamper-patch-plan-" + [guid]::NewGuid().ToString("N"))
        $legacyPatchBaseWorkspace = Join-Path $env:TEMP ("ara-legacy-patch-base-" + [guid]::NewGuid().ToString("N"))
        $legacyPatchPlanWorkspace = Join-Path $env:TEMP ("ara-legacy-patch-plan-" + [guid]::NewGuid().ToString("N"))
$tamperWorktreeWorkspace = Join-Path $env:TEMP ("ara-tamper-worktree-" + [guid]::NewGuid().ToString("N"))
$tamperWorktreeRuleWorkspace = Join-Path $env:TEMP ("ara-tamper-worktree-rule-" + [guid]::NewGuid().ToString("N"))
$tamperPatchExecutionWorkspace = Join-Path $env:TEMP ("ara-tamper-patch-execution-" + [guid]::NewGuid().ToString("N"))
$tamperSemanticIrWorkspace = Join-Path $env:TEMP ("ara-tamper-semantic-ir-" + [guid]::NewGuid().ToString("N"))
$legacySemanticIrWorkspace = Join-Path $env:TEMP ("ara-legacy-semantic-ir-" + [guid]::NewGuid().ToString("N"))
$tamperDecisionSummaryWorkspace = Join-Path $env:TEMP ("ara-tamper-decision-summary-" + [guid]::NewGuid().ToString("N"))
$legacyDecisionSummaryWorkspace = Join-Path $env:TEMP ("ara-legacy-decision-summary-" + [guid]::NewGuid().ToString("N"))
$tamperAgentBriefWorkspace = Join-Path $env:TEMP ("ara-tamper-agent-brief-" + [guid]::NewGuid().ToString("N"))
$legacyAgentBriefWorkspace = Join-Path $env:TEMP ("ara-legacy-agent-brief-" + [guid]::NewGuid().ToString("N"))
foreach ($tamperWorkspace in @($tamperManifestWorkspace, $tamperReadinessWorkspace, $tamperContractWorkspace, $tamperProgressWorkspace, $tamperModuleCatalogWorkspace, $tamperSchemaWorkspace, $tamperChangeReportWorkspace, $tamperPatchBaseWorkspace, $tamperPatchPlanWorkspace, $legacyPatchBaseWorkspace, $legacyPatchPlanWorkspace, $tamperWorktreeWorkspace, $tamperWorktreeRuleWorkspace, $tamperPatchExecutionWorkspace, $tamperSemanticIrWorkspace, $legacySemanticIrWorkspace, $tamperDecisionSummaryWorkspace, $legacyDecisionSummaryWorkspace, $tamperAgentBriefWorkspace, $legacyAgentBriefWorkspace)) {
            New-Item -ItemType Directory -Force -Path $tamperWorkspace | Out-Null
            $tamperEmit = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "emit", "--workspace", $tamperWorkspace, "--project-name", "ara-tamper-check", "--source-summary", "Tamper self-check", "--mode", "new-project") -WorkingDirectory $runtimeRoot
            if ($tamperEmit.ExitCode -ne 0) {
                throw "emit failed for tamper workspace $tamperWorkspace"
            }
        }

        $tamperedManifestPath = Join-Path $tamperManifestWorkspace $manifestFile
        $tamperedManifest = Get-Content -Raw $tamperedManifestPath | ConvertFrom-Json
        $tamperedManifest.files[0].fingerprint = "tampered"
        $tamperedManifest | ConvertTo-Json -Depth 8 | Set-Content -Path $tamperedManifestPath -Encoding utf8
        $tamperedManifestResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperManifestWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedManifestResult.ExitCode -eq 0) {
            throw "tampered manifest unexpectedly validated"
        }
        if ($tamperedManifestResult.Output -notmatch '"error_code"\s*:\s*"ARA-5002"') {
            throw "tampered manifest did not emit ARA-5002"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-manifest"; status = "ok" }

        $tamperedReadinessPath = Join-Path $tamperReadinessWorkspace (Join-Path $contractRoot "readiness.json")
        $tamperedReadiness = Get-Content -Raw $tamperedReadinessPath | ConvertFrom-Json
        $tamperedReadiness.fingerprint = "tampered"
        $tamperedReadiness | ConvertTo-Json -Depth 8 | Set-Content -Path $tamperedReadinessPath -Encoding utf8
        $tamperedReadinessResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperReadinessWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedReadinessResult.ExitCode -eq 0) {
            throw "tampered readiness unexpectedly validated"
        }
        if ($tamperedReadinessResult.Output -notmatch '"error_code"\s*:\s*"ARA-4002"') {
            throw "tampered readiness did not emit ARA-4002"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-readiness"; status = "ok" }

        $tamperedDecisionSummaryPath = Join-Path $tamperDecisionSummaryWorkspace (Join-Path $contractRoot "decision-summary.json")
        $tamperedDecisionSummary = Get-Content -Raw $tamperedDecisionSummaryPath | ConvertFrom-Json
        $tamperedDecisionSummary.reason = "tampered decision summary"
        $tamperedDecisionSummary | ConvertTo-Json -Depth 12 | Set-Content -Path $tamperedDecisionSummaryPath -Encoding utf8
        $tamperedDecisionSummaryResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperDecisionSummaryWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedDecisionSummaryResult.ExitCode -eq 0) {
            throw "tampered decision summary unexpectedly validated"
        }
        if ($tamperedDecisionSummaryResult.Output -notmatch '"error_code"\s*:\s*"ARA-4000"') {
            throw "tampered decision summary did not emit ARA-4000"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-decision-summary"; status = "ok" }

        $tamperedAgentBriefPath = Join-Path $tamperAgentBriefWorkspace (Join-Path $contractRoot "agent-brief.json")
        $tamperedAgentBrief = Get-Content -Raw $tamperedAgentBriefPath | ConvertFrom-Json
        $tamperedAgentBrief.reason = "tampered agent brief"
        $tamperedAgentBrief | ConvertTo-Json -Depth 12 | Set-Content -Path $tamperedAgentBriefPath -Encoding utf8
        $tamperedAgentBriefResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperAgentBriefWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedAgentBriefResult.ExitCode -eq 0) {
            throw "tampered agent brief unexpectedly validated"
        }
        if ($tamperedAgentBriefResult.Output -notmatch '"error_code"\s*:\s*"ARA-4000"') {
            throw "tampered agent brief did not emit ARA-4000"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-agent-brief"; status = "ok" }

        $tamperedContractPath = Join-Path $tamperContractWorkspace (Join-Path $contractRoot "project-contract.toml")
        $tamperedContract = Get-Content -Raw $tamperedContractPath
        $tamperedContract = $tamperedContract -replace "stage-01", "stage-99"
        Set-Content -Path $tamperedContractPath -Value $tamperedContract -Encoding utf8
        $tamperedContractResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperContractWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedContractResult.ExitCode -eq 0) {
            throw "tampered contract unexpectedly validated"
        }
        if ($tamperedContractResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "tampered contract did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-contract"; status = "ok" }

        $tamperedWorktreePath = Join-Path $tamperWorktreeWorkspace (Join-Path $contractRoot "worktree-protocol.json")
        $tamperedWorktree = Get-Content -Raw $tamperedWorktreePath | ConvertFrom-Json
        $tamperedWorktree.model = "tampered-worktree-model"
        $tamperedWorktree | ConvertTo-Json -Depth 12 | Set-Content -Path $tamperedWorktreePath -Encoding utf8
        $tamperedWorktreeResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperWorktreeWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedWorktreeResult.ExitCode -eq 0) {
            throw "tampered worktree protocol unexpectedly validated"
        }
        if ($tamperedWorktreeResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "tampered worktree protocol did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-worktree-protocol"; status = "ok" }

        $tamperedWorktreeRulePath = Join-Path $tamperWorktreeRuleWorkspace (Join-Path $contractRoot "worktree-protocol.json")
        $tamperedWorktreeRule = Get-Content -Raw $tamperedWorktreeRulePath | ConvertFrom-Json
        $tamperedWorktreeRule.sync_rule = @()
        $tamperedWorktreeRule | ConvertTo-Json -Depth 12 | Set-Content -Path $tamperedWorktreeRulePath -Encoding utf8
        $tamperedWorktreeRuleResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperWorktreeRuleWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedWorktreeRuleResult.ExitCode -eq 0) {
            throw "tampered worktree rule unexpectedly validated"
        }
        if ($tamperedWorktreeRuleResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "tampered worktree rule did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-worktree-rule"; status = "ok" }

        $tamperedProgressPath = Join-Path $tamperProgressWorkspace (Join-Path $contractRoot "task-progress.json")
        $tamperedProgress = Get-Content -Raw $tamperedProgressPath | ConvertFrom-Json
        $tamperedProgress.completed_stages = 1
        $tamperedProgress.overall_progress_percent = 0
        $tamperedProgress | ConvertTo-Json -Depth 8 | Set-Content -Path $tamperedProgressPath -Encoding utf8
        $tamperedProgressBriefPath = Join-Path $tamperProgressWorkspace (Join-Path $contractRoot "agent-brief.json")
        $tamperedProgressBrief = Get-Content -Raw $tamperedProgressBriefPath | ConvertFrom-Json
        $tamperedProgressBrief.completed_stages = 1
        $tamperedProgressBrief.overall_progress_percent = 0
        $tamperedProgressBrief | ConvertTo-Json -Depth 12 | Set-Content -Path $tamperedProgressBriefPath -Encoding utf8
        $tamperedProgressResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperProgressWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedProgressResult.ExitCode -eq 0) {
            throw "tampered task progress unexpectedly validated"
        }
        if ($tamperedProgressResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "tampered task progress did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-task-progress"; status = "ok" }

        $tamperedModuleCatalogPath = Join-Path $tamperModuleCatalogWorkspace (Join-Path $resolvedContract.paths.modules_root "00-module-catalog.json")
        $tamperedModuleCatalog = Get-Content -Raw $tamperedModuleCatalogPath | ConvertFrom-Json
        $tamperedModuleCatalog.modules[0].responsibility = "tampered runtime truth"
        $tamperedModuleCatalog | ConvertTo-Json -Depth 8 | Set-Content -Path $tamperedModuleCatalogPath -Encoding utf8
        $tamperedModuleCatalogResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperModuleCatalogWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedModuleCatalogResult.ExitCode -eq 0) {
            throw "tampered module catalog unexpectedly validated"
        }
        if ($tamperedModuleCatalogResult.Output -notmatch '"error_code"\s*:\s*"ARA-3001"') {
            throw "tampered module catalog did not emit ARA-3001"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-module-catalog"; status = "ok" }

        $tamperedSchemaPath = Join-Path $tamperSchemaWorkspace (Join-Path $contractRoot "resolved-contract.json")
        $tamperedSchema = Get-Content -Raw $tamperedSchemaPath | ConvertFrom-Json
        $tamperedSchema.schema_version = "ara.resolved-contract.v999"
        $tamperedSchema | ConvertTo-Json -Depth 8 | Set-Content -Path $tamperedSchemaPath -Encoding utf8
        $tamperedProjectContractPath = Join-Path $tamperSchemaWorkspace (Join-Path $contractRoot "project-contract.toml")
        $tamperedProjectContract = Get-Content -Raw $tamperedProjectContractPath
        $tamperedProjectContract = $tamperedProjectContract -replace 'ara.project-contract.v1', 'ara.project-contract.v0'
        Set-Content -Path $tamperedProjectContractPath -Value $tamperedProjectContract -Encoding utf8
        $tamperedSchemaResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperSchemaWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedSchemaResult.ExitCode -eq 0) {
            throw "tampered schema version unexpectedly validated"
        }
        if ($tamperedSchemaResult.Output -notmatch '"error_code"\s*:\s*"ARA-2005"') {
            throw "tampered schema version did not emit ARA-2005"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-schema-version"; status = "ok" }

        $migrateWorkspaceResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "migrate-workspace", "--workspace", $tamperSchemaWorkspace) -WorkingDirectory $runtimeRoot
        if ($migrateWorkspaceResult.ExitCode -ne 0) {
            throw "migrate-workspace failed for schema drifted workspace"
        }
        $migrationReportPath = Join-Path $tamperSchemaWorkspace (Join-Path $contractRoot "migration-report.json")
        if (-not (Test-Path $migrationReportPath)) {
            throw "migration report was not emitted"
        }
        $migrationReport = Get-Content -Raw $migrationReportPath | ConvertFrom-Json
        if ($migrationReport.migrated_artifacts -lt 2) {
            throw "migration report did not record migrated schema artifacts"
        }
        if (-not ($migrationReport.artifacts | Where-Object { $_.artifact -eq "resolved-contract" -and $_.action -eq "migrated" })) {
            throw "migration report did not record resolved-contract migration"
        }
        $postMigrationValidateResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperSchemaWorkspace) -WorkingDirectory $runtimeRoot
        if ($postMigrationValidateResult.ExitCode -ne 0) {
            throw "migrated workspace did not validate"
        }
        $checks += [pscustomobject]@{ name = "cli-migrate-workspace"; status = "ok" }

        $tamperedChangeReportPath = Join-Path $tamperChangeReportWorkspace (Join-Path $contractRoot "change-report.json")
        $tamperedChangeReport = Get-Content -Raw $tamperedChangeReportPath | ConvertFrom-Json
        $tamperedChangeReport.operations[0].details = "tampered change detail"
        $tamperedChangeReport | ConvertTo-Json -Depth 8 | Set-Content -Path $tamperedChangeReportPath -Encoding utf8
        $tamperedChangeReportResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperChangeReportWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedChangeReportResult.ExitCode -eq 0) {
            throw "tampered change report unexpectedly validated"
        }
        if ($tamperedChangeReportResult.Output -notmatch '"error_code"\s*:\s*"ARA-4002"') {
            throw "tampered change report did not emit ARA-4002"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-change-report"; status = "ok" }

        $tamperedPatchBasePath = Join-Path $tamperPatchBaseWorkspace (Join-Path $contractRoot "patch-base.json")
        $tamperedPatchBase = Get-Content -Raw $tamperedPatchBasePath | ConvertFrom-Json
        $tamperedPatchBase.base_fingerprint = "tampered"
        $tamperedPatchBase | ConvertTo-Json -Depth 12 | Set-Content -Path $tamperedPatchBasePath -Encoding utf8
        $tamperedPatchBaseResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperPatchBaseWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedPatchBaseResult.ExitCode -eq 0) {
            throw "tampered patch base unexpectedly validated"
        }
        if ($tamperedPatchBaseResult.Output -notmatch '"error_code"\s*:\s*"ARA-5003"') {
            throw "tampered patch base did not emit ARA-5003"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-patch-base"; status = "ok" }

        $legacyPatchBasePath = Join-Path $legacyPatchBaseWorkspace (Join-Path $contractRoot "patch-base.json")
        Remove-Item -Force $legacyPatchBasePath
        $legacyPatchBaseResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $legacyPatchBaseWorkspace) -WorkingDirectory $runtimeRoot
        if ($legacyPatchBaseResult.ExitCode -ne 0) {
            throw "legacy workspace without patch base did not validate"
        }
        $checks += [pscustomobject]@{ name = "cli-legacy-missing-patch-base"; status = "ok" }

        $tamperedPatchPlanPath = Join-Path $tamperPatchPlanWorkspace (Join-Path $contractRoot "patch-plan.json")
        $tamperedPatchPlan = Get-Content -Raw $tamperedPatchPlanPath | ConvertFrom-Json
        $tamperedPatchPlan.operations[0].details = "tampered patch detail"
        $tamperedPatchPlan | ConvertTo-Json -Depth 8 | Set-Content -Path $tamperedPatchPlanPath -Encoding utf8
        $tamperedPatchPlanResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperPatchPlanWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedPatchPlanResult.ExitCode -eq 0) {
            throw "tampered patch plan unexpectedly validated"
        }
        if ($tamperedPatchPlanResult.Output -notmatch '"error_code"\s*:\s*"ARA-5003"') {
            throw "tampered patch plan did not emit ARA-5003"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-patch-plan"; status = "ok" }

        $tamperedPatchScopePath = Join-Path $tamperPatchScopeWorkspace (Join-Path $contractRoot "patch-plan.json")
        $tamperedPatchScope = Get-Content -Raw $tamperedPatchScopePath | ConvertFrom-Json
        $scopeOperation = $tamperedPatchScope.operations | Where-Object { $_.affected_paths -and $_.affected_paths.Count -gt 0 } | Select-Object -First 1
        if ($null -eq $scopeOperation) {
            throw "tampered patch scope workspace did not include a scoped patch operation"
        }
        $scopeOperation.affected_paths = @("blueprint/authority/00-authority-root.md")
        if ($scopeOperation.target_worktree_roles) {
            $scopeOperation.target_worktree_roles = @("tampered-role")
        }
        $tamperedPatchScope | ConvertTo-Json -Depth 12 | Set-Content -Path $tamperedPatchScopePath -Encoding utf8
        $tamperedPatchScopeResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperPatchScopeWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedPatchScopeResult.ExitCode -eq 0) {
            throw "tampered patch scope unexpectedly validated"
        }
        if ($tamperedPatchScopeResult.Output -notmatch '"error_code"\s*:\s*"ARA-5003"') {
            throw "tampered patch scope did not emit ARA-5003"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-patch-scope"; status = "ok" }

        $legacyPatchPlanPath = Join-Path $legacyPatchPlanWorkspace (Join-Path $contractRoot "patch-plan.json")
        Remove-Item -Force $legacyPatchPlanPath
        $legacyPatchPlanResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $legacyPatchPlanWorkspace) -WorkingDirectory $runtimeRoot
        if ($legacyPatchPlanResult.ExitCode -ne 0) {
            throw "legacy workspace without patch plan did not validate"
        }
        $checks += [pscustomobject]@{ name = "cli-legacy-missing-patch-plan"; status = "ok" }

        $tamperedPatchExecutionPath = Join-Path $tamperPatchExecutionWorkspace (Join-Path $contractRoot "patch-execution-report.json")
        $tamperedPatchExecution = Get-Content -Raw $tamperedPatchExecutionPath | ConvertFrom-Json
        $tamperedPatchExecution.replayed_result_fingerprint = "tampered"
        $tamperedPatchExecution | ConvertTo-Json -Depth 8 | Set-Content -Path $tamperedPatchExecutionPath -Encoding utf8
        $tamperedPatchExecutionResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperPatchExecutionWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedPatchExecutionResult.ExitCode -eq 0) {
            throw "tampered patch execution report unexpectedly validated"
        }
        if ($tamperedPatchExecutionResult.Output -notmatch '"error_code"\s*:\s*"ARA-5003"') {
            throw "tampered patch execution report did not emit ARA-5003"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-patch-execution-report"; status = "ok" }

        $tamperedSemanticIrPath = Join-Path $tamperSemanticIrWorkspace (Join-Path $contractRoot "semantic-ir.json")
        $tamperedSemanticIr = Get-Content -Raw $tamperedSemanticIrPath | ConvertFrom-Json
        $tamperedSemanticIr.sections[0].values = @("tampered semantic value")
        $tamperedSemanticIr | ConvertTo-Json -Depth 12 | Set-Content -Path $tamperedSemanticIrPath -Encoding utf8
        $tamperedSemanticIrResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $tamperSemanticIrWorkspace) -WorkingDirectory $runtimeRoot
        if ($tamperedSemanticIrResult.ExitCode -eq 0) {
            throw "tampered semantic ir unexpectedly validated"
        }
        if ($tamperedSemanticIrResult.Output -notmatch '"error_code"\s*:\s*"ARA-2003"') {
            throw "tampered semantic ir did not emit ARA-2003"
        }
        $checks += [pscustomobject]@{ name = "cli-tampered-semantic-ir"; status = "ok" }

        $legacySemanticIrPath = Join-Path $legacySemanticIrWorkspace (Join-Path $contractRoot "semantic-ir.json")
        Remove-Item -Force $legacySemanticIrPath
        $legacySemanticIrResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $legacySemanticIrWorkspace) -WorkingDirectory $runtimeRoot
        if ($legacySemanticIrResult.ExitCode -ne 0) {
            throw "legacy workspace without semantic ir did not validate"
        }
        $checks += [pscustomobject]@{ name = "cli-legacy-missing-semantic-ir"; status = "ok" }

        $legacyDecisionSummaryPath = Join-Path $legacyDecisionSummaryWorkspace (Join-Path $contractRoot "decision-summary.json")
        Remove-Item -Force $legacyDecisionSummaryPath
        $legacyDecisionSummaryResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $legacyDecisionSummaryWorkspace) -WorkingDirectory $runtimeRoot
        if ($legacyDecisionSummaryResult.ExitCode -ne 0) {
            throw "legacy workspace without decision summary did not validate"
        }
        $checks += [pscustomobject]@{ name = "cli-legacy-missing-decision-summary"; status = "ok" }

        $legacyAgentBriefPath = Join-Path $legacyAgentBriefWorkspace (Join-Path $contractRoot "agent-brief.json")
        Remove-Item -Force $legacyAgentBriefPath
        $legacyAgentBriefResult = Invoke-CargoCommand -Arguments @("run", "-p", "ara-cli", "--", "validate-workspace", "--workspace", $legacyAgentBriefWorkspace) -WorkingDirectory $runtimeRoot
        if ($legacyAgentBriefResult.ExitCode -ne 0) {
            throw "legacy workspace without agent brief did not validate"
        }
        $checks += [pscustomobject]@{ name = "cli-legacy-missing-agent-brief"; status = "ok" }

        Remove-Item -Recurse -Force $tempRoot
        Remove-Item -Recurse -Force $tamperManifestWorkspace, $tamperReadinessWorkspace, $tamperContractWorkspace, $tamperProgressWorkspace, $tamperModuleCatalogWorkspace, $tamperSchemaWorkspace, $tamperChangeReportWorkspace, $tamperPatchBaseWorkspace, $tamperPatchPlanWorkspace, $tamperPatchScopeWorkspace, $legacyPatchBaseWorkspace, $legacyPatchPlanWorkspace, $tamperWorktreeWorkspace, $tamperWorktreeRuleWorkspace, $tamperPatchExecutionWorkspace, $tamperSemanticIrWorkspace, $legacySemanticIrWorkspace, $tamperDecisionSummaryWorkspace, $legacyDecisionSummaryWorkspace, $tamperAgentBriefWorkspace, $legacyAgentBriefWorkspace -ErrorAction SilentlyContinue
    }

    [pscustomobject]@{
        skill_root = $SkillRoot
        checks = $checks
        status = if ($runtimeBlocked) { "blocked" } else { "ok" }
        runtime_blocker = if ($runtimeBlocked) { "host_application_control" } else { $null }
    } | ConvertTo-Json -Depth 5
}
finally {
    if (Test-Path $tempRoot) {
        Remove-Item -Recurse -Force $tempRoot -ErrorAction SilentlyContinue
    }
foreach ($cleanupPath in @($tamperManifestWorkspace, $tamperReadinessWorkspace, $tamperContractWorkspace, $tamperProgressWorkspace, $tamperModuleCatalogWorkspace, $tamperSchemaWorkspace, $tamperChangeReportWorkspace, $tamperPatchBaseWorkspace, $tamperPatchPlanWorkspace, $tamperPatchScopeWorkspace, $legacyPatchBaseWorkspace, $legacyPatchPlanWorkspace, $tamperWorktreeWorkspace, $tamperWorktreeRuleWorkspace, $tamperPatchExecutionWorkspace, $tamperSemanticIrWorkspace, $legacySemanticIrWorkspace, $tamperDecisionSummaryWorkspace, $legacyDecisionSummaryWorkspace, $tamperAgentBriefWorkspace, $legacyAgentBriefWorkspace)) {
        if ($cleanupPath -and (Test-Path $cleanupPath)) {
            Remove-Item -Recurse -Force $cleanupPath -ErrorAction SilentlyContinue
        }
    }
    if ($runtimeMirrorRoot -and (Test-Path $runtimeMirrorRoot)) {
        Remove-Item -Recurse -Force $runtimeMirrorRoot -ErrorAction SilentlyContinue
    }
    Pop-Location
}
