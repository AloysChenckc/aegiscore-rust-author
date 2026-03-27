use ara_runtime::{
    fingerprint_text, normalize_repo_relative_path, prune_directory_files, read_utf8,
    read_workspace_blueprint_bundle_with_roots, to_pretty_json, write_utf8_atomic,
};
use ara_schemas::{
    AgentBrief, AuthorMode, BlueprintManifest, BlueprintManifestEntry, ChangeOperation,
    ChangeReport, CliOutput, ContractPaths, ContractStage, DecisionSummary,
    DecisionSummaryEntry, ErrorCode, MigrationArtifact, MigrationReport, ModuleCatalog,
    ModuleLanguage, ModuleSpec, NormalizationReport, PatchBase, PatchExecutionReport,
    PatchOperation, PatchPlan, ReadinessReport, ResolvedContract, SemanticCluster,
    SemanticConflict, SemanticConflictSourceGroup, SemanticFrame, SemanticIr, SemanticSection,
    SemanticStage, TaskProgressReport, TaskProgressStage, TaskStageStatus, WorktreeProtocol,
    WorktreeRoleSpec, AGENT_BRIEF_SCHEMA_VERSION, BLUEPRINT_MANIFEST_SCHEMA_VERSION,
    CHANGE_REPORT_SCHEMA_VERSION, DECISION_SUMMARY_SCHEMA_VERSION, MIGRATION_REPORT_SCHEMA_VERSION,
    MODULE_CATALOG_SCHEMA_VERSION, NORMALIZATION_REPORT_SCHEMA_VERSION,
    PATCH_BASE_SCHEMA_VERSION, PATCH_EXECUTION_REPORT_SCHEMA_VERSION,
    PATCH_PLAN_SCHEMA_VERSION, READINESS_SCHEMA_VERSION, RESOLVED_CONTRACT_SCHEMA_VERSION,
    SEMANTIC_IR_SCHEMA_VERSION, TASK_PROGRESS_SCHEMA_VERSION, WORKTREE_PROTOCOL_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use thiserror::Error;

const DEFAULT_PROJECT_CONTRACT_TEMPLATE: &str =
    include_str!("../../../defaults/project-contract.toml");
const DEFAULT_MODULE_LANGUAGE_POLICY: &str =
    include_str!("../../../defaults/module-language-policy.toml");
const DEFAULT_BLUEPRINT_POLICY: &str = include_str!("../../../defaults/blueprint-policy.toml");
const DEFAULT_NORMALIZATION_POLICY: &str =
    include_str!("../../../defaults/normalization-policy.toml");

#[derive(Debug, Clone)]
pub struct BlueprintAuthorInput {
    pub project_name: String,
    pub source_summary: String,
    pub source_text: Option<String>,
    pub workspace_source_text: Option<String>,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BlueprintPackage {
    pub mode: AuthorMode,
    pub project_name: String,
    pub source_provenance: String,
    pub authority_doc: String,
    pub workflow_doc: String,
    pub stage_documents: Vec<StageDocument>,
    pub module_policy_doc: String,
    pub author_report_markdown: String,
    pub project_contract_toml: String,
    pub module_catalog: ModuleCatalog,
    pub manifest: BlueprintManifest,
    pub worktree_protocol: WorktreeProtocol,
    pub semantic_ir: SemanticIr,
    pub normalization_report: NormalizationReport,
    pub change_report: ChangeReport,
    pub patch_base: PatchBase,
    pub patch_plan: PatchPlan,
    pub patch_execution_report: PatchExecutionReport,
    pub decision_summary: DecisionSummary,
    pub agent_brief: AgentBrief,
    pub readiness: ReadinessReport,
    pub task_progress: TaskProgressReport,
    pub resolved_contract: ResolvedContract,
}

#[derive(Debug, Clone)]
pub struct StageDocument {
    pub path: String,
    pub stage_id: String,
    pub content: String,
}

#[derive(Debug, Clone)]
struct NormalizedBlueprint {
    sections: BTreeMap<String, Vec<String>>,
    stage_sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    base_source: Option<ParsedBlueprintSource>,
    semantic_frames: Vec<SemanticFrame>,
    source_files: Vec<String>,
    preserved_sections: Vec<String>,
    inferred_sections: Vec<String>,
    dropped_sections: Vec<String>,
    semantic_hints: Vec<String>,
    semantic_risks: Vec<String>,
    semantic_conflicts: Vec<SemanticConflict>,
    unresolved_ambiguities: Vec<String>,
    change_operations: Vec<ChangeOperation>,
    patch_operations: Vec<PatchOperation>,
    source_provenance: String,
}

#[derive(Debug, Clone, Default)]
struct ParsedBlueprintSource {
    sections: BTreeMap<String, Vec<String>>,
    stage_sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    semantic_frames: Vec<SemanticFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReadinessGateSummary {
    blocking_semantic_conflict_count: usize,
    review_required_semantic_conflict_count: usize,
    high_risk_patch_operation_count: usize,
    review_required_patch_operation_count: usize,
    gate_holds: Vec<String>,
    recommended_actions: Vec<String>,
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("project name cannot be empty")]
    EmptyProjectName,
    #[error("manifest cannot be empty")]
    EmptyManifest,
    #[error("invalid module language assignment for module {module_id}")]
    InvalidModuleLanguage { module_id: String },
    #[error("module catalog does not match embedded defaults for module {module_id}")]
    ModuleCatalogMismatch { module_id: String },
    #[error("manifest is missing expected entry for {path}")]
    MissingManifestEntry { path: String },
    #[error("manifest contains unexpected entry for {path}")]
    UnexpectedManifestEntry { path: String },
    #[error("manifest path must use forward slashes: {path}")]
    NonNormalizedManifestPath { path: String },
    #[error("manifest fingerprint does not match package content for {path}")]
    ManifestFingerprintMismatch { path: String },
    #[error("external blueprint is too ambiguous to normalize: {reason}")]
    TooAmbiguousToNormalize { reason: String },
    #[error("update would rewrite authority truth in section `{section}`")]
    AuthorityConflict { section: String },
    #[error("workspace blueprint source is missing or incomplete: {workspace}")]
    MissingWorkspaceBlueprint { workspace: String },
    #[error("source format is unsupported by normalization policy: {input_source}")]
    UnsupportedImportSourceFormat { input_source: String },
    #[error("stage graph is invalid: {reason}")]
    InvalidStageGraph { reason: String },
    #[error("schema version mismatch for {artifact}: expected {expected}, found {found}")]
    SchemaVersionMismatch {
        artifact: String,
        expected: String,
        found: String,
    },
    #[error("resolved contract is inconsistent: {field}")]
    ResolvedContractInconsistent { field: String },
    #[error("normalization evidence is incomplete: {field}")]
    NormalizationEvidenceMissing { field: String },
    #[error("change report is inconsistent: {field}")]
    ChangeReportInconsistent { field: String },
    #[error("patch base is unavailable for standalone patch apply: {reason}")]
    PatchBaseUnavailable { reason: String },
    #[error("readiness report is inconsistent: {field}")]
    ReadinessInconsistent { field: String },
    #[error("readiness fingerprint does not match package content")]
    ReadinessFingerprintMismatch,
    #[error("package is not ready for blueprint gate: {state}")]
    PackageNotReady { state: String },
    #[error("runtime error: {0}")]
    Runtime(#[from] ara_runtime::RuntimeError),
    #[error("toml serialization error: {0}")]
    Toml(#[from] toml::ser::Error),
    #[error("toml parse error: {0}")]
    TomlDe(#[from] toml::de::Error),
}

impl CoreError {
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::EmptyProjectName => ErrorCode::BlueprintSchemaIncomplete,
            Self::EmptyManifest => ErrorCode::ManifestGenerationFailed,
            Self::InvalidModuleLanguage { .. } => ErrorCode::ModuleLanguagePolicyViolation,
            Self::ModuleCatalogMismatch { .. } => ErrorCode::ModuleLanguagePolicyViolation,
            Self::MissingManifestEntry { .. } => ErrorCode::RequiredOutputMissing,
            Self::UnexpectedManifestEntry { .. } => ErrorCode::ArtifactFingerprintMismatch,
            Self::NonNormalizedManifestPath { .. } => ErrorCode::ArtifactFingerprintMismatch,
            Self::ManifestFingerprintMismatch { .. } => ErrorCode::ArtifactFingerprintMismatch,
            Self::TooAmbiguousToNormalize { .. } => ErrorCode::ExternalBlueprintTooAmbiguous,
            Self::AuthorityConflict { .. } => ErrorCode::AuthorityRewriteRequired,
            Self::MissingWorkspaceBlueprint { .. } => ErrorCode::UnsupportedSourceType,
            Self::UnsupportedImportSourceFormat { .. } => ErrorCode::UnsupportedSourceType,
            Self::InvalidStageGraph { .. } => ErrorCode::StageGraphInvalid,
            Self::SchemaVersionMismatch { .. } => ErrorCode::SchemaVersionMismatch,
            Self::ResolvedContractInconsistent { .. } => ErrorCode::ResolvedContractInconsistent,
            Self::NormalizationEvidenceMissing { .. } => ErrorCode::NormalizationEvidenceMissing,
            Self::ChangeReportInconsistent { .. } => ErrorCode::ChangeReportInvalid,
            Self::PatchBaseUnavailable { .. } => ErrorCode::RequiredOutputMissing,
            Self::ReadinessInconsistent { .. } => ErrorCode::ReadinessCalculationFailed,
            Self::ReadinessFingerprintMismatch => ErrorCode::PackageFingerprintStale,
            Self::PackageNotReady { .. } => ErrorCode::PackageNotReadyForBlueprintGate,
            Self::Runtime(error) => match error {
                ara_runtime::RuntimeError::MissingParent(_) => ErrorCode::PathResolutionFailed,
                ara_runtime::RuntimeError::MissingInputPath(_) => ErrorCode::UnsupportedSourceType,
                ara_runtime::RuntimeError::Io(_) => ErrorCode::RuntimeFailure,
                ara_runtime::RuntimeError::Json(_) => ErrorCode::ManifestGenerationFailed,
                ara_runtime::RuntimeError::TimeFormat(_) => ErrorCode::RuntimeFailure,
            },
            Self::Toml(_) | Self::TomlDe(_) => ErrorCode::ContractTomlInvalid,
        }
    }

    pub fn details(&self) -> Vec<String> {
        match self {
            Self::InvalidModuleLanguage { module_id } => vec![format!("module_id={module_id}")],
            Self::ModuleCatalogMismatch { module_id } => vec![format!("module_id={module_id}")],
            Self::MissingManifestEntry { path } => vec![format!("path={path}")],
            Self::UnexpectedManifestEntry { path } => vec![format!("path={path}")],
            Self::NonNormalizedManifestPath { path } => vec![format!("path={path}")],
            Self::ManifestFingerprintMismatch { path } => vec![format!("path={path}")],
            Self::TooAmbiguousToNormalize { reason } => vec![reason.clone()],
            Self::AuthorityConflict { section } => vec![format!("section={section}")],
            Self::MissingWorkspaceBlueprint { workspace } => vec![format!("workspace={workspace}")],
            Self::UnsupportedImportSourceFormat { input_source } => {
                vec![format!("source={input_source}")]
            }
            Self::InvalidStageGraph { reason } => vec![reason.clone()],
            Self::SchemaVersionMismatch {
                artifact,
                expected,
                found,
            } => vec![
                format!("artifact={artifact}"),
                format!("expected={expected}"),
                format!("found={found}"),
            ],
            Self::ResolvedContractInconsistent { field } => vec![format!("field={field}")],
            Self::NormalizationEvidenceMissing { field } => vec![format!("field={field}")],
            Self::ChangeReportInconsistent { field } => vec![format!("field={field}")],
            Self::PatchBaseUnavailable { reason } => vec![reason.clone()],
            Self::ReadinessInconsistent { field } => vec![format!("field={field}")],
            Self::ReadinessFingerprintMismatch => Vec::new(),
            Self::PackageNotReady { state } => vec![format!("state={state}")],
            Self::Runtime(error) => vec![error.to_string()],
            Self::Toml(error) => vec![error.to_string()],
            Self::TomlDe(error) => vec![error.to_string()],
            Self::EmptyProjectName | Self::EmptyManifest => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProjectContractToml {
    schema_version: String,
    project_name: String,
    workflow_mode: String,
    paths: ContractPaths,
    stages: Vec<ContractStage>,
    #[serde(default)]
    worktree: WorktreeProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ModuleLanguagePolicyToml {
    schema_version: String,
    rules: Vec<ModuleLanguageRuleToml>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ModuleLanguageRuleToml {
    layer: String,
    recommended_language: ModuleLanguage,
    allowed_languages: Vec<ModuleLanguage>,
    forbidden_languages: Vec<ModuleLanguage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct BlueprintPolicyToml {
    schema_version: String,
    default_mode: String,
    emit_machine_outputs: bool,
    stop_at_readiness: String,
    require_module_language_assignments: bool,
    require_machine_contract: bool,
    package: BlueprintPackagePolicyToml,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct BlueprintPackagePolicyToml {
    blueprint_root: String,
    contract_root: String,
    manifest_file: String,
    author_report_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct NormalizationPolicyToml {
    schema_version: String,
    preserve_source_provenance: bool,
    allow_authority_rewrite: bool,
    allow_implicit_stage_creation: bool,
    require_deterministic_stage_ids: bool,
    require_unresolved_ambiguities_report: bool,
    import: NormalizationImportPolicyToml,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct NormalizationImportPolicyToml {
    supported_sources: Vec<String>,
    normalize_paths_to_forward_slashes: bool,
}

pub fn build_package(
    mode: AuthorMode,
    input: &BlueprintAuthorInput,
) -> Result<BlueprintPackage, CoreError> {
    if input.project_name.trim().is_empty() {
        return Err(CoreError::EmptyProjectName);
    }

    let project_name = input.project_name.trim().to_string();
    let contract_template = default_project_contract_template()?;
    let module_language_policy = default_module_language_policy()?;
    let blueprint_policy = default_blueprint_policy()?;
    let normalization_policy = default_normalization_policy()?;
    validate_source_inputs(mode, input, &normalization_policy)?;
    let normalized = normalize_blueprint(mode, input, &normalization_policy)?;
    let stages = build_stages(
        normalized.sections.get("stage_order"),
        mode,
        &normalization_policy,
    )?;
    let paths = ContractPaths {
        blueprint_root: blueprint_policy.package.blueprint_root.clone(),
        authority_root: format!("{}/authority", blueprint_policy.package.blueprint_root),
        workflow_root: format!("{}/workflow", blueprint_policy.package.blueprint_root),
        stages_root: format!("{}/stages", blueprint_policy.package.blueprint_root),
        modules_root: format!("{}/modules", blueprint_policy.package.blueprint_root),
        contract_root: blueprint_policy.package.contract_root.clone(),
    };

    let mut module_catalog = ModuleCatalog {
        schema_version: MODULE_CATALOG_SCHEMA_VERSION.to_string(),
        modules: default_modules(&module_language_policy)?,
    };

    let current_source = ParsedBlueprintSource {
        sections: normalized.sections.clone(),
        stage_sections: normalized.stage_sections.clone(),
        semantic_frames: normalized.semantic_frames.clone(),
    };
    let base_source = normalized
        .base_source
        .clone()
        .unwrap_or_else(|| current_source.clone());

    let stage_documents = build_stage_documents(
        &normalized.sections,
        &normalized.stage_sections,
        &stages,
        &paths.stages_root,
    );
    let worktree_protocol = build_worktree_protocol(
        &normalized.sections,
        &stages,
        &stage_documents,
        &module_catalog,
        &paths,
    )?;
    apply_worktree_bindings_to_module_catalog(&mut module_catalog, &worktree_protocol);
    validate_module_catalog(
        &module_catalog,
        &module_language_policy,
        Some(&worktree_protocol),
    )?;
    let workflow_sections =
        sections_with_worktree_protocol(&normalized.sections, &worktree_protocol);

    let resolved_contract = ResolvedContract {
        schema_version: RESOLVED_CONTRACT_SCHEMA_VERSION.to_string(),
        project_name: project_name.clone(),
        workflow_mode: contract_template.workflow_mode.clone(),
        paths: paths.clone(),
        stages: stages.clone(),
        worktree: worktree_protocol.clone(),
    };

    let project_contract_toml = toml::to_string_pretty(&ProjectContractToml {
        schema_version: contract_template.schema_version.clone(),
        project_name: project_name.clone(),
        workflow_mode: contract_template.workflow_mode.clone(),
        paths: paths.clone(),
        stages: stages.clone(),
        worktree: worktree_protocol.clone(),
    })?;

    let authority_doc = render_authority_doc(input, &normalized.sections);
    let workflow_doc = render_workflow_doc(&workflow_sections, &stages, &worktree_protocol);
    let module_policy_doc = render_module_policy_doc(&module_catalog);
    let module_catalog_json = to_pretty_json(&module_catalog)?;
    let rendered_projection_source = parse_workspace_source(&workspace_bundle_text_from_docs(
        &format!("{}/00-authority-root.md", paths.authority_root),
        &authority_doc,
        &format!("{}/00-workflow-overview.md", paths.workflow_root),
        &workflow_doc,
        &stage_documents,
    ));
    let semantic_ir = build_semantic_ir(
        mode,
        &project_name,
        &normalized.source_provenance,
        mode.source_type(),
        "source-first-normalized-projection",
        &normalized.source_files,
        &normalized.preserved_sections,
        &normalized.inferred_sections,
        &current_source,
        &rendered_projection_source,
        &stages,
        &normalized.semantic_hints,
        &normalized.semantic_risks,
        &normalized.semantic_conflicts,
        &normalized.unresolved_ambiguities,
        &normalized.semantic_frames,
    );
    let mut hydrated_patch_operations = normalized.patch_operations.clone();
    hydrate_patch_plan_with_base_state(
        &mut hydrated_patch_operations,
        &base_source.sections,
        &base_source.stage_sections,
    );
    hydrate_patch_plan_from_state(
        &mut hydrated_patch_operations,
        &current_source.sections,
        &current_source.stage_sections,
    );
    hydrate_patch_operation_worktree_scope(
        &mut hydrated_patch_operations,
        &resolved_contract,
        &worktree_protocol,
        &stage_documents,
    )?;

    let mut manifest_files = vec![
        manifest_entry(
            &format!("{}/00-authority-root.md", paths.authority_root),
            "authority",
            "authority-root",
            &normalized.source_provenance,
            &authority_doc,
        ),
        manifest_entry(
            &format!("{}/00-workflow-overview.md", paths.workflow_root),
            "workflow",
            "workflow-overview",
            &normalized.source_provenance,
            &workflow_doc,
        ),
        manifest_entry(
            &format!("{}/01-language-policy.md", paths.modules_root),
            "module-policy",
            "module-language-policy",
            &normalized.source_provenance,
            &module_policy_doc,
        ),
        manifest_entry(
            &format!("{}/00-module-catalog.json", paths.modules_root),
            "module-catalog",
            "module-catalog",
            &normalized.source_provenance,
            &module_catalog_json,
        ),
    ];
    for stage_document in &stage_documents {
        manifest_files.push(manifest_entry(
            &stage_document.path,
            "stage",
            &stage_document.stage_id,
            &normalized.source_provenance,
            &stage_document.content,
        ));
    }
    let manifest = BlueprintManifest {
        schema_version: BLUEPRINT_MANIFEST_SCHEMA_VERSION.to_string(),
        files: manifest_files,
    };

    let normalization_report = NormalizationReport {
        schema_version: NORMALIZATION_REPORT_SCHEMA_VERSION.to_string(),
        source_type: mode.source_type().to_string(),
        source_files: normalized.source_files.clone(),
        preserved_sections: normalized.preserved_sections.clone(),
        inferred_sections: normalized.inferred_sections.clone(),
        dropped_sections: normalized.dropped_sections.clone(),
        semantic_hints: normalized.semantic_hints.clone(),
        semantic_risks: normalized.semantic_risks.clone(),
        semantic_conflicts: normalized.semantic_conflicts.clone(),
        unresolved_ambiguities: normalized.unresolved_ambiguities.clone(),
        status: if matches!(
            mode,
            AuthorMode::ImportBlueprint | AuthorMode::UpdateBlueprint
        ) {
            "normalized".to_string()
        } else {
            "generated".to_string()
        },
    };
    let change_report = ChangeReport {
        schema_version: CHANGE_REPORT_SCHEMA_VERSION.to_string(),
        mode: mode.as_str().to_string(),
        operation_count: normalized.change_operations.len(),
        conflict_count: normalized
            .change_operations
            .iter()
            .filter(|operation| operation.action == "retained-conflict")
            .count(),
        operations: normalized.change_operations.clone(),
        patch_operation_count: hydrated_patch_operations.len(),
        patch_operations: hydrated_patch_operations.clone(),
    };
    let patch_plan = build_patch_plan(
        mode,
        &hydrated_patch_operations,
        &base_source,
        &current_source,
    );
    let patch_base = build_patch_base(mode, &base_source);
    let patch_execution_report = build_patch_execution_report(
        mode,
        &patch_plan,
        &base_source,
        &resolved_contract,
        &worktree_protocol,
        &stage_documents,
    )?;

    let manifest_fingerprint = package_manifest_fingerprint(&manifest);
    let readiness_summary = summarize_readiness_gate_holds(&normalization_report, &patch_plan);
    let mut readiness = ReadinessReport {
        schema_version: READINESS_SCHEMA_VERSION.to_string(),
        state: blueprint_policy.stop_at_readiness.clone(),
        reason: readiness_reason(&readiness_summary),
        fingerprint: String::new(),
        blocking_semantic_conflict_count: readiness_summary.blocking_semantic_conflict_count,
        review_required_semantic_conflict_count: readiness_summary
            .review_required_semantic_conflict_count,
        high_risk_patch_operation_count: readiness_summary.high_risk_patch_operation_count,
        review_required_patch_operation_count: readiness_summary
            .review_required_patch_operation_count,
        gate_holds: readiness_summary.gate_holds.clone(),
        recommended_actions: readiness_summary.recommended_actions.clone(),
    };
    let decision_summary = build_decision_summary(
        mode,
        &readiness,
        &normalization_report,
        &patch_plan,
        &resolved_contract,
        &worktree_protocol,
        &stage_documents,
    )?;
    let task_progress = build_task_progress(&resolved_contract.stages);
    let agent_brief = build_agent_brief(
        &project_name,
        mode,
        &readiness,
        &task_progress,
        &decision_summary,
    );
    let readiness_fingerprint = readiness_fingerprint_for(
        &project_name,
        mode,
        &normalized.source_provenance,
        &manifest_fingerprint,
        &resolved_contract,
        &worktree_protocol,
        &normalization_report,
        &change_report,
        &patch_plan,
        &patch_execution_report,
        &decision_summary,
        &agent_brief,
    );
    readiness.fingerprint = readiness_fingerprint.clone();

    let package = BlueprintPackage {
        mode,
        project_name,
        source_provenance: normalized.source_provenance.clone(),
        authority_doc,
        workflow_doc,
        stage_documents,
        module_policy_doc,
        author_report_markdown: render_author_report(
            mode,
            input,
            &normalization_report,
            &change_report,
            &module_catalog,
            &readiness,
            &task_progress,
        ),
        project_contract_toml,
        module_catalog,
        manifest,
        worktree_protocol,
        semantic_ir,
        normalization_report,
        change_report,
        patch_base,
        patch_plan,
        patch_execution_report,
        decision_summary,
        agent_brief,
        readiness,
        task_progress,
        resolved_contract,
    };

    validate_package(&package)?;
    Ok(package)
}

pub fn validate_package(package: &BlueprintPackage) -> Result<(), CoreError> {
    let module_language_policy = default_module_language_policy()?;
    validate_module_catalog(
        &package.module_catalog,
        &module_language_policy,
        Some(&package.worktree_protocol),
    )?;
    let blueprint_policy = default_blueprint_policy()?;
    let normalization_policy = default_normalization_policy()?;
    if package.manifest.files.is_empty() {
        return Err(CoreError::EmptyManifest);
    }
    if package.stage_documents.len() != package.resolved_contract.stages.len() {
        return Err(CoreError::InvalidStageGraph {
            reason: "stage document count does not match resolved contract stage count".to_string(),
        });
    }

    let required_paths = [
        format!(
            "{}/00-authority-root.md",
            package.resolved_contract.paths.authority_root
        ),
        format!(
            "{}/00-workflow-overview.md",
            package.resolved_contract.paths.workflow_root
        ),
        format!(
            "{}/01-language-policy.md",
            package.resolved_contract.paths.modules_root
        ),
        format!(
            "{}/00-module-catalog.json",
            package.resolved_contract.paths.modules_root
        ),
    ];
    let mut seen_manifest_paths = BTreeSet::new();
    let mut seen_stage_document_paths = BTreeSet::new();
    let expected_manifest_entries = expected_manifest_entries(package)?;
    let expected_manifest_by_path = expected_manifest_entries
        .iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();

    for path in &required_paths {
        if !package
            .manifest
            .files
            .iter()
            .any(|entry| entry.path == *path)
        {
            return Err(CoreError::MissingManifestEntry {
                path: path.to_string(),
            });
        }
    }

    for stage_document in &package.stage_documents {
        if !seen_stage_document_paths.insert(stage_document.path.clone()) {
            return Err(CoreError::InvalidStageGraph {
                reason: format!("duplicate stage document path `{}`", stage_document.path),
            });
        }
        if !package.manifest.files.iter().any(|entry| {
            entry.path == stage_document.path && entry.canonical_id == stage_document.stage_id
        }) {
            return Err(CoreError::MissingManifestEntry {
                path: stage_document.path.clone(),
            });
        }
    }

    for entry in &package.manifest.files {
        if !seen_manifest_paths.insert(entry.path.clone()) {
            return Err(CoreError::InvalidStageGraph {
                reason: format!("duplicate manifest path `{}`", entry.path),
            });
        }
        if entry.path.contains('\\') {
            return Err(CoreError::NonNormalizedManifestPath {
                path: entry.path.clone(),
            });
        }
        let Some(expected_entry) = expected_manifest_by_path.get(&entry.path) else {
            return Err(CoreError::UnexpectedManifestEntry {
                path: entry.path.clone(),
            });
        };
        if entry != *expected_entry {
            return Err(CoreError::ManifestFingerprintMismatch {
                path: entry.path.clone(),
            });
        }
    }

    for expected_entry in &expected_manifest_entries {
        if !package
            .manifest
            .files
            .iter()
            .any(|entry| entry.path == expected_entry.path)
        {
            return Err(CoreError::MissingManifestEntry {
                path: expected_entry.path.clone(),
            });
        }
    }

    if package.normalization_report.source_files.is_empty() {
        return Err(CoreError::NormalizationEvidenceMissing {
            field: "source_files".to_string(),
        });
    }
    if package
        .normalization_report
        .source_files
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err(CoreError::NormalizationEvidenceMissing {
            field: "source_files".to_string(),
        });
    }
    if package.normalization_report.status.trim().is_empty() {
        return Err(CoreError::NormalizationEvidenceMissing {
            field: "status".to_string(),
        });
    }
    if package
        .normalization_report
        .semantic_hints
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err(CoreError::NormalizationEvidenceMissing {
            field: "semantic_hints".to_string(),
        });
    }
    if package
        .normalization_report
        .semantic_risks
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err(CoreError::NormalizationEvidenceMissing {
            field: "semantic_risks".to_string(),
        });
    }
    if package
        .normalization_report
        .semantic_conflicts
        .iter()
        .any(|conflict| {
            conflict.scope.trim().is_empty()
                || conflict.canonical_section.trim().is_empty()
                || conflict.conflict_kind.trim().is_empty()
                || !is_valid_semantic_conflict_severity(&conflict.severity)
                || conflict.recommended_action.trim().is_empty()
                || (conflict.blocking && !conflict.review_required)
                || conflict.source_groups.is_empty()
                || conflict
                    .source_groups
                    .iter()
                    .any(|group| group.source_group.trim().is_empty())
        })
    {
        return Err(CoreError::NormalizationEvidenceMissing {
            field: "semantic_conflicts".to_string(),
        });
    }
    validate_readiness_alignment(
        &package.readiness,
        &package.normalization_report,
        &package.patch_plan,
    )?;
    validate_decision_summary_alignment(
        &package.decision_summary,
        package.mode,
        &package.readiness,
        &package.normalization_report,
        &package.patch_plan,
        &package.resolved_contract,
        &package.worktree_protocol,
        &package.stage_documents,
    )?;
    validate_agent_brief_alignment(
        &package.agent_brief,
        &package.project_name,
        package.mode,
        &package.readiness,
        &package.task_progress,
        &package.decision_summary,
    )?;
    validate_change_report_alignment(&package.change_report, package.mode)?;
    validate_patch_base_alignment(&package.patch_base, &package.patch_plan, package.mode)?;
    validate_patch_plan_alignment(&package.patch_plan, &package.change_report, package.mode)?;
    validate_patch_execution_report_alignment(
        &package.patch_execution_report,
        &package.patch_plan,
        package.mode,
        package,
    )?;
    validate_semantic_ir_alignment(package)?;

    if package.readiness.state != blueprint_policy.stop_at_readiness {
        return Err(CoreError::PackageNotReady {
            state: package.readiness.state.clone(),
        });
    }
    if package.source_provenance.trim().is_empty() {
        return Err(CoreError::NormalizationEvidenceMissing {
            field: "source_provenance".to_string(),
        });
    }

    let parsed_contract = toml::from_str::<ProjectContractToml>(&package.project_contract_toml)?;
    validate_schema_versions(package, &parsed_contract)?;
    validate_contract_alignment(&parsed_contract, &package.resolved_contract)?;
    validate_worktree_protocol_alignment(
        &package.worktree_protocol,
        &package.resolved_contract,
        &package.module_catalog,
    )?;
    if !collect_patch_operation_scope_mismatches(
        &package.patch_plan.operations,
        &package.resolved_contract,
        &package.worktree_protocol,
        &package.stage_documents,
    )?
    .is_empty()
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.worktree_scope".to_string(),
        });
    }
    validate_workflow_alignment(
        &package.workflow_doc,
        &package.resolved_contract,
        &package.stage_documents,
        &package.module_catalog,
    )?;
    validate_stage_document_alignment(&package.stage_documents, &package.resolved_contract)?;
    validate_task_progress_alignment(&package.task_progress, &package.resolved_contract)?;
    if normalization_policy.require_unresolved_ambiguities_report
        && package
            .normalization_report
            .unresolved_ambiguities
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(CoreError::NormalizationEvidenceMissing {
            field: "unresolved_ambiguities".to_string(),
        });
    }

    let expected_readiness_fingerprint = readiness_fingerprint_for(
        &package.project_name,
        package.mode,
        &package.source_provenance,
        &manifest_fingerprint_from_entries(&expected_manifest_entries),
        &package.resolved_contract,
        &package.worktree_protocol,
        &package.normalization_report,
        &package.change_report,
        &package.patch_plan,
        &package.patch_execution_report,
        &package.decision_summary,
        &package.agent_brief,
    );
    if package.readiness.fingerprint != expected_readiness_fingerprint {
        return Err(CoreError::ReadinessFingerprintMismatch);
    }

    Ok(())
}

pub fn emit_package(
    workspace_root: &Path,
    package: &BlueprintPackage,
) -> Result<CliOutput, CoreError> {
    validate_package(package)?;
    let blueprint_policy = default_blueprint_policy()?;
    let contract_root = &package.resolved_contract.paths.contract_root;
    let authority_root = &package.resolved_contract.paths.authority_root;
    let workflow_root = &package.resolved_contract.paths.workflow_root;
    let stages_root = &package.resolved_contract.paths.stages_root;
    let modules_root = &package.resolved_contract.paths.modules_root;

    write_utf8_atomic(
        &workspace_root.join(format!("{authority_root}/00-authority-root.md")),
        &package.authority_doc,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{workflow_root}/00-workflow-overview.md")),
        &package.workflow_doc,
    )?;
    for stage_document in &package.stage_documents {
        write_utf8_atomic(
            &workspace_root.join(&stage_document.path),
            &stage_document.content,
        )?;
    }
    let active_stage_paths = package
        .stage_documents
        .iter()
        .map(|document| document.path.clone())
        .collect::<Vec<_>>();
    prune_directory_files(workspace_root, stages_root, &active_stage_paths)?;
    write_utf8_atomic(
        &workspace_root.join(format!("{modules_root}/01-language-policy.md")),
        &package.module_policy_doc,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{modules_root}/00-module-catalog.json")),
        &to_pretty_json(&package.module_catalog)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/project-contract.toml")),
        &(package.project_contract_toml.clone() + "\n"),
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/resolved-contract.json")),
        &to_pretty_json(&package.resolved_contract)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/worktree-protocol.json")),
        &to_pretty_json(&package.worktree_protocol)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(&blueprint_policy.package.manifest_file),
        &to_pretty_json(&package.manifest)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/semantic-ir.json")),
        &to_pretty_json(&package.semantic_ir)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/normalization-report.json")),
        &to_pretty_json(&package.normalization_report)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/change-report.json")),
        &to_pretty_json(&package.change_report)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/patch-base.json")),
        &to_pretty_json(&package.patch_base)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/patch-plan.json")),
        &to_pretty_json(&package.patch_plan)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/patch-execution-report.json")),
        &to_pretty_json(&package.patch_execution_report)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/decision-summary.json")),
        &to_pretty_json(&package.decision_summary)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/agent-brief.json")),
        &to_pretty_json(&package.agent_brief)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/readiness.json")),
        &to_pretty_json(&package.readiness)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/task-progress.json")),
        &to_pretty_json(&package.task_progress)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(&blueprint_policy.package.author_report_file),
        &package.author_report_markdown,
    )?;

    Ok(CliOutput {
        status: "ok".to_string(),
        mode: package.mode.as_str().to_string(),
        project_name: package.project_name.clone(),
        readiness: package.readiness.state.clone(),
        correlation_id: package.readiness.fingerprint.clone(),
    })
}

pub fn load_workspace_package(workspace_root: &Path) -> Result<BlueprintPackage, CoreError> {
    let blueprint_policy = default_blueprint_policy()?;
    let authority_doc = read_utf8(&workspace_root.join(format!(
        "{}/authority/00-authority-root.md",
        blueprint_policy.package.blueprint_root
    )))?;
    let workflow_doc = read_utf8(&workspace_root.join(format!(
        "{}/workflow/00-workflow-overview.md",
        blueprint_policy.package.blueprint_root
    )))?;
    let module_policy_doc = read_utf8(&workspace_root.join(format!(
        "{}/modules/01-language-policy.md",
        blueprint_policy.package.blueprint_root
    )))?;
    let author_report_markdown =
        read_utf8(&workspace_root.join(&blueprint_policy.package.author_report_file))?;
    let project_contract_toml = read_utf8(&workspace_root.join(format!(
        "{}/project-contract.toml",
        blueprint_policy.package.contract_root
    )))?;

    let mut module_catalog =
        serde_json::from_str::<ModuleCatalog>(&read_utf8(&workspace_root.join(format!(
            "{}/modules/00-module-catalog.json",
            blueprint_policy.package.blueprint_root
        )))?)
        .map_err(ara_runtime::RuntimeError::Json)?;
    let manifest = serde_json::from_str::<BlueprintManifest>(&read_utf8(
        &workspace_root.join(&blueprint_policy.package.manifest_file),
    )?)
    .map_err(ara_runtime::RuntimeError::Json)?;
    let semantic_ir_path = workspace_root.join(format!(
        "{}/semantic-ir.json",
        blueprint_policy.package.contract_root
    ));
    let patch_base_path = workspace_root.join(format!(
        "{}/patch-base.json",
        blueprint_policy.package.contract_root
    ));
    let normalization_report =
        serde_json::from_str::<NormalizationReport>(&read_utf8(&workspace_root.join(format!(
            "{}/normalization-report.json",
            blueprint_policy.package.contract_root
        )))?)
        .map_err(ara_runtime::RuntimeError::Json)?;
    let mut change_report =
        serde_json::from_str::<ChangeReport>(&read_utf8(&workspace_root.join(format!(
            "{}/change-report.json",
            blueprint_policy.package.contract_root
        )))?)
        .map_err(ara_runtime::RuntimeError::Json)?;
    let patch_plan_path = workspace_root.join(format!(
        "{}/patch-plan.json",
        blueprint_policy.package.contract_root
    ));
    let patch_plan_exists = patch_plan_path.exists();
    let mut patch_plan = if patch_plan_exists {
        serde_json::from_str::<PatchPlan>(&read_utf8(&patch_plan_path)?)
            .map_err(ara_runtime::RuntimeError::Json)?
    } else {
        build_patch_plan_from_change_report(&change_report)
    };
    let readiness = serde_json::from_str::<ReadinessReport>(&read_utf8(&workspace_root.join(
        format!("{}/readiness.json", blueprint_policy.package.contract_root),
    ))?)
    .map_err(ara_runtime::RuntimeError::Json)?;
    let decision_summary_path = workspace_root.join(format!(
        "{}/decision-summary.json",
        blueprint_policy.package.contract_root
    ));
    let decision_summary_exists = decision_summary_path.exists();
    let agent_brief_path = workspace_root.join(format!(
        "{}/agent-brief.json",
        blueprint_policy.package.contract_root
    ));
    let agent_brief_exists = agent_brief_path.exists();
    let task_progress =
        serde_json::from_str::<TaskProgressReport>(&read_utf8(&workspace_root.join(format!(
            "{}/task-progress.json",
            blueprint_policy.package.contract_root
        )))?)
        .map_err(ara_runtime::RuntimeError::Json)?;
    let mut resolved_contract =
        serde_json::from_str::<ResolvedContract>(&read_utf8(&workspace_root.join(format!(
            "{}/resolved-contract.json",
            blueprint_policy.package.contract_root
        )))?)
        .map_err(ara_runtime::RuntimeError::Json)?;
    let worktree_protocol_path = workspace_root.join(format!(
        "{}/worktree-protocol.json",
        blueprint_policy.package.contract_root
    ));
    let worktree_protocol_exists = worktree_protocol_path.exists();
    let patch_execution_report_path = workspace_root.join(format!(
        "{}/patch-execution-report.json",
        blueprint_policy.package.contract_root
    ));
    let patch_execution_report_exists = patch_execution_report_path.exists();

    let stage_documents = resolved_contract
        .stages
        .iter()
        .map(|stage| {
            let entry = manifest
                .files
                .iter()
                .find(|entry| entry.doc_role == "stage" && entry.canonical_id == stage.stage_id)
                .ok_or_else(|| CoreError::MissingManifestEntry {
                    path: format!("stage:{}", stage.stage_id),
                })?;
            let content = read_utf8(&workspace_root.join(&entry.path))?;
            Ok(StageDocument {
                path: entry.path.clone(),
                stage_id: stage.stage_id.clone(),
                content,
            })
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    let mut worktree_protocol = if worktree_protocol_exists {
        serde_json::from_str::<WorktreeProtocol>(&read_utf8(&worktree_protocol_path)?)
            .map_err(ara_runtime::RuntimeError::Json)?
    } else {
        build_worktree_protocol(
            &parse_sections(&workflow_doc),
            &resolved_contract.stages,
            &stage_documents,
            &module_catalog,
            &resolved_contract.paths,
        )?
    };
    if worktree_protocol_is_empty(&resolved_contract.worktree) {
        resolved_contract.worktree = worktree_protocol.clone();
    }
    if worktree_protocol_is_empty(&worktree_protocol) {
        worktree_protocol = resolved_contract.worktree.clone();
    }
    if module_catalog_needs_worktree_bindings(&module_catalog, &worktree_protocol) {
        apply_worktree_bindings_to_module_catalog(&mut module_catalog, &worktree_protocol);
    }

    let source_provenance = manifest
        .files
        .first()
        .map(|entry| entry.source_provenance.clone())
        .unwrap_or_default();
    let mode = mode_from_source_type(&normalization_report.source_type)?;
    let workspace_bundle = read_workspace_blueprint_bundle_from_defaults(workspace_root)?;
    let parsed_workspace = parse_workspace_source(&workspace_bundle);
    let current_source_fingerprint = parsed_source_fingerprint(&parsed_workspace);
    let patch_base_exists = patch_base_path.exists();
    let mut patch_base = if patch_base_exists {
        serde_json::from_str::<PatchBase>(&read_utf8(&patch_base_path)?)
            .map_err(ara_runtime::RuntimeError::Json)?
    } else {
        build_patch_base_fallback(mode, &parsed_workspace, &patch_plan)
    };
    if patch_base.artifact_status.trim().is_empty() {
        patch_base.artifact_status = if patch_base_exists {
            "emitted".to_string()
        } else {
            "legacy-derived-current-state".to_string()
        };
    }
    if patch_base.base_fingerprint.trim().is_empty() {
        patch_base.base_fingerprint = match patch_base_to_source(&patch_base) {
            Some(source) => parsed_source_fingerprint(&source),
            None if !patch_plan.base_fingerprint.trim().is_empty() => {
                patch_plan.base_fingerprint.clone()
            }
            None => current_source_fingerprint.clone(),
        };
    }
    let mut should_hydrate_patch_state = patch_plan.operations.iter().any(|operation| {
        matches!(operation.scope.as_str(), "section" | "stage-section")
            && operation.value_lines.is_empty()
    }) || patch_plan
        .operations
        .iter()
        .any(|operation| operation.scope == "stage" && operation.stage_name.is_none())
        || change_report.patch_operations.iter().any(|operation| {
            matches!(operation.scope.as_str(), "section" | "stage-section")
                && operation.value_lines.is_empty()
        })
        || change_report
            .patch_operations
            .iter()
            .any(|operation| operation.scope == "stage" && operation.stage_name.is_none());
    should_hydrate_patch_state =
        should_hydrate_patch_state && (!patch_plan_exists || !patch_execution_report_exists);
    let patch_execution_report = if patch_execution_report_exists {
        serde_json::from_str::<PatchExecutionReport>(&read_utf8(&patch_execution_report_path)?)
            .map_err(ara_runtime::RuntimeError::Json)?
    } else {
        build_patch_execution_report_fallback(mode, &patch_plan, &current_source_fingerprint)
    };
    if patch_plan.base_fingerprint.trim().is_empty() {
        patch_plan.base_fingerprint = if patch_execution_report.base_fingerprint.trim().is_empty() {
            current_source_fingerprint.clone()
        } else {
            patch_execution_report.base_fingerprint.clone()
        };
    }
    if patch_plan.result_fingerprint.trim().is_empty() {
        patch_plan.result_fingerprint = if patch_execution_report
            .expected_result_fingerprint
            .trim()
            .is_empty()
        {
            current_source_fingerprint.clone()
        } else {
            patch_execution_report.expected_result_fingerprint.clone()
        };
    }
    let decision_summary = if decision_summary_exists {
        serde_json::from_str::<DecisionSummary>(&read_utf8(&decision_summary_path)?)
            .map_err(ara_runtime::RuntimeError::Json)?
    } else {
        build_decision_summary(
            mode,
            &readiness,
            &normalization_report,
            &patch_plan,
            &resolved_contract,
            &worktree_protocol,
            &stage_documents,
        )?
    };
    let agent_brief = if agent_brief_exists {
        serde_json::from_str::<AgentBrief>(&read_utf8(&agent_brief_path)?)
            .map_err(ara_runtime::RuntimeError::Json)?
    } else {
        build_agent_brief(
            &resolved_contract.project_name,
            mode,
            &readiness,
            &task_progress,
            &decision_summary,
        )
    };
    let should_hydrate_patch_base_state = patch_plan.operations.iter().any(|operation| {
        matches!(operation.scope.as_str(), "section" | "stage-section")
            && operation.previous_value_lines.is_empty()
    }) || patch_plan
        .operations
        .iter()
        .any(|operation| operation.scope == "stage" && operation.previous_stage_name.is_none())
        || patch_plan.operations.iter().any(|operation| {
            operation.reverse_strategy.is_none() || operation.strategy_metadata.is_empty()
        })
        || change_report.patch_operations.iter().any(|operation| {
            matches!(operation.scope.as_str(), "section" | "stage-section")
                && operation.previous_value_lines.is_empty()
        })
        || change_report
            .patch_operations
            .iter()
            .any(|operation| operation.scope == "stage" && operation.previous_stage_name.is_none())
        || change_report.patch_operations.iter().any(|operation| {
            operation.reverse_strategy.is_none() || operation.strategy_metadata.is_empty()
        });
    if should_hydrate_patch_state {
        hydrate_patch_plan_from_state(
            &mut patch_plan.operations,
            &parsed_workspace.sections,
            &parsed_workspace.stage_sections,
        );
        hydrate_patch_plan_from_state(
            &mut change_report.patch_operations,
            &parsed_workspace.sections,
            &parsed_workspace.stage_sections,
        );
    }
    if should_hydrate_patch_base_state {
        let base_hydration_source =
            patch_base_to_source(&patch_base).unwrap_or_else(|| parsed_workspace.clone());
        hydrate_patch_plan_with_base_state(
            &mut patch_plan.operations,
            &base_hydration_source.sections,
            &base_hydration_source.stage_sections,
        );
        hydrate_patch_plan_with_base_state(
            &mut change_report.patch_operations,
            &base_hydration_source.sections,
            &base_hydration_source.stage_sections,
        );
    }
    if patch_operations_need_worktree_scope_hydration(&patch_plan.operations)
        || patch_operations_need_worktree_scope_hydration(&change_report.patch_operations)
    {
        hydrate_patch_operation_worktree_scope(
            &mut patch_plan.operations,
            &resolved_contract,
            &worktree_protocol,
            &stage_documents,
        )?;
        hydrate_patch_operation_worktree_scope(
            &mut change_report.patch_operations,
            &resolved_contract,
            &worktree_protocol,
            &stage_documents,
        )?;
    }
    let semantic_ir_exists = semantic_ir_path.exists();
    let reconstructed_source =
        reconstruct_result_source(mode, &patch_base, &patch_plan, &parsed_workspace)?;
    let mut semantic_ir = if semantic_ir_path.exists() {
        serde_json::from_str::<SemanticIr>(&read_utf8(&semantic_ir_path)?)
            .map_err(ara_runtime::RuntimeError::Json)?
    } else {
        build_semantic_ir(
            mode,
            &resolved_contract.project_name,
            &source_provenance,
            &normalization_report.source_type,
            "workspace-source-reconstructed-fallback",
            &normalization_report.source_files,
            &normalization_report.preserved_sections,
            &normalization_report.inferred_sections,
            &reconstructed_source,
            &parsed_workspace,
            &resolved_contract.stages,
            &normalization_report.semantic_hints,
            &normalization_report.semantic_risks,
            &normalization_report.semantic_conflicts,
            &normalization_report.unresolved_ambiguities,
            &fallback_semantic_frames_for_source(
                &reconstructed_source.sections,
                &reconstructed_source.stage_sections,
                &normalization_report.preserved_sections,
                &normalization_report.inferred_sections,
            ),
        )
    };
    if semantic_ir.schema_version != SEMANTIC_IR_SCHEMA_VERSION {
        if semantic_ir.normalized_sections.is_empty() {
            semantic_ir.normalized_sections = reconstructed_source.sections.clone();
        }
        if semantic_ir.normalized_stage_sections.is_empty() {
            semantic_ir.normalized_stage_sections = reconstructed_source.stage_sections.clone();
        }
        if semantic_ir.normalized_section_origins.is_empty() {
            semantic_ir.normalized_section_origins = semantic_section_origins_from_source(
                &semantic_ir.normalized_sections,
                &normalization_report.preserved_sections,
                &normalization_report.inferred_sections,
            );
        }
        if semantic_ir.normalized_stage_section_origins.is_empty() {
            semantic_ir.normalized_stage_section_origins =
                semantic_stage_section_origins_from_source(&semantic_ir.normalized_stage_sections);
        }
        if semantic_ir.source_fingerprint.trim().is_empty() {
            semantic_ir.source_fingerprint = parsed_source_fingerprint(&reconstructed_source);
        }
        if semantic_ir.projection_fingerprint.trim().is_empty() {
            semantic_ir.projection_fingerprint = parsed_source_fingerprint(&parsed_workspace);
        }
        if semantic_ir.semantic_frames.is_empty() {
            semantic_ir.semantic_frames = fallback_semantic_frames_for_source(
                &semantic_ir.normalized_sections,
                &semantic_ir.normalized_stage_sections,
                &normalization_report.preserved_sections,
                &normalization_report.inferred_sections,
            );
        }
        if semantic_ir.semantic_clusters.is_empty() {
            semantic_ir.semantic_clusters =
                semantic_clusters_from_frames(&semantic_ir.semantic_frames);
        }
    }
    if semantic_ir.semantic_frames.is_empty() {
        semantic_ir.semantic_frames = fallback_semantic_frames_for_source(
            &semantic_ir.normalized_sections,
            &semantic_ir.normalized_stage_sections,
            &normalization_report.preserved_sections,
            &normalization_report.inferred_sections,
        );
    }
    if semantic_ir.semantic_clusters.is_empty() {
        semantic_ir.semantic_clusters = semantic_clusters_from_frames(&semantic_ir.semantic_frames);
    }

    let mut package = BlueprintPackage {
        mode,
        project_name: resolved_contract.project_name.clone(),
        source_provenance,
        authority_doc,
        workflow_doc,
        stage_documents,
        module_policy_doc,
        author_report_markdown,
        project_contract_toml,
        module_catalog,
        manifest,
        worktree_protocol,
        semantic_ir,
        normalization_report,
        change_report,
        patch_base,
        patch_plan,
        patch_execution_report,
        decision_summary,
        agent_brief,
        readiness,
        task_progress,
        resolved_contract,
    };
    if !patch_base_exists
        || !patch_plan_exists
        || !patch_execution_report_exists
        || !decision_summary_exists
        || !agent_brief_exists
        || !semantic_ir_exists
        || !worktree_protocol_exists
    {
        refresh_readiness_fingerprint(&mut package)?;
    }

    Ok(package)
}

pub fn validate_workspace_package(workspace_root: &Path) -> Result<BlueprintPackage, CoreError> {
    let package = load_workspace_package(workspace_root)?;
    validate_package(&package)?;
    Ok(package)
}

pub fn migrate_workspace_package(workspace_root: &Path) -> Result<MigrationReport, CoreError> {
    let mut package = load_workspace_package(workspace_root)?;
    let mut parsed_contract =
        toml::from_str::<ProjectContractToml>(&package.project_contract_toml)?;
    let migration_report =
        upgrade_package_schema_versions(workspace_root, &mut package, &mut parsed_contract)?;
    package.project_contract_toml = toml::to_string_pretty(&parsed_contract)? + "\n";
    package.semantic_ir = derive_semantic_ir_from_package(&package)?;
    refresh_readiness_fingerprint(&mut package)?;

    validate_package(&package)?;
    emit_package(workspace_root, &package)?;
    validate_workspace_package(workspace_root)?;

    write_utf8_atomic(
        &workspace_root.join(format!(
            "{}/migration-report.json",
            package.resolved_contract.paths.contract_root
        )),
        &to_pretty_json(&migration_report)?,
    )?;

    Ok(migration_report)
}

pub fn apply_workspace_patch_plan(
    workspace_root: &Path,
) -> Result<PatchExecutionReport, CoreError> {
    let mut package = load_workspace_package(workspace_root)?;
    if !matches!(package.mode, AuthorMode::UpdateBlueprint) {
        return Err(CoreError::PatchBaseUnavailable {
            reason: format!(
                "apply-patch-plan requires `update-blueprint` packages, found `{}`",
                package.mode.as_str()
            ),
        });
    }
    if package.patch_base.artifact_status != "emitted" {
        return Err(CoreError::PatchBaseUnavailable {
            reason: format!(
                "workspace patch base is `{}`; standalone apply requires an emitted patch-base.json artifact",
                package.patch_base.artifact_status
            ),
        });
    }

    validate_change_report_alignment(&package.change_report, package.mode)?;
    validate_patch_base_alignment(&package.patch_base, &package.patch_plan, package.mode)?;
    validate_patch_plan_alignment(&package.patch_plan, &package.change_report, package.mode)?;
    let scope_mismatches = collect_patch_operation_scope_mismatches(
        &package.patch_plan.operations,
        &package.resolved_contract,
        &package.worktree_protocol,
        &package.stage_documents,
    )?;
    if !scope_mismatches.is_empty() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.worktree_scope".to_string(),
        });
    }

    let base_source = patch_base_to_source(&package.patch_base).ok_or_else(|| {
        CoreError::PatchBaseUnavailable {
            reason: "patch-base.json does not contain a replayable normalized base source"
                .to_string(),
        }
    })?;
    let patch_execution_report = build_patch_execution_report(
        package.mode,
        &package.patch_plan,
        &base_source,
        &package.resolved_contract,
        &package.worktree_protocol,
        &package.stage_documents,
    )?;
    validate_patch_execution_report_alignment(
        &patch_execution_report,
        &package.patch_plan,
        package.mode,
        &package,
    )?;

    package.patch_execution_report = patch_execution_report.clone();
    refresh_readiness_fingerprint(&mut package)?;

    let contract_root = package.resolved_contract.paths.contract_root.clone();
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/patch-execution-report.json")),
        &to_pretty_json(&package.patch_execution_report)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/decision-summary.json")),
        &to_pretty_json(&package.decision_summary)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/agent-brief.json")),
        &to_pretty_json(&package.agent_brief)?,
    )?;
    write_utf8_atomic(
        &workspace_root.join(format!("{contract_root}/readiness.json")),
        &to_pretty_json(&package.readiness)?,
    )?;

    Ok(patch_execution_report)
}

pub fn default_contract_root() -> Result<String, CoreError> {
    Ok(default_blueprint_policy()?.package.contract_root)
}

pub fn read_workspace_blueprint_bundle_from_defaults(
    workspace_root: &Path,
) -> Result<String, CoreError> {
    let policy = default_blueprint_policy()?;
    read_workspace_blueprint_bundle_with_roots(
        workspace_root,
        &[
            format!("{}/authority", policy.package.blueprint_root),
            format!("{}/workflow", policy.package.blueprint_root),
            format!("{}/stages", policy.package.blueprint_root),
        ],
    )
    .map_err(CoreError::from)
}

fn normalize_blueprint(
    mode: AuthorMode,
    input: &BlueprintAuthorInput,
    policy: &NormalizationPolicyToml,
) -> Result<NormalizedBlueprint, CoreError> {
    let explicit_sections = input
        .source_text
        .as_deref()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| parse_input_source(raw, input.source_path.as_deref()))
        .transpose()?;
    let workspace_sections = input
        .workspace_source_text
        .as_deref()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(parse_workspace_source);
    let explicit_semantic_frames = explicit_sections
        .as_ref()
        .map(|parsed| parsed.semantic_frames.clone())
        .unwrap_or_default();
    let workspace_semantic_frames = workspace_sections
        .as_ref()
        .map(|parsed| parsed.semantic_frames.clone())
        .unwrap_or_default();

    let source_files = collect_source_files(input);
    let dropped_sections = collect_dropped_sections(&[
        input.workspace_source_text.as_deref(),
        input.source_text.as_deref(),
    ]);
    let mut semantic_hints = collect_semantic_hints(&[
        input.workspace_source_text.as_deref(),
        input.source_text.as_deref(),
    ]);
    let mut semantic_risks = collect_semantic_risks(&dropped_sections);
    let mut semantic_conflicts = Vec::new();

    let (
        mut sections,
        stage_sections,
        base_source,
        preserved_sections,
        mut unresolved_ambiguities,
        mut change_operations,
        mut patch_operations,
        source_provenance,
    ) = match mode {
        AuthorMode::UpdateBlueprint => {
            merge_update_sections(workspace_sections, explicit_sections, input, policy)?
        }
        AuthorMode::RecompileContract => {
            let parsed = workspace_sections
                .or(explicit_sections)
                .unwrap_or_else(|| parse_workspace_source(input.source_summary.trim()));
            let base = parsed.clone();
            let mut preserved = parsed.sections.keys().cloned().collect::<Vec<_>>();
            if !parsed.stage_sections.is_empty() {
                preserved.push("stage_details".to_string());
            }
            preserved.sort();
            (
                parsed.sections,
                parsed.stage_sections,
                Some(base),
                preserved,
                Vec::new(),
                vec![change_operation(
                    "package",
                    input.project_name.trim(),
                    "recompiled",
                    "recompiled machine outputs from the existing blueprint workspace",
                )],
                vec![patch_operation(
                    "package",
                    input.project_name.trim(),
                    "recompile-machine-outputs",
                    "applied",
                    "recompiled machine outputs from the existing workspace package",
                )],
                "workspace-recompiled".to_string(),
            )
        }
        _ => {
            let raw_source = input
                .source_text
                .as_deref()
                .unwrap_or(input.source_summary.as_str())
                .trim();
            let has_explicit_source = input.source_text.is_some();
            let raw_non_empty_lines = raw_source
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .count();
            let parsed = explicit_sections.unwrap_or_else(|| parse_workspace_source(raw_source));
            let base = parsed.clone();
            let recognized_before_inference = parsed.sections.len() + parsed.stage_sections.len();
            if has_explicit_source
                && matches!(mode, AuthorMode::ImportBlueprint)
                && recognized_before_inference == 0
                && raw_non_empty_lines < 2
            {
                return Err(CoreError::TooAmbiguousToNormalize {
                    reason: "external blueprint did not expose recognizable sections".to_string(),
                });
            }
            let mut preserved = parsed.sections.keys().cloned().collect::<Vec<_>>();
            if !parsed.stage_sections.is_empty() {
                preserved.push("stage_details".to_string());
            }
            preserved.sort();
            (
                parsed.sections,
                parsed.stage_sections,
                Some(base),
                preserved,
                Vec::new(),
                vec![change_operation(
                    "package",
                    input.project_name.trim(),
                    if has_explicit_source && matches!(mode, AuthorMode::ImportBlueprint) {
                        "normalized-import"
                    } else {
                        "generated"
                    },
                    if has_explicit_source && matches!(mode, AuthorMode::ImportBlueprint) {
                        "normalized an external blueprint into the local authoring schema"
                    } else {
                        "generated a new blueprint package from the provided source intent"
                    },
                )],
                vec![patch_operation(
                    "package",
                    input.project_name.trim(),
                    if has_explicit_source && matches!(mode, AuthorMode::ImportBlueprint) {
                        "normalize-external-blueprint"
                    } else {
                        "generate-package"
                    },
                    "applied",
                    if has_explicit_source && matches!(mode, AuthorMode::ImportBlueprint) {
                        "normalized an external blueprint into the local schema"
                    } else {
                        "generated a new package from the provided source intent"
                    },
                )],
                if has_explicit_source && matches!(mode, AuthorMode::ImportBlueprint) {
                    "normalized-import".to_string()
                } else {
                    "generated".to_string()
                },
            )
        }
    };
    let mut inferred_sections = Vec::new();

    if !sections.contains_key("stage_order") {
        let inferred_from_stage_sections = infer_stage_order_from_stage_sections(&stage_sections);
        if !inferred_from_stage_sections.is_empty() {
            sections.insert("stage_order".to_string(), inferred_from_stage_sections);
            inferred_sections.push("stage_order".to_string());
            change_operations.push(change_operation(
                "section",
                "stage_order",
                "inferred",
                "derived stage order from stage-specific detail blocks",
            ));
            patch_operations.push(patch_operation(
                "section",
                "stage_order",
                "infer-stage-order",
                "applied",
                "derived stage order from stage-specific detail blocks",
            ));
        }
    }

    if !sections.contains_key("stage_order") {
        let inferred_stage_order = infer_stage_order_from_source_files(&[
            input.workspace_source_text.as_deref(),
            input.source_text.as_deref(),
        ]);
        if !inferred_stage_order.is_empty() {
            sections.insert("stage_order".to_string(), inferred_stage_order);
            inferred_sections.push("stage_order".to_string());
            change_operations.push(change_operation(
                "section",
                "stage_order",
                "inferred",
                "recovered stage order from stage source-file metadata",
            ));
            patch_operations.push(patch_operation(
                "section",
                "stage_order",
                "infer-stage-order",
                "applied",
                "recovered stage order from stage source-file metadata",
            ));
        }
    }

    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "purpose",
        vec![format!(
            "Deliver the blueprint package for `{}`.",
            input.project_name.trim()
        )],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "authority_scope",
        vec![
            "Product intent".to_string(),
            "Constraints".to_string(),
            "Non-goals".to_string(),
        ],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "truth_rules",
        vec![
            "Authority docs outrank workflow and stage docs.".to_string(),
            "Machine contracts must preserve canonical identifiers.".to_string(),
        ],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "conflict_resolution",
        vec!["Stop authoring if downstream docs rewrite authority truth.".to_string()],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "non_goals",
        vec![
            "Unplanned scope expansion".to_string(),
            "Stage pollution".to_string(),
        ],
    );
    if !sections.contains_key("stage_order") {
        if matches!(mode, AuthorMode::NewProject) || policy.allow_implicit_stage_creation {
            ensure_section(
                &mut sections,
                &mut inferred_sections,
                "stage_order",
                vec!["stage-01: foundation".to_string()],
            );
        } else {
            return Err(CoreError::TooAmbiguousToNormalize {
                reason: "normalization policy forbids implicit stage creation when no deterministic stage order was supplied".to_string(),
            });
        }
    }
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "cross_stage_split_rule",
        vec!["Do not move into later execution stages during authoring.".to_string()],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "stop_conditions",
        vec![
            "Ambiguous stage graph".to_string(),
            "Conflicting authority truth".to_string(),
            "Missing machine outputs".to_string(),
        ],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "entry_rule",
        vec!["Authority, workflow, and module planning inputs must be available.".to_string()],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "exit_gate",
        vec!["Emit contract, manifest, readiness, and author report artifacts.".to_string()],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "allowed_scope",
        vec![
            "Blueprint docs".to_string(),
            "Module catalog".to_string(),
            "Contract outputs".to_string(),
        ],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "forbidden_scope",
        vec![
            "Target project feature implementation".to_string(),
            "Stage advance beyond authoring".to_string(),
        ],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "deliverables",
        vec![
            "Authority doc".to_string(),
            "Workflow doc".to_string(),
            "Module catalog".to_string(),
            "Contract bundle".to_string(),
        ],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "required_verification",
        vec![
            "Structural validation".to_string(),
            "Module language policy validation".to_string(),
            "Readiness emission".to_string(),
        ],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "review_focus",
        vec![
            "Missing contract fields".to_string(),
            "Ambiguous paths".to_string(),
            "Invalid module languages".to_string(),
        ],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "advance_rule",
        vec!["Stop at `candidate-for-blueprint-gate`.".to_string()],
    );
    ensure_section(
        &mut sections,
        &mut inferred_sections,
        "repair_routing",
        vec!["Return to authoring and normalize again if validation fails.".to_string()],
    );

    if input.source_text.is_none()
        && matches!(
            mode,
            AuthorMode::ImportBlueprint | AuthorMode::UpdateBlueprint
        )
    {
        unresolved_ambiguities.push("import mode did not provide an external blueprint file; normalization relied on source summary".to_string());
    }

    for ambiguity in &unresolved_ambiguities {
        semantic_risks.push(format!("normalization ambiguity: {ambiguity}"));
    }

    for key in &inferred_sections {
        if key == "stage_order"
            && change_operations.iter().any(|operation| {
                operation.target_kind == "section"
                    && operation.target_id == "stage_order"
                    && operation.action == "inferred"
            })
        {
            continue;
        }
        change_operations.push(change_operation(
            "section",
            key,
            "inferred",
            "runtime filled a required section using embedded defaults",
        ));
        patch_operations.push(patch_operation(
            "section",
            key,
            "fill-default",
            "applied",
            "runtime filled a required section using embedded defaults",
        ));
    }
    for stage_id in stage_sections.keys() {
        change_operations.push(change_operation(
            "stage",
            stage_id,
            "stage-detail-preserved",
            "preserved stage-specific detail during normalization",
        ));
        patch_operations.push(patch_operation(
            "stage",
            stage_id,
            "preserve-stage-detail",
            "applied",
            "preserved stage-specific detail during normalization",
        ));
    }
    let mut semantic_frames = workspace_semantic_frames;
    semantic_frames.extend(explicit_semantic_frames);
    semantic_frames = complete_semantic_frames_with_fallback(
        &semantic_frames,
        &sections,
        &stage_sections,
        &preserved_sections,
        &inferred_sections,
    );
    augment_semantic_analysis_from_frames(
        &semantic_frames,
        &mut semantic_hints,
        &mut semantic_risks,
        &mut semantic_conflicts,
        &mut unresolved_ambiguities,
    );
    semantic_hints.sort();
    semantic_hints.dedup();
    semantic_risks.sort();
    semantic_risks.dedup();
    unresolved_ambiguities.sort();
    unresolved_ambiguities.dedup();
    for ambiguity in &unresolved_ambiguities {
        if !change_operations.iter().any(|operation| {
            operation.target_kind == "normalization"
                && operation.target_id == "ambiguity"
                && operation.details == *ambiguity
        }) {
            change_operations.push(change_operation(
                "normalization",
                "ambiguity",
                "retained-conflict",
                ambiguity,
            ));
        }
    }

    Ok(NormalizedBlueprint {
        sections,
        stage_sections,
        base_source,
        semantic_frames,
        source_files,
        preserved_sections,
        inferred_sections,
        dropped_sections,
        semantic_hints,
        semantic_risks,
        semantic_conflicts,
        unresolved_ambiguities,
        change_operations,
        patch_operations,
        source_provenance,
    })
}

fn build_stages(
    stage_order: Option<&Vec<String>>,
    mode: AuthorMode,
    policy: &NormalizationPolicyToml,
) -> Result<Vec<ContractStage>, CoreError> {
    let lines = stage_order
        .cloned()
        .unwrap_or_else(|| vec!["stage-01: foundation".to_string()]);
    let mut stages = Vec::new();
    let mut seen_stage_ids = BTreeSet::new();

    for (index, line) in lines.iter().enumerate() {
        if policy.require_deterministic_stage_ids
            && !matches!(mode, AuthorMode::NewProject)
            && !stage_line_has_explicit_stage_id(line)
        {
            return Err(CoreError::TooAmbiguousToNormalize {
                reason: format!(
                    "stage order line `{}` did not provide a deterministic stage id",
                    line.trim()
                ),
            });
        }
        let stage = parse_stage_line(line, index);
        if !seen_stage_ids.insert(stage.stage_id.clone()) {
            return Err(CoreError::InvalidStageGraph {
                reason: format!("duplicate stage id `{}`", stage.stage_id),
            });
        }
        stages.push(stage);
    }

    if stages.is_empty() {
        return Err(CoreError::InvalidStageGraph {
            reason: "no stages could be derived from stage_order".to_string(),
        });
    }

    Ok(stages)
}

fn parse_stage_line(line: &str, index: usize) -> ContractStage {
    let cleaned = sanitize_line(line);
    for separator in [":", " - ", " -- "] {
        if let Some((left, right)) = cleaned.split_once(separator) {
            let stage_id = left.trim().trim_matches('`').to_string();
            let stage_name = right.trim().trim_matches('`').to_string();
            if !stage_id.is_empty() && !stage_name.is_empty() {
                return ContractStage {
                    stage_id,
                    stage_name: stage_name.clone(),
                    default_next_goal: format!("Complete the {} stage deliverables.", stage_name),
                };
            }
        }
    }

    let mut parts = cleaned.split_whitespace();
    let first = parts.next().unwrap_or("stage-01");
    let rest = parts.collect::<Vec<_>>().join(" ");
    let stage_id = if looks_like_stage_id(first) {
        first.trim_matches('`').to_string()
    } else {
        format!("stage-{index_plus:02}", index_plus = index + 1)
    };
    let stage_name = if rest.is_empty() {
        "foundation".to_string()
    } else {
        rest.trim_matches('`').to_string()
    };

    ContractStage {
        stage_id,
        stage_name: stage_name.clone(),
        default_next_goal: format!("Complete the {} stage deliverables.", stage_name),
    }
}

fn looks_like_stage_id(token: &str) -> bool {
    let lowered = token.trim_matches('`').to_lowercase();
    lowered.starts_with("stage-")
        || (lowered.starts_with('p')
            && lowered.chars().nth(1).is_some_and(|ch| ch.is_ascii_digit()))
}

fn stage_line_has_explicit_stage_id(line: &str) -> bool {
    let cleaned = sanitize_line(line);
    for separator in [":", " - ", " -- "] {
        if let Some((left, _)) = cleaned.split_once(separator) {
            return looks_like_stage_id(left.trim());
        }
    }
    cleaned
        .split_whitespace()
        .next()
        .map(looks_like_stage_id)
        .unwrap_or(false)
}

fn build_stage_documents(
    sections: &BTreeMap<String, Vec<String>>,
    stage_sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
    stages: &[ContractStage],
    stages_root: &str,
) -> Vec<StageDocument> {
    let mut used_paths = BTreeSet::new();

    stages
        .iter()
        .enumerate()
        .map(|(index, stage)| {
            let mut path = stage_doc_path(index, stage, stages_root);
            if !used_paths.insert(path.clone()) {
                path = unique_stage_doc_path(index, stage, stages_root);
                let mut disambiguator = 2usize;
                while !used_paths.insert(path.clone()) {
                    path =
                        unique_stage_doc_path_with_suffix(index, stage, disambiguator, stages_root);
                    disambiguator += 1;
                }
            }

            StageDocument {
                path,
                stage_id: stage.stage_id.clone(),
                content: render_stage_doc(
                    &sections_for_stage(sections, stage_sections.get(&stage.stage_id)),
                    stage,
                ),
            }
        })
        .collect()
}

fn sections_for_stage(
    base_sections: &BTreeMap<String, Vec<String>>,
    stage_sections: Option<&BTreeMap<String, Vec<String>>>,
) -> BTreeMap<String, Vec<String>> {
    let mut merged = base_sections.clone();
    if let Some(stage_specific) = stage_sections {
        for (key, values) in stage_specific {
            merged.insert(key.clone(), values.clone());
        }
    }
    merged
}

fn infer_stage_order_from_source_files(sources: &[Option<&str>]) -> Vec<String> {
    let mut ordered_stages = Vec::new();
    let mut seen_stage_ids = BTreeSet::new();

    for source in sources.iter().flatten() {
        for block in parse_source_file_blocks(source) {
            if !block.path.starts_with("blueprint/stages/") {
                continue;
            }

            if let Some((stage_id, stage_name)) =
                extract_stage_metadata(&block.path, &block.content)
            {
                if seen_stage_ids.insert(stage_id.clone()) {
                    ordered_stages.push(format!("{stage_id}: {stage_name}"));
                }
            }
        }
    }

    ordered_stages
}

fn infer_stage_order_from_stage_sections(
    stage_sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
) -> Vec<String> {
    stage_sections
        .iter()
        .map(|(stage_id, sections)| {
            let stage_name = sections
                .get("stage_name")
                .and_then(|values| values.first())
                .cloned()
                .unwrap_or_else(|| stage_id.clone());
            format!("{stage_id}: {stage_name}")
        })
        .collect()
}

#[derive(Debug)]
struct SourceFileBlock {
    path: String,
    content: String,
}

fn parse_source_file_blocks(raw_source: &str) -> Vec<SourceFileBlock> {
    let mut blocks = Vec::new();
    let mut lines = raw_source.lines().peekable();

    while let Some(line) = lines.next() {
        if line.trim() != "# Source File" {
            continue;
        }

        let mut path = None;
        while let Some(next_line) = lines.next() {
            let trimmed = next_line.trim();
            if trimmed.is_empty() {
                continue;
            }
            path = Some(normalize_repo_relative_path(trimmed));
            break;
        }

        let Some(path) = path else {
            continue;
        };

        let mut content_lines = Vec::new();
        while let Some(peeked) = lines.peek() {
            if peeked.trim() == "# Source File" {
                break;
            }
            content_lines.push(lines.next().unwrap_or_default().to_string());
        }

        blocks.push(SourceFileBlock {
            path,
            content: content_lines.join("\n").trim().to_string(),
        });
    }

    blocks
}

fn parse_input_source(
    raw_source: &str,
    source_path: Option<&str>,
) -> Result<ParsedBlueprintSource, CoreError> {
    match primary_source_format(source_path) {
        Some("json") => parse_json_sections(raw_source),
        Some("toml") => parse_toml_sections(raw_source),
        _ => Ok(parse_workspace_source(raw_source)),
    }
}

fn primary_source_format(source_path: Option<&str>) -> Option<&'static str> {
    source_path
        .and_then(|value| {
            source_candidates(&BlueprintAuthorInput {
                project_name: String::new(),
                source_summary: String::new(),
                source_text: None,
                workspace_source_text: None,
                source_path: Some(value.to_string()),
            })
            .into_iter()
            .find(|candidate| !candidate.starts_with("workspace:"))
        })
        .as_deref()
        .and_then(detect_source_format)
}

fn semantic_frame(
    scope: &str,
    canonical_section: &str,
    source_label: &str,
    source_locator: &str,
    origin_kind: &str,
    confidence: &str,
    values: Vec<String>,
) -> SemanticFrame {
    SemanticFrame {
        scope: scope.to_string(),
        canonical_section: canonical_section.to_string(),
        source_label: source_label.to_string(),
        source_locator: source_locator.to_string(),
        origin_kind: origin_kind.to_string(),
        confidence: confidence.to_string(),
        values,
    }
}

fn semantic_mapping_confidence(
    source_label: &str,
    canonical_section: &str,
    inline: bool,
) -> String {
    let normalized_label = source_label
        .replace('_', " ")
        .replace('-', " ")
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let normalized_section = canonical_section
        .replace('_', " ")
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if inline {
        if normalized_label == normalized_section {
            return "inline-exact".to_string();
        }
        return "inline-heuristic".to_string();
    }
    if normalized_label == normalized_section {
        "exact".to_string()
    } else {
        "alias".to_string()
    }
}

fn stage_alias_confidence(source_label: &str, canonical_section: &str, inline: bool) -> String {
    if inline {
        return "cross-source-heuristic".to_string();
    }
    match semantic_mapping_confidence(source_label, canonical_section, false).as_str() {
        "exact" => "cross-source-exact".to_string(),
        _ => "cross-source-alias".to_string(),
    }
}

fn stage_path_alias_confidence(
    source_label: &str,
    canonical_section: &str,
    inline: bool,
) -> String {
    if inline {
        return "path-alias-heuristic".to_string();
    }
    match semantic_mapping_confidence(source_label, canonical_section, false).as_str() {
        "exact" => "path-alias-exact".to_string(),
        _ => "path-alias-alias".to_string(),
    }
}

fn confidence_is_heuristic(confidence: &str) -> bool {
    confidence.contains("heuristic")
}

fn confidence_is_indirect(confidence: &str) -> bool {
    confidence.starts_with("cross-source") || confidence.starts_with("path-alias")
}

fn structured_key_to_section_key(key: &str) -> Option<&'static str> {
    let normalized = key.replace('_', " ").replace('-', " ").to_lowercase();
    match normalized.as_str() {
        "workflow" | "workflow overview" | "phases" | "milestones" | "steps" => None,
        _ => heading_key(&normalized),
    }
}

fn dedupe_semantic_frames(mut frames: Vec<SemanticFrame>) -> Vec<SemanticFrame> {
    frames.sort_by(|left, right| {
        (
            &left.scope,
            &left.canonical_section,
            &left.source_locator,
            &left.source_label,
            &left.origin_kind,
            &left.confidence,
            &left.values,
        )
            .cmp(&(
                &right.scope,
                &right.canonical_section,
                &right.source_locator,
                &right.source_label,
                &right.origin_kind,
                &right.confidence,
                &right.values,
            ))
    });
    frames.dedup();
    frames
}

fn semantic_clusters_from_frames(frames: &[SemanticFrame]) -> Vec<SemanticCluster> {
    let mut grouped = BTreeMap::<(String, String), Vec<&SemanticFrame>>::new();
    for frame in frames.iter().filter(|frame| !frame.values.is_empty()) {
        grouped
            .entry((frame.scope.clone(), frame.canonical_section.clone()))
            .or_default()
            .push(frame);
    }

    grouped
        .into_iter()
        .map(|((scope, canonical_section), cluster_frames)| {
            let mut source_labels = cluster_frames
                .iter()
                .map(|frame| frame.source_label.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let mut source_locators = cluster_frames
                .iter()
                .map(|frame| frame.source_locator.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let mut origin_kinds = cluster_frames
                .iter()
                .map(|frame| frame.origin_kind.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let mut confidence_levels = cluster_frames
                .iter()
                .map(|frame| frame.confidence.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let mut merged_values = Vec::new();
            for frame in &cluster_frames {
                merged_values = merge_unique_lines(&merged_values, &frame.values);
            }
            let has_heuristic = confidence_levels
                .iter()
                .any(|value| confidence_is_heuristic(value));
            let merge_pattern = if source_locators.len() <= 1 {
                if has_heuristic {
                    "single-source-heuristic".to_string()
                } else {
                    "single-source".to_string()
                }
            } else if has_heuristic {
                "multi-source-heuristic".to_string()
            } else {
                "multi-source".to_string()
            };
            source_labels.sort();
            source_locators.sort();
            origin_kinds.sort();
            confidence_levels.sort();

            SemanticCluster {
                scope,
                canonical_section,
                source_labels,
                source_locators,
                origin_kinds,
                confidence_levels,
                merged_values,
                merge_pattern,
            }
        })
        .collect()
}

fn stage_conflict_sensitive_section(section: &str) -> bool {
    matches!(
        section,
        "stage_name"
            | "intent"
            | "advance_rule"
            | "repair_routing"
            | "deliverables"
            | "required_verification"
    )
}

fn values_overlap(left: &[String], right: &[String]) -> bool {
    let left = left.iter().cloned().collect::<BTreeSet<_>>();
    let right = right.iter().cloned().collect::<BTreeSet<_>>();
    !left.is_disjoint(&right)
}

fn semantic_source_group_key(source_locator: &str) -> String {
    source_locator
        .split_once('#')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_else(|| source_locator.to_string())
}

fn detect_stage_semantic_conflicts(
    frames: &[SemanticFrame],
    semantic_hints: &mut Vec<String>,
    semantic_risks: &mut Vec<String>,
    semantic_conflicts: &mut Vec<SemanticConflict>,
    unresolved_ambiguities: &mut Vec<String>,
) {
    let mut grouped = BTreeMap::<(String, String), Vec<&SemanticFrame>>::new();
    for frame in frames.iter().filter(|frame| {
        frame.scope.starts_with("stage:")
            && !frame.values.is_empty()
            && stage_conflict_sensitive_section(&frame.canonical_section)
    }) {
        grouped
            .entry((frame.scope.clone(), frame.canonical_section.clone()))
            .or_default()
            .push(frame);
    }

    for ((scope, canonical_section), cluster_frames) in grouped {
        let mut source_groups = BTreeMap::<String, Vec<String>>::new();
        let mut source_group_labels = BTreeMap::<String, BTreeSet<String>>::new();
        let mut source_group_confidence = BTreeMap::<String, BTreeSet<String>>::new();
        let mut has_indirect = false;
        let mut has_heuristic = false;

        for frame in cluster_frames {
            let source_group = semantic_source_group_key(&frame.source_locator);
            let entry = source_groups
                .entry(source_group.clone())
                .or_default()
                .clone();
            source_groups.insert(
                source_group,
                merge_unique_lines(&entry, &frame.values),
            );
            source_group_labels
                .entry(semantic_source_group_key(&frame.source_locator))
                .or_default()
                .insert(frame.source_label.clone());
            source_group_confidence
                .entry(semantic_source_group_key(&frame.source_locator))
                .or_default()
                .insert(frame.confidence.clone());
            has_indirect |= confidence_is_indirect(&frame.confidence);
            has_heuristic |= confidence_is_heuristic(&frame.confidence);
        }

        let source_values = source_groups.values().cloned().collect::<Vec<_>>();
        if source_values.len() <= 1 {
            continue;
        }

        let distinct_sets = source_values
            .iter()
            .map(|values| values.join("\u{1f}"))
            .collect::<BTreeSet<_>>();
        if distinct_sets.len() <= 1 {
            continue;
        }

        let all_disjoint = source_values.iter().enumerate().all(|(index, left)| {
            source_values
                .iter()
                .skip(index + 1)
                .all(|right| !values_overlap(left, right))
        });
        let (severity, blocking, review_required, recommended_action) =
            semantic_conflict_decision(
                &canonical_section,
                all_disjoint,
                has_indirect,
                has_heuristic,
            );

        if matches!(
            canonical_section.as_str(),
            "stage_name" | "intent" | "advance_rule" | "repair_routing"
        ) || (all_disjoint && (has_indirect || has_heuristic))
        {
            let source_group_conflict_entries = source_group_labels
                .iter()
                .map(|(source_group, labels)| SemanticConflictSourceGroup {
                    source_group: source_group.clone(),
                    source_labels: labels.iter().cloned().collect::<Vec<_>>(),
                    source_locators: vec![source_group.clone()],
                    confidence_levels: source_group_confidence
                        .get(source_group)
                        .map(|values| values.iter().cloned().collect::<Vec<_>>())
                        .unwrap_or_default(),
                    merged_values: source_groups
                        .get(source_group)
                        .cloned()
                        .unwrap_or_default(),
                })
                .collect::<Vec<_>>();
            let source_group_details = source_group_labels
                .iter()
                .map(|(source_group, labels)| {
                    let values = source_groups
                        .get(source_group)
                        .cloned()
                        .unwrap_or_default()
                        .join(" | ");
                    let labels = labels.iter().cloned().collect::<Vec<_>>().join(", ");
                    let confidence = source_group_confidence
                        .get(source_group)
                        .map(|values| values.iter().cloned().collect::<Vec<_>>().join(", "))
                        .unwrap_or_else(|| "<none>".to_string());
                    format!(
                        "{} => [{}] via labels [{}] confidence [{}]",
                        source_group,
                        values,
                        labels,
                        confidence
                    )
                })
                .collect::<Vec<_>>();
            let source_group_list = source_group_labels.keys().cloned().collect::<Vec<_>>();
            let confidence_profile = source_group_confidence
                .values()
                .flat_map(|values| values.iter().cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            semantic_conflicts.push(SemanticConflict {
                scope: scope.clone(),
                canonical_section: canonical_section.clone(),
                conflict_kind: "indirect-source-divergence".to_string(),
                source_groups: source_group_conflict_entries,
                confidence_profile: confidence_profile.clone(),
                severity: severity.to_string(),
                blocking,
                review_required,
                recommended_action: recommended_action.to_string(),
            });
            semantic_hints.push(format!(
                "detected stage-scoped semantic divergence in `{}` for `{}` across source groups [{}] with confidence [{}]",
                scope,
                canonical_section,
                source_group_list.join(", "),
                confidence_profile.join(", ")
            ));
            semantic_risks.push(format!(
                "conflicting stage semantic evidence remained in `{}` for `{}` across multiple sources: {}",
                scope,
                canonical_section,
                source_group_details.join("; ")
            ));
            unresolved_ambiguities.push(format!(
                "stage semantic conflict remained in `{}` for `{}` because source groups [{}] did not converge",
                scope,
                canonical_section,
                source_group_list.join(", ")
            ));
        }
    }
}

fn semantic_conflict_decision(
    canonical_section: &str,
    all_disjoint: bool,
    has_indirect: bool,
    has_heuristic: bool,
) -> (&'static str, bool, bool, &'static str) {
    if matches!(
        canonical_section,
        "stage_name" | "intent" | "advance_rule" | "repair_routing"
    ) {
        return (
            "high",
            true,
            true,
            "add explicit stage-scoped workflow or authority truth before this stage can be trusted",
        );
    }

    if all_disjoint && has_indirect && has_heuristic {
        return (
            "high",
            true,
            true,
            "add explicit stage metadata or canonical stage-scoped sections to disambiguate conflicting source groups",
        );
    }

    if all_disjoint && (has_indirect || has_heuristic) {
        return (
            "medium",
            false,
            true,
            "add explicit stage metadata or canonical stage-scoped sections to disambiguate conflicting source groups",
        );
    }

    (
        "low",
        false,
        true,
        "review the merged stage-scoped semantic evidence before relying on this section downstream",
    )
}

fn is_valid_semantic_conflict_severity(value: &str) -> bool {
    matches!(value, "low" | "medium" | "high")
}

fn augment_semantic_analysis_from_frames(
    frames: &[SemanticFrame],
    semantic_hints: &mut Vec<String>,
    semantic_risks: &mut Vec<String>,
    semantic_conflicts: &mut Vec<SemanticConflict>,
    unresolved_ambiguities: &mut Vec<String>,
) {
    for cluster in semantic_clusters_from_frames(frames) {
        if cluster.merge_pattern == "multi-source" {
            semantic_hints.push(format!(
                "merged multiple semantic sources into `{}` for `{}` from locators [{}]",
                cluster.scope,
                cluster.canonical_section,
                cluster.source_locators.join(", ")
            ));
        }
        if cluster.merge_pattern == "single-source-heuristic" {
            semantic_hints.push(format!(
                "heuristically mapped a single semantic source into `{}` for `{}` from labels [{}]",
                cluster.scope,
                cluster.canonical_section,
                cluster.source_labels.join(", ")
            ));
            semantic_risks.push(format!(
                "semantic mapping for `{}` / `{}` depends on a heuristic source label from [{}]",
                cluster.scope,
                cluster.canonical_section,
                cluster.source_locators.join(", ")
            ));
            unresolved_ambiguities.push(format!(
                "heuristic semantic mapping remained in `{}` for `{}` from labels [{}]",
                cluster.scope,
                cluster.canonical_section,
                cluster.source_labels.join(", ")
            ));
        }
        if cluster.merge_pattern == "multi-source-heuristic" {
            semantic_hints.push(format!(
                "merged heuristic semantic sources into `{}` for `{}` from locators [{}]",
                cluster.scope,
                cluster.canonical_section,
                cluster.source_locators.join(", ")
            ));
            semantic_risks.push(format!(
                "semantic mapping for `{}` / `{}` depends on heuristic source labels from [{}]",
                cluster.scope,
                cluster.canonical_section,
                cluster.source_labels.join(", ")
            ));
            unresolved_ambiguities.push(format!(
                "heuristic semantic merge remained in `{}` for `{}` from locators [{}]",
                cluster.scope,
                cluster.canonical_section,
                cluster.source_locators.join(", ")
            ));
        }
        if cluster
            .confidence_levels
            .iter()
            .any(|value| confidence_is_indirect(value))
        {
            semantic_hints.push(format!(
                "resolved `{}` / `{}` through cross-source or path-alias stage evidence from [{}]",
                cluster.scope,
                cluster.canonical_section,
                cluster.source_locators.join(", ")
            ));
        }
    }
    detect_stage_semantic_conflicts(
        frames,
        semantic_hints,
        semantic_risks,
        semantic_conflicts,
        unresolved_ambiguities,
    );
}

fn source_from_semantic_frames(frames: Vec<SemanticFrame>) -> ParsedBlueprintSource {
    let frames = dedupe_semantic_frames(frames);
    let mut sections = BTreeMap::<String, Vec<String>>::new();
    let mut stage_sections = BTreeMap::<String, BTreeMap<String, Vec<String>>>::new();

    for frame in &frames {
        if frame.values.is_empty() {
            continue;
        }
        if let Some(stage_id) = frame.scope.strip_prefix("stage:") {
            let stage_entry = stage_sections
                .entry(stage_id.to_string())
                .or_insert_with(BTreeMap::new);
            let existing = stage_entry
                .get(&frame.canonical_section)
                .cloned()
                .unwrap_or_default();
            stage_entry.insert(
                frame.canonical_section.clone(),
                merge_unique_lines(&existing, &frame.values),
            );
        } else {
            let existing = sections
                .get(&frame.canonical_section)
                .cloned()
                .unwrap_or_default();
            sections.insert(
                frame.canonical_section.clone(),
                merge_unique_lines(&existing, &frame.values),
            );
        }
    }

    ParsedBlueprintSource {
        sections,
        stage_sections,
        semantic_frames: frames,
    }
}

fn stage_collection_key(key: &str) -> bool {
    matches!(
        key.to_lowercase().as_str(),
        "stages" | "stage_details" | "phases" | "milestones" | "steps"
    )
}

fn json_semantic_lines(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(value) => vec![sanitize_line(value)],
        serde_json::Value::Bool(value) => vec![value.to_string()],
        serde_json::Value::Number(value) => vec![value.to_string()],
        serde_json::Value::Array(values) => values.iter().flat_map(json_semantic_lines).collect(),
        serde_json::Value::Object(object) => object
            .iter()
            .flat_map(|(key, value)| {
                json_semantic_lines(value)
                    .into_iter()
                    .map(|line| format!("{key}: {line}"))
                    .collect::<Vec<_>>()
            })
            .collect(),
        serde_json::Value::Null => Vec::new(),
    }
}

fn toml_semantic_lines(value: &toml::Value) -> Vec<String> {
    match value {
        toml::Value::String(value) => vec![sanitize_line(value)],
        toml::Value::Integer(value) => vec![value.to_string()],
        toml::Value::Float(value) => vec![value.to_string()],
        toml::Value::Boolean(value) => vec![value.to_string()],
        toml::Value::Datetime(value) => vec![value.to_string()],
        toml::Value::Array(values) => values.iter().flat_map(toml_semantic_lines).collect(),
        toml::Value::Table(table) => table
            .iter()
            .flat_map(|(key, value)| {
                toml_semantic_lines(value)
                    .into_iter()
                    .map(|line| format!("{key}: {line}"))
                    .collect::<Vec<_>>()
            })
            .collect(),
    }
}

fn push_semantic_frame(
    frames: &mut Vec<SemanticFrame>,
    scope: &str,
    canonical_section: &str,
    source_label: &str,
    source_locator: &str,
    origin_kind: &str,
    confidence: &str,
    values: Vec<String>,
) {
    if values.is_empty() {
        return;
    }
    frames.push(semantic_frame(
        scope,
        canonical_section,
        source_label,
        source_locator,
        origin_kind,
        confidence,
        values,
    ));
}

fn should_emit_json_container_frame(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => !object
            .keys()
            .any(|key| structured_key_to_section_key(key).is_some() || stage_collection_key(key)),
        serde_json::Value::Array(values) => values.iter().all(|value| {
            !matches!(
                value,
                serde_json::Value::Object(_) | serde_json::Value::Array(_)
            )
        }),
        _ => true,
    }
}

fn should_emit_toml_container_frame(value: &toml::Value) -> bool {
    match value {
        toml::Value::Table(table) => !table
            .keys()
            .any(|key| structured_key_to_section_key(key).is_some() || stage_collection_key(key)),
        toml::Value::Array(values) => values
            .iter()
            .all(|value| !matches!(value, toml::Value::Table(_) | toml::Value::Array(_))),
        _ => true,
    }
}

fn collect_json_semantic_frames(
    value: &serde_json::Value,
    scope: &str,
    path: &[String],
    frames: &mut Vec<SemanticFrame>,
) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object {
                let mut child_path = path.to_vec();
                child_path.push(key.clone());
                let locator = child_path.join(".");
                if stage_collection_key(key) {
                    collect_json_stage_semantic_frames(child, &child_path, frames);
                    continue;
                }
                if let Some(section_key) = structured_key_to_section_key(key) {
                    if section_key == "stage_order" {
                        let value_lines = json_stage_order_from_structured_value(Some(child))
                            .unwrap_or_else(|| json_semantic_lines(child));
                        push_semantic_frame(
                            frames,
                            scope,
                            section_key,
                            key,
                            &locator,
                            if path.is_empty() {
                                "structured-key"
                            } else {
                                "nested-structured-key"
                            },
                            &semantic_mapping_confidence(key, section_key, false),
                            value_lines,
                        );
                    } else if should_emit_json_container_frame(child) {
                        push_semantic_frame(
                            frames,
                            scope,
                            section_key,
                            key,
                            &locator,
                            if path.is_empty() {
                                "structured-key"
                            } else {
                                "nested-structured-key"
                            },
                            &semantic_mapping_confidence(key, section_key, false),
                            json_semantic_lines(child),
                        );
                    }
                }
                collect_json_semantic_frames(child, scope, &child_path, frames);
            }
        }
        serde_json::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(index.to_string());
                collect_json_semantic_frames(child, scope, &child_path, frames);
            }
        }
        _ => {}
    }
}

fn collect_json_stage_semantic_frames(
    value: &serde_json::Value,
    path: &[String],
    frames: &mut Vec<SemanticFrame>,
) {
    let serde_json::Value::Array(stages) = value else {
        return;
    };
    for (index, stage) in stages.iter().enumerate() {
        let Some(stage_object) = stage.as_object() else {
            continue;
        };
        let Some(stage_id) = stage_object
            .get("stage_id")
            .and_then(|value| value.as_str())
            .or_else(|| stage_object.get("id").and_then(|value| value.as_str()))
        else {
            continue;
        };
        let stage_scope = format!("stage:{stage_id}");
        let stage_name = stage_object
            .get("stage_name")
            .and_then(|value| value.as_str())
            .or_else(|| stage_object.get("name").and_then(|value| value.as_str()))
            .unwrap_or(stage_id);
        let mut stage_path = path.to_vec();
        stage_path.push(index.to_string());
        push_semantic_frame(
            frames,
            &stage_scope,
            "stage_name",
            "stage_name",
            &format!("{}.stage_name", stage_path.join(".")),
            "stage-structured-key",
            "exact",
            vec![stage_name.to_string()],
        );
        for (key, child) in stage_object {
            if matches!(key.as_str(), "stage_id" | "id" | "stage_name" | "name") {
                continue;
            }
            let mut child_path = stage_path.clone();
            child_path.push(key.clone());
            let locator = child_path.join(".");
            if let Some(section_key) = structured_key_to_section_key(key) {
                if should_emit_json_container_frame(child) {
                    push_semantic_frame(
                        frames,
                        &stage_scope,
                        section_key,
                        key,
                        &locator,
                        "stage-structured-key",
                        &semantic_mapping_confidence(key, section_key, false),
                        json_semantic_lines(child),
                    );
                }
            }
            collect_json_semantic_frames(child, &stage_scope, &child_path, frames);
        }
    }
}

fn collect_toml_semantic_frames(
    value: &toml::Value,
    scope: &str,
    path: &[String],
    frames: &mut Vec<SemanticFrame>,
) {
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table {
                let mut child_path = path.to_vec();
                child_path.push(key.clone());
                let locator = child_path.join(".");
                if stage_collection_key(key) {
                    collect_toml_stage_semantic_frames(child, &child_path, frames);
                    continue;
                }
                if let Some(section_key) = structured_key_to_section_key(key) {
                    if section_key == "stage_order" {
                        let value_lines = toml_stage_order_from_structured_value(Some(child))
                            .unwrap_or_else(|| toml_semantic_lines(child));
                        push_semantic_frame(
                            frames,
                            scope,
                            section_key,
                            key,
                            &locator,
                            if path.is_empty() {
                                "structured-key"
                            } else {
                                "nested-structured-key"
                            },
                            &semantic_mapping_confidence(key, section_key, false),
                            value_lines,
                        );
                    } else if should_emit_toml_container_frame(child) {
                        push_semantic_frame(
                            frames,
                            scope,
                            section_key,
                            key,
                            &locator,
                            if path.is_empty() {
                                "structured-key"
                            } else {
                                "nested-structured-key"
                            },
                            &semantic_mapping_confidence(key, section_key, false),
                            toml_semantic_lines(child),
                        );
                    }
                }
                collect_toml_semantic_frames(child, scope, &child_path, frames);
            }
        }
        toml::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(index.to_string());
                collect_toml_semantic_frames(child, scope, &child_path, frames);
            }
        }
        _ => {}
    }
}

fn collect_toml_stage_semantic_frames(
    value: &toml::Value,
    path: &[String],
    frames: &mut Vec<SemanticFrame>,
) {
    let toml::Value::Array(stages) = value else {
        return;
    };
    for (index, stage) in stages.iter().enumerate() {
        let Some(stage_table) = stage.as_table() else {
            continue;
        };
        let Some(stage_id) = stage_table
            .get("stage_id")
            .and_then(|value| value.as_str())
            .or_else(|| stage_table.get("id").and_then(|value| value.as_str()))
        else {
            continue;
        };
        let stage_scope = format!("stage:{stage_id}");
        let stage_name = stage_table
            .get("stage_name")
            .and_then(|value| value.as_str())
            .or_else(|| stage_table.get("name").and_then(|value| value.as_str()))
            .unwrap_or(stage_id);
        let mut stage_path = path.to_vec();
        stage_path.push(index.to_string());
        push_semantic_frame(
            frames,
            &stage_scope,
            "stage_name",
            "stage_name",
            &format!("{}.stage_name", stage_path.join(".")),
            "stage-structured-key",
            "exact",
            vec![stage_name.to_string()],
        );
        for (key, child) in stage_table {
            if matches!(key.as_str(), "stage_id" | "id" | "stage_name" | "name") {
                continue;
            }
            let mut child_path = stage_path.clone();
            child_path.push(key.clone());
            let locator = child_path.join(".");
            if let Some(section_key) = structured_key_to_section_key(key) {
                if should_emit_toml_container_frame(child) {
                    push_semantic_frame(
                        frames,
                        &stage_scope,
                        section_key,
                        key,
                        &locator,
                        "stage-structured-key",
                        &semantic_mapping_confidence(key, section_key, false),
                        toml_semantic_lines(child),
                    );
                }
            }
            collect_toml_semantic_frames(child, &stage_scope, &child_path, frames);
        }
    }
}

fn trim_list_prefix(line: &str) -> &str {
    let trimmed = line.trim();
    for prefix in ["- ", "* ", "+ ", "• "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.trim();
        }
    }
    let mut chars = trimmed.chars().peekable();
    let mut consumed = 0usize;
    while let Some(ch) = chars.peek() {
        if ch.is_ascii_digit() {
            consumed += ch.len_utf8();
            chars.next();
        } else {
            break;
        }
    }
    if consumed > 0 {
        let rest = &trimmed[consumed..];
        if let Some(rest) = rest.strip_prefix('.') {
            return rest.trim();
        }
    }
    trimmed
}

fn parse_inline_semantic_label(line: &str) -> Option<(&'static str, String, Option<String>)> {
    let trimmed = trim_list_prefix(line);
    let (label, value) = trimmed.split_once(':')?;
    let label = label.trim().trim_matches('`');
    let value = value.trim().trim_matches('`');
    let section_key = heading_key(label)?;
    let value = if value.is_empty() {
        None
    } else {
        Some(sanitize_line(value))
    };
    Some((section_key, label.to_string(), value))
}

fn parse_markdown_heading_line(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if level == 0 {
        return None;
    }
    let heading = trimmed[level..].trim();
    if heading.is_empty() {
        return None;
    }
    Some((level, heading.to_string()))
}

fn stage_alias_key(value: &str) -> String {
    let cleaned = sanitize_line(value).trim_matches('`').trim().to_string();
    if cleaned.is_empty() {
        return String::new();
    }
    if stage_line_has_explicit_stage_id(&cleaned) {
        return slugify_for_path(&parse_stage_line(&cleaned, 0).stage_name);
    }
    slugify_for_path(&cleaned)
}

fn stage_alias_candidates(value: &str) -> Vec<String> {
    const TRAILING_ALIAS_QUALIFIERS: &[&str] = &[
        "stage",
        "notes",
        "note",
        "details",
        "detail",
        "validation",
        "verify",
        "verification",
        "review",
        "reviews",
        "check",
        "checks",
        "plan",
        "plans",
        "summary",
        "summaries",
        "doc",
        "docs",
        "routing",
        "route",
        "advance",
        "gate",
        "gates",
        "deliverables",
        "artifacts",
        "artifact",
    ];

    let alias_key = stage_alias_key(value);
    if alias_key.is_empty() {
        return Vec::new();
    }

    let mut candidates = vec![alias_key.clone()];
    let mut parts = alias_key
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    while parts.len() > 1 {
        let Some(last) = parts.last() else {
            break;
        };
        if !TRAILING_ALIAS_QUALIFIERS.contains(last) {
            break;
        }
        parts.pop();
        let candidate = parts.join("-");
        if !candidate.is_empty() && !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }

    candidates
}

fn register_stage_aliases(registry: &mut BTreeMap<String, ContractStage>, stage: &ContractStage) {
    for alias in [
        stage.stage_id.clone(),
        stage.stage_name.clone(),
        format!("{} {}", stage.stage_id, stage.stage_name),
        format!("{}: {}", stage.stage_id, stage.stage_name),
        format!("{} stage", stage.stage_name),
    ] {
        let key = stage_alias_key(&alias);
        if !key.is_empty() {
            registry.entry(key).or_insert_with(|| stage.clone());
        }
    }
}

fn stage_alias_registry_from_frames(frames: &[SemanticFrame]) -> BTreeMap<String, ContractStage> {
    let mut registry = BTreeMap::new();
    let mut stage_names = BTreeMap::<String, String>::new();

    for frame in frames {
        let Some(stage_id) = frame.scope.strip_prefix("stage:") else {
            continue;
        };
        if frame.canonical_section != "stage_name" {
            continue;
        }
        let Some(stage_name) = frame.values.first() else {
            continue;
        };
        stage_names
            .entry(stage_id.to_string())
            .or_insert_with(|| stage_name.clone());
    }

    for (stage_id, stage_name) in stage_names {
        register_stage_aliases(
            &mut registry,
            &ContractStage {
                stage_id,
                stage_name: stage_name.clone(),
                default_next_goal: format!("Complete the {} stage deliverables.", stage_name),
            },
        );
    }

    registry
}

fn stage_from_alias_text(
    alias_text: &str,
    alias_registry: &BTreeMap<String, ContractStage>,
) -> Option<ContractStage> {
    if stage_line_has_explicit_stage_id(alias_text) {
        return None;
    }

    for candidate in stage_alias_candidates(alias_text) {
        if let Some(stage) = alias_registry.get(&candidate) {
            return Some(stage.clone());
        }
    }

    let alias_key = stage_alias_key(alias_text);
    if alias_key.is_empty() {
        return None;
    }

    alias_registry
        .iter()
        .filter(|(candidate, _)| alias_key.starts_with(&format!("{candidate}-")))
        .max_by_key(|(candidate, _)| candidate.len())
        .map(|(_, stage)| stage.clone())
}

fn stage_from_alias_heading(
    heading_text: &str,
    alias_registry: &BTreeMap<String, ContractStage>,
) -> Option<ContractStage> {
    stage_from_alias_text(heading_text, alias_registry)
}

fn stage_from_path_alias(
    path: &str,
    alias_registry: &BTreeMap<String, ContractStage>,
) -> Option<ContractStage> {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    stage_from_alias_text(stem, alias_registry)
}

fn stage_from_explicit_stage_id(
    stage_id: &str,
    alias_registry: &BTreeMap<String, ContractStage>,
) -> Option<ContractStage> {
    alias_registry
        .values()
        .find(|stage| stage.stage_id.eq_ignore_ascii_case(stage_id.trim()))
        .cloned()
}

fn parse_metadata_label_value(line: &str) -> Option<(String, String)> {
    let trimmed = trim_list_prefix(line).trim();
    let (label, value) = trimmed
        .split_once(':')
        .or_else(|| trimmed.split_once('='))?;
    let label = label.trim().trim_matches('`').trim_matches('"').trim_matches('\'');
    let value = value.trim().trim_matches('`').trim_matches('"').trim_matches('\'');
    if label.is_empty() || value.is_empty() {
        return None;
    }
    Some((label.to_string(), value.to_string()))
}

fn stage_from_document_metadata(
    content: &str,
    alias_registry: &BTreeMap<String, ContractStage>,
) -> Option<(ContractStage, String)> {
    if alias_registry.is_empty() {
        return None;
    }

    let lines = content.lines().collect::<Vec<_>>();
    let mut candidates = Vec::<(String, String)>::new();
    let mut index = 0usize;

    if lines
        .first()
        .map(|line| line.trim() == "---")
        .unwrap_or(false)
    {
        index = 1;
        while index < lines.len() {
            let trimmed = lines[index].trim();
            if trimmed == "---" {
                index += 1;
                break;
            }
            if let Some((label, value)) = parse_metadata_label_value(trimmed) {
                candidates.push((label, value));
            }
            index += 1;
        }
    }

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            break;
        }
        if let Some((label, value)) = parse_metadata_label_value(trimmed) {
            candidates.push((label, value));
        }
        index += 1;
    }

    for (label, value) in candidates {
        let normalized_label = label
            .replace('_', " ")
            .replace('-', " ")
            .trim()
            .to_ascii_lowercase();
        if matches!(
            normalized_label.as_str(),
            "stage id"
                | "stageid"
                | "phase id"
                | "phaseid"
                | "milestone id"
                | "milestoneid"
                | "step id"
                | "stepid"
        ) {
            if let Some(stage) = stage_from_explicit_stage_id(&value, alias_registry) {
                return Some((stage, "metadata-exact".to_string()));
            }
        }
        if matches!(
            normalized_label.as_str(),
            "stage"
                | "stage name"
                | "phase"
                | "phase name"
                | "milestone"
                | "milestone name"
                | "step"
                | "step name"
        ) {
            if let Some(stage) = stage_from_alias_text(&value, alias_registry) {
                return Some((stage, "metadata-alias".to_string()));
            }
        }
    }

    None
}

fn stage_metadata_confidence(base_confidence: &str, inline: bool) -> String {
    if inline {
        "metadata-heuristic".to_string()
    } else if base_confidence.contains("exact") {
        "metadata-exact".to_string()
    } else {
        "metadata-alias".to_string()
    }
}

fn collect_stage_metadata_semantic_frames_from_markdown_document(
    content: &str,
    stage: &ContractStage,
    metadata_confidence: &str,
    source_locator_prefix: &str,
    frames: &mut Vec<SemanticFrame>,
) {
    let mut current_stage_section: Option<(String, String, Vec<String>, String, String)> = None;

    push_semantic_frame(
        frames,
        &format!("stage:{}", stage.stage_id),
        "stage_name",
        "stage_name",
        &format!(
            "{source_locator_prefix}#stage-metadata-{}",
            slugify_for_path(&stage.stage_id)
        ),
        "stage-metadata",
        metadata_confidence,
        vec![stage.stage_name.clone()],
    );

    let flush_stage_section =
        |current_stage_section: &mut Option<(String, String, Vec<String>, String, String)>,
         frames: &mut Vec<SemanticFrame>| {
            let Some((section_key, label, values, origin_kind, confidence)) =
                current_stage_section.take()
            else {
                return;
            };
            if values.is_empty() {
                return;
            }
            push_semantic_frame(
                frames,
                &format!("stage:{}", stage.stage_id),
                &section_key,
                &label,
                &format!(
                    "{source_locator_prefix}#stage-{}-{}",
                    slugify_for_path(&stage.stage_id),
                    slugify_for_path(&label)
                ),
                &origin_kind,
                &confidence,
                values,
            );
        };

    let lines = content.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        if let Some((_, heading_text)) = parse_markdown_heading_line(trimmed) {
            if let Some(section_key) = heading_key(&heading_text) {
                if section_key != "stage_order" {
                    flush_stage_section(&mut current_stage_section, frames);
                    current_stage_section = Some((
                        section_key.to_string(),
                        heading_text.clone(),
                        Vec::new(),
                        "stage-metadata-heading-section".to_string(),
                        stage_metadata_confidence(metadata_confidence, false),
                    ));
                    index += 1;
                    continue;
                }
            }
        }

        if let Some((section_key, label, value)) = parse_inline_semantic_label(trimmed) {
            flush_stage_section(&mut current_stage_section, frames);
            let mut semantic_values = value.into_iter().collect::<Vec<_>>();
            let mut lookahead = index + 1;
            while lookahead < lines.len() {
                let next_trimmed = lines[lookahead].trim();
                if next_trimmed.is_empty()
                    || next_trimmed.starts_with('#')
                    || parse_inline_semantic_label(next_trimmed).is_some()
                {
                    break;
                }
                semantic_values.push(sanitize_line(trim_list_prefix(next_trimmed)));
                lookahead += 1;
            }
            push_semantic_frame(
                frames,
                &format!("stage:{}", stage.stage_id),
                section_key,
                &label,
                &format!(
                    "{source_locator_prefix}#stage-{}-{}",
                    slugify_for_path(&stage.stage_id),
                    slugify_for_path(&label)
                ),
                "stage-metadata-inline-label",
                &stage_metadata_confidence(metadata_confidence, true),
                semantic_values,
            );
            index = lookahead;
            continue;
        }

        if let Some((_, _, values, _, _)) = current_stage_section.as_mut() {
            values.push(sanitize_line(trim_list_prefix(trimmed)));
        }

        index += 1;
    }

    flush_stage_section(&mut current_stage_section, frames);
}

fn collect_stage_scoped_semantic_frames_from_markdown_document(
    content: &str,
    stage: &ContractStage,
    source_locator_prefix: &str,
    frames: &mut Vec<SemanticFrame>,
) {
    let mut current_stage_section: Option<(String, String, Vec<String>, String, String)> = None;

    let flush_stage_section =
        |current_stage_section: &mut Option<(String, String, Vec<String>, String, String)>,
         frames: &mut Vec<SemanticFrame>| {
            let Some((section_key, label, values, origin_kind, confidence)) =
                current_stage_section.take()
            else {
                return;
            };
            if values.is_empty() {
                return;
            }
            push_semantic_frame(
                frames,
                &format!("stage:{}", stage.stage_id),
                &section_key,
                &label,
                &format!(
                    "{source_locator_prefix}#stage-{}-{}",
                    slugify_for_path(&stage.stage_id),
                    slugify_for_path(&label)
                ),
                &origin_kind,
                &confidence,
                values,
            );
        };

    let lines = content.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        if let Some((_, heading_text)) = parse_markdown_heading_line(trimmed) {
            if let Some(section_key) = heading_key(&heading_text) {
                if section_key != "stage_order" {
                    flush_stage_section(&mut current_stage_section, frames);
                    current_stage_section = Some((
                        section_key.to_string(),
                        heading_text.clone(),
                        Vec::new(),
                        "stage-path-alias-heading-section".to_string(),
                        stage_path_alias_confidence(&heading_text, section_key, false),
                    ));
                    index += 1;
                    continue;
                }
            }
        }

        if let Some((section_key, label, value)) = parse_inline_semantic_label(trimmed) {
            flush_stage_section(&mut current_stage_section, frames);
            let mut semantic_values = value.into_iter().collect::<Vec<_>>();
            let mut lookahead = index + 1;
            while lookahead < lines.len() {
                let next_trimmed = lines[lookahead].trim();
                if next_trimmed.is_empty()
                    || next_trimmed.starts_with('#')
                    || parse_inline_semantic_label(next_trimmed).is_some()
                {
                    break;
                }
                semantic_values.push(sanitize_line(trim_list_prefix(next_trimmed)));
                lookahead += 1;
            }
            push_semantic_frame(
                frames,
                &format!("stage:{}", stage.stage_id),
                section_key,
                &label,
                &format!(
                    "{source_locator_prefix}#stage-{}-{}",
                    slugify_for_path(&stage.stage_id),
                    slugify_for_path(&label)
                ),
                "stage-path-alias-inline-label",
                &stage_path_alias_confidence(&label, section_key, true),
                semantic_values,
            );
            index = lookahead;
            continue;
        }

        if let Some((_, _, values, _, _)) = current_stage_section.as_mut() {
            values.push(sanitize_line(trim_list_prefix(trimmed)));
        }

        index += 1;
    }

    flush_stage_section(&mut current_stage_section, frames);
}

fn collect_stage_alias_semantic_frames_from_markdown_document(
    content: &str,
    source_locator_prefix: &str,
    alias_registry: &BTreeMap<String, ContractStage>,
    frames: &mut Vec<SemanticFrame>,
) {
    if alias_registry.is_empty() {
        return;
    }

    let mut current_stage: Option<(usize, ContractStage)> = None;
    let mut current_stage_section: Option<(String, String, Vec<String>, String, String)> = None;

    let flush_stage_section =
        |current_stage: &Option<(usize, ContractStage)>,
         current_stage_section: &mut Option<(String, String, Vec<String>, String, String)>,
         frames: &mut Vec<SemanticFrame>| {
            let Some((_, stage)) = current_stage.as_ref() else {
                current_stage_section.take();
                return;
            };
            let Some((section_key, label, values, origin_kind, confidence)) =
                current_stage_section.take()
            else {
                return;
            };
            if values.is_empty() {
                return;
            }
            push_semantic_frame(
                frames,
                &format!("stage:{}", stage.stage_id),
                &section_key,
                &label,
                &format!(
                    "{source_locator_prefix}#stage-{}-{}",
                    slugify_for_path(&stage.stage_id),
                    slugify_for_path(&label)
                ),
                &origin_kind,
                &confidence,
                values,
            );
        };

    let lines = content.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        if let Some((level, heading_text)) = parse_markdown_heading_line(trimmed) {
            if let Some((current_level, _)) = current_stage.as_ref() {
                if level > *current_level {
                    if let Some(section_key) = heading_key(&heading_text) {
                        if section_key != "stage_order" {
                            flush_stage_section(&current_stage, &mut current_stage_section, frames);
                            current_stage_section = Some((
                                section_key.to_string(),
                                heading_text.clone(),
                                Vec::new(),
                                "stage-alias-heading-section".to_string(),
                                stage_alias_confidence(&heading_text, section_key, false),
                            ));
                            index += 1;
                            continue;
                        }
                    }
                }
            }

            flush_stage_section(&current_stage, &mut current_stage_section, frames);
            current_stage =
                stage_from_alias_heading(&heading_text, alias_registry).map(|stage| (level, stage));
            index += 1;
            continue;
        }

        let Some((_, stage)) = current_stage.as_ref() else {
            index += 1;
            continue;
        };

        if let Some((section_key, label, value)) = parse_inline_semantic_label(trimmed) {
            flush_stage_section(&current_stage, &mut current_stage_section, frames);
            let mut semantic_values = value.into_iter().collect::<Vec<_>>();
            let mut lookahead = index + 1;
            while lookahead < lines.len() {
                let next_trimmed = lines[lookahead].trim();
                if next_trimmed.is_empty()
                    || next_trimmed.starts_with('#')
                    || parse_inline_semantic_label(next_trimmed).is_some()
                {
                    break;
                }
                semantic_values.push(sanitize_line(trim_list_prefix(next_trimmed)));
                lookahead += 1;
            }
            push_semantic_frame(
                frames,
                &format!("stage:{}", stage.stage_id),
                section_key,
                &label,
                &format!(
                    "{source_locator_prefix}#stage-{}-{}",
                    slugify_for_path(&stage.stage_id),
                    slugify_for_path(&label)
                ),
                "stage-alias-inline-label",
                &stage_alias_confidence(&label, section_key, true),
                semantic_values,
            );
            index = lookahead;
            continue;
        }

        if let Some((_, _, values, _, _)) = current_stage_section.as_mut() {
            values.push(sanitize_line(trim_list_prefix(trimmed)));
        }

        index += 1;
    }

    flush_stage_section(&current_stage, &mut current_stage_section, frames);
}

fn extract_stage_semantic_frames_from_markdown_lines(
    lines: &[String],
    source_locator_prefix: &str,
    frames: &mut Vec<SemanticFrame>,
) -> Option<Vec<String>> {
    let mut stage_order_lines = Vec::new();
    let mut current_stage: Option<ContractStage> = None;
    let mut current_stage_section: Option<(String, String, Vec<String>, String, String)> = None;
    let mut stage_index = 0usize;
    let mut index = 0usize;

    let flush_stage_section =
        |current_stage: &Option<ContractStage>,
         current_stage_section: &mut Option<(String, String, Vec<String>, String, String)>,
         frames: &mut Vec<SemanticFrame>| {
            let Some(stage) = current_stage.as_ref() else {
                current_stage_section.take();
                return;
            };
            let Some((section_key, label, values, origin_kind, confidence)) =
                current_stage_section.take()
            else {
                return;
            };
            if values.is_empty() {
                return;
            }
            push_semantic_frame(
                frames,
                &format!("stage:{}", stage.stage_id),
                &section_key,
                &label,
                &format!(
                    "{source_locator_prefix}#stage-{}-{}",
                    slugify_for_path(&stage.stage_id),
                    slugify_for_path(&label)
                ),
                &origin_kind,
                &confidence,
                values,
            );
        };

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        if let Some((_, heading_text)) = parse_markdown_heading_line(trimmed) {
            if stage_line_has_explicit_stage_id(&heading_text) {
                flush_stage_section(&current_stage, &mut current_stage_section, frames);
                let stage = parse_stage_line(&heading_text, stage_index);
                stage_index += 1;
                let stage_scope = format!("stage:{}", stage.stage_id);
                push_semantic_frame(
                    frames,
                    &stage_scope,
                    "stage_name",
                    "stage_name",
                    &format!(
                        "{source_locator_prefix}#stage-{}",
                        slugify_for_path(&stage.stage_id)
                    ),
                    "stage-heading",
                    "exact",
                    vec![stage.stage_name.clone()],
                );
                stage_order_lines.push(format!("{}: {}", stage.stage_id, stage.stage_name));
                current_stage = Some(stage);
                index += 1;
                continue;
            }
            if current_stage.is_some() {
                if let Some(section_key) = heading_key(&heading_text) {
                    if section_key != "stage_order" {
                        flush_stage_section(&current_stage, &mut current_stage_section, frames);
                        current_stage_section = Some((
                            section_key.to_string(),
                            heading_text.clone(),
                            Vec::new(),
                            "stage-heading-section".to_string(),
                            semantic_mapping_confidence(&heading_text, section_key, false),
                        ));
                        index += 1;
                        continue;
                    }
                }
            }
        }

        if stage_line_has_explicit_stage_id(trimmed) {
            flush_stage_section(&current_stage, &mut current_stage_section, frames);
            let stage = parse_stage_line(trimmed, stage_index);
            stage_index += 1;
            let stage_scope = format!("stage:{}", stage.stage_id);
            push_semantic_frame(
                frames,
                &stage_scope,
                "stage_name",
                "stage_name",
                &format!(
                    "{source_locator_prefix}#stage-{}",
                    slugify_for_path(&stage.stage_id)
                ),
                "stage-list-item",
                "exact",
                vec![stage.stage_name.clone()],
            );
            stage_order_lines.push(format!("{}: {}", stage.stage_id, stage.stage_name));
            current_stage = Some(stage);
            index += 1;
            continue;
        }

        let Some(stage) = current_stage.as_ref() else {
            index += 1;
            continue;
        };

        if let Some((section_key, label, value)) = parse_inline_semantic_label(trimmed) {
            flush_stage_section(&current_stage, &mut current_stage_section, frames);
            let mut semantic_values = value.into_iter().collect::<Vec<_>>();
            let mut lookahead = index + 1;
            while lookahead < lines.len() {
                let next_trimmed = lines[lookahead].trim();
                if next_trimmed.is_empty()
                    || stage_line_has_explicit_stage_id(next_trimmed)
                    || parse_markdown_heading_line(next_trimmed).is_some()
                    || parse_inline_semantic_label(next_trimmed).is_some()
                {
                    break;
                }
                semantic_values.push(sanitize_line(trim_list_prefix(next_trimmed)));
                lookahead += 1;
            }
            push_semantic_frame(
                frames,
                &format!("stage:{}", stage.stage_id),
                section_key,
                &label,
                &format!(
                    "{source_locator_prefix}#stage-{}-{}",
                    slugify_for_path(&stage.stage_id),
                    slugify_for_path(&label)
                ),
                "stage-inline-label",
                &semantic_mapping_confidence(&label, section_key, true),
                semantic_values,
            );
            index = lookahead;
            continue;
        }

        if let Some((_, _, values, _, _)) = current_stage_section.as_mut() {
            values.push(sanitize_line(trim_list_prefix(trimmed)));
        }

        index += 1;
    }

    flush_stage_section(&current_stage, &mut current_stage_section, frames);

    if stage_order_lines.is_empty() {
        None
    } else {
        Some(stage_order_lines)
    }
}

fn collect_markdown_semantic_frames_from_document(
    content: &str,
    scope: &str,
    source_locator_prefix: &str,
    frames: &mut Vec<SemanticFrame>,
) {
    let mut current_heading: Option<(usize, String)> = None;
    let mut current_lines = Vec::new();

    let flush_heading = |heading: &Option<(usize, String)>,
                         lines: &mut Vec<String>,
                         frames: &mut Vec<SemanticFrame>| {
        let Some((_, heading)) = heading.as_ref() else {
            lines.clear();
            return;
        };
        let Some(section_key) = heading_key(heading) else {
            lines.clear();
            return;
        };
        let mut emitted_lines = lines.clone();
        if section_key == "stage_order" {
            if let Some(stage_order_lines) = extract_stage_semantic_frames_from_markdown_lines(
                lines,
                source_locator_prefix,
                frames,
            ) {
                emitted_lines = stage_order_lines;
            }
        }
        if emitted_lines.is_empty() {
            return;
        }
        push_semantic_frame(
            frames,
            scope,
            section_key,
            heading,
            &format!("{source_locator_prefix}#{}", slugify_for_path(heading)),
            "heading",
            &semantic_mapping_confidence(heading, section_key, false),
            emitted_lines,
        );
        lines.clear();
    };

    let lines = content.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        let current_section_key = current_heading
            .as_ref()
            .and_then(|(_, heading)| heading_key(heading));
        if let Some((level, heading_text)) = parse_markdown_heading_line(trimmed) {
            let current_heading_level = current_heading
                .as_ref()
                .map(|(value, _)| *value)
                .unwrap_or(0);
            if current_section_key == Some("stage_order") && level > current_heading_level {
                current_lines.push(trimmed.to_string());
                index += 1;
                continue;
            }
            flush_heading(&current_heading, &mut current_lines, frames);
            current_heading = Some((level, heading_text));
            index += 1;
            continue;
        }
        if current_section_key == Some("stage_order") {
            current_lines.push(trimmed.to_string());
            index += 1;
            continue;
        } else {
            if let Some((section_key, label, value)) = parse_inline_semantic_label(trimmed) {
                let mut semantic_values = value.into_iter().collect::<Vec<_>>();
                let mut lookahead = index + 1;
                while lookahead < lines.len() {
                    let next_trimmed = lines[lookahead].trim();
                    if next_trimmed.is_empty()
                        || next_trimmed.starts_with('#')
                        || parse_inline_semantic_label(next_trimmed).is_some()
                    {
                        break;
                    }
                    semantic_values.push(sanitize_line(trim_list_prefix(next_trimmed)));
                    lookahead += 1;
                }
                push_semantic_frame(
                    frames,
                    scope,
                    section_key,
                    &label,
                    &format!(
                        "{source_locator_prefix}#inline-{}",
                        slugify_for_path(&label)
                    ),
                    "inline-label",
                    &semantic_mapping_confidence(&label, section_key, true),
                    semantic_values,
                );
                current_lines.push(sanitize_line(trimmed));
                for continuation in &lines[index + 1..lookahead] {
                    current_lines.push(sanitize_line(continuation.trim()));
                }
                index = lookahead;
                continue;
            }
        }
        current_lines.push(sanitize_line(trimmed));
        index += 1;
    }

    flush_heading(&current_heading, &mut current_lines, frames);
}

fn parse_json_sections(raw_source: &str) -> Result<ParsedBlueprintSource, CoreError> {
    let value = serde_json::from_str::<serde_json::Value>(raw_source).map_err(|error| {
        CoreError::TooAmbiguousToNormalize {
            reason: format!("json blueprint source could not be parsed: {error}"),
        }
    })?;
    let mut frames = Vec::new();
    collect_json_semantic_frames(&value, "package", &[], &mut frames);
    let mut parsed = source_from_semantic_frames(frames);
    if !parsed.sections.contains_key("stage_order") {
        if let Some(stage_order) = json_stage_order_from_structured_value(
            value
                .as_object()
                .and_then(json_stage_collection_from_object),
        ) {
            push_semantic_frame(
                &mut parsed.semantic_frames,
                "package",
                "stage_order",
                "stage-collection",
                "$.stages",
                "stage-collection",
                "heuristic",
                stage_order,
            );
            parsed = source_from_semantic_frames(parsed.semantic_frames);
        }
    }
    Ok(parsed)
}

fn parse_toml_sections(raw_source: &str) -> Result<ParsedBlueprintSource, CoreError> {
    let value = toml::from_str::<toml::Value>(raw_source).map_err(|error| {
        CoreError::TooAmbiguousToNormalize {
            reason: format!("toml blueprint source could not be parsed: {error}"),
        }
    })?;
    let mut frames = Vec::new();
    collect_toml_semantic_frames(&value, "package", &[], &mut frames);
    let mut parsed = source_from_semantic_frames(frames);
    if !parsed.sections.contains_key("stage_order") {
        if let Some(stage_order) = toml_stage_order_from_structured_value(
            value.as_table().and_then(toml_stage_collection_from_table),
        ) {
            push_semantic_frame(
                &mut parsed.semantic_frames,
                "package",
                "stage_order",
                "stage-collection",
                "stages",
                "stage-collection",
                "heuristic",
                stage_order,
            );
            parsed = source_from_semantic_frames(parsed.semantic_frames);
        }
    }
    Ok(parsed)
}

fn parse_workspace_source(raw_source: &str) -> ParsedBlueprintSource {
    let blocks = parse_source_file_blocks(raw_source);
    let mut frames = Vec::new();

    if blocks.is_empty() {
        collect_markdown_semantic_frames_from_document(
            raw_source,
            "package",
            "<inline-source>",
            &mut frames,
        );
        let alias_registry = stage_alias_registry_from_frames(&frames);
        collect_stage_alias_semantic_frames_from_markdown_document(
            raw_source,
            "<inline-source>",
            &alias_registry,
            &mut frames,
        );
    } else {
        for block in blocks {
            if block.path.starts_with("blueprint/stages/") {
                if let Some((stage_id, stage_name)) =
                    extract_stage_metadata(&block.path, &block.content)
                {
                    let stage_scope = format!("stage:{stage_id}");
                    push_semantic_frame(
                        &mut frames,
                        &stage_scope,
                        "stage_name",
                        "stage_name",
                        &format!("{}#stage_name", block.path),
                        "stage-source-file",
                        "exact",
                        vec![stage_name],
                    );
                    collect_markdown_semantic_frames_from_document(
                        &block.content,
                        &stage_scope,
                        &block.path,
                        &mut frames,
                    );
                }
            } else {
                collect_markdown_semantic_frames_from_document(
                    &block.content,
                    "package",
                    &block.path,
                    &mut frames,
                );
                let alias_registry = stage_alias_registry_from_frames(&frames);
                if let Some((stage, metadata_confidence)) =
                    stage_from_document_metadata(&block.content, &alias_registry)
                {
                    collect_stage_metadata_semantic_frames_from_markdown_document(
                        &block.content,
                        &stage,
                        &metadata_confidence,
                        &format!("{}#stage-metadata", block.path),
                        &mut frames,
                    );
                } else if let Some(stage) = stage_from_path_alias(&block.path, &alias_registry) {
                    collect_stage_scoped_semantic_frames_from_markdown_document(
                        &block.content,
                        &stage,
                        &format!("{}#path-alias", block.path),
                        &mut frames,
                    );
                    collect_stage_alias_semantic_frames_from_markdown_document(
                        &block.content,
                        &block.path,
                        &alias_registry,
                        &mut frames,
                    );
                } else {
                    collect_stage_alias_semantic_frames_from_markdown_document(
                        &block.content,
                        &block.path,
                        &alias_registry,
                        &mut frames,
                    );
                }
            }
        }
    }
    let mut parsed = source_from_semantic_frames(frames);
    if parsed.sections.is_empty() && !raw_source.trim().is_empty() {
        parsed.sections.insert(
            "purpose".to_string(),
            raw_source
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(sanitize_line)
                .take(3)
                .collect(),
        );
        parsed.semantic_frames = complete_semantic_frames_with_fallback(
            &parsed.semantic_frames,
            &parsed.sections,
            &parsed.stage_sections,
            &["purpose".to_string()],
            &[],
        );
    }
    parsed
}

fn json_stage_collection_from_object<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
) -> Option<&'a serde_json::Value> {
    object
        .get("stages")
        .or_else(|| object.get("stage_details"))
        .or_else(|| object.get("phases"))
        .or_else(|| object.get("milestones"))
        .or_else(|| object.get("steps"))
}

fn toml_stage_collection_from_table<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
) -> Option<&'a toml::Value> {
    table
        .get("stages")
        .or_else(|| table.get("stage_details"))
        .or_else(|| table.get("phases"))
        .or_else(|| table.get("milestones"))
        .or_else(|| table.get("steps"))
}

fn json_stage_order_from_structured_value(
    value: Option<&serde_json::Value>,
) -> Option<Vec<String>> {
    let serde_json::Value::Array(stages) = value? else {
        return None;
    };
    let lines = stages
        .iter()
        .filter_map(|stage| {
            let stage = stage.as_object()?;
            let stage_id = stage
                .get("stage_id")
                .and_then(|value| value.as_str())
                .or_else(|| stage.get("id").and_then(|value| value.as_str()))?;
            let stage_name = stage
                .get("stage_name")
                .and_then(|value| value.as_str())
                .or_else(|| stage.get("name").and_then(|value| value.as_str()))
                .unwrap_or(stage_id);
            Some(format!("{stage_id}: {stage_name}"))
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        None
    } else {
        Some(lines)
    }
}

fn toml_stage_order_from_structured_value(value: Option<&toml::Value>) -> Option<Vec<String>> {
    let toml::Value::Array(stages) = value? else {
        return None;
    };
    let lines = stages
        .iter()
        .filter_map(|stage| {
            let stage = stage.as_table()?;
            let stage_id = stage
                .get("stage_id")
                .and_then(|value| value.as_str())
                .or_else(|| stage.get("id").and_then(|value| value.as_str()))?;
            let stage_name = stage
                .get("stage_name")
                .and_then(|value| value.as_str())
                .or_else(|| stage.get("name").and_then(|value| value.as_str()))
                .unwrap_or(stage_id);
            Some(format!("{stage_id}: {stage_name}"))
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        None
    } else {
        Some(lines)
    }
}

fn collect_source_files(input: &BlueprintAuthorInput) -> Vec<String> {
    let mut source_files = BTreeSet::new();

    for source in [
        input.workspace_source_text.as_deref(),
        input.source_text.as_deref(),
    ]
    .iter()
    .flatten()
    {
        for block in parse_source_file_blocks(source) {
            source_files.insert(block.path);
        }
    }

    if source_files.is_empty() {
        if let Some(source_path) = &input.source_path {
            for path in source_path.split(" + ") {
                let trimmed = path.trim();
                if !trimmed.is_empty() {
                    source_files.insert(normalize_repo_relative_path(trimmed));
                }
            }
        }
    }

    if source_files.is_empty() {
        if input.workspace_source_text.is_some() {
            source_files.insert("<workspace-blueprint-bundle>".to_string());
        }
        if input.source_text.is_some() {
            source_files.insert("<inline-source>".to_string());
        }
        if !input.source_summary.trim().is_empty() {
            source_files.insert("<source-summary>".to_string());
        }
    }

    source_files.into_iter().collect()
}

fn collect_dropped_sections(sources: &[Option<&str>]) -> Vec<String> {
    let mut dropped = BTreeSet::new();

    for source in sources.iter().flatten() {
        for line in source.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with('#') {
                continue;
            }

            let heading = trimmed.trim_start_matches('#').trim();
            if heading.is_empty()
                || heading_key(heading).is_some()
                || ignored_unmapped_heading(heading)
            {
                continue;
            }
            dropped.insert(heading.to_string());
        }
    }

    dropped.into_iter().collect()
}

fn collect_semantic_hints(sources: &[Option<&str>]) -> Vec<String> {
    let mut hints = BTreeSet::new();

    for source in sources.iter().flatten() {
        let lowered = source.to_lowercase();
        if lowered.contains("acceptance criteria") || lowered.contains("acceptance_criteria") {
            hints.insert("mapped `Acceptance Criteria` into `required_verification`".to_string());
        }
        if lowered.contains("success criteria") || lowered.contains("success_criteria") {
            hints.insert("mapped `Success Criteria` into `required_verification`".to_string());
        }
        if lowered.contains("validation plan") || lowered.contains("test plan") {
            hints.insert(
                "mapped validation-oriented headings into `required_verification`".to_string(),
            );
        }
        if lowered.contains("constraint") || lowered.contains("guardrail") {
            hints.insert("mapped constraints or guardrails into `truth_rules`".to_string());
        }
        if lowered.contains("out of scope")
            || lowered.contains("out_of_scope")
            || lowered.contains("excluded scope")
            || lowered.contains("exclusions")
        {
            hints.insert("mapped exclusion-oriented scope into `forbidden_scope`".to_string());
        }
        if lowered.contains("in scope") || lowered.contains("included scope") {
            hints.insert("mapped inclusion-oriented scope into `allowed_scope`".to_string());
        }
        if lowered.contains("assumption")
            || lowered.contains("dependency")
            || lowered.contains("risk")
        {
            hints.insert(
                "mapped assumptions, dependencies, or risks into `review_focus`".to_string(),
            );
        }
        if lowered.contains("\"phases\"")
            || lowered.contains("\"milestones\"")
            || lowered.contains("\"steps\"")
            || lowered.contains("## phases")
            || lowered.contains("## milestones")
            || lowered.contains("## phase order")
        {
            hints.insert(
                "mapped phase or milestone structures into explicit `stage_order` and stage detail"
                    .to_string(),
            );
        }
    }

    hints.into_iter().collect()
}

fn collect_semantic_risks(dropped_sections: &[String]) -> Vec<String> {
    let mut risks = BTreeSet::new();

    for heading in dropped_sections {
        let lowered = heading.to_lowercase();
        if lowered.contains("assumption") {
            risks.insert(format!(
                "dropped heading `{heading}` may contain execution assumptions that were not normalized"
            ));
        }
        if lowered.contains("dependency") {
            risks.insert(format!(
                "dropped heading `{heading}` may contain dependency semantics that were not normalized"
            ));
        }
        if lowered.contains("risk") {
            risks.insert(format!(
                "dropped heading `{heading}` may contain delivery or review risks that were not normalized"
            ));
        }
        if lowered.contains("acceptance") || lowered.contains("success criteria") {
            risks.insert(format!(
                "dropped heading `{heading}` may contain verification semantics that were not normalized"
            ));
        }
    }

    risks.into_iter().collect()
}

fn ignored_unmapped_heading(heading: &str) -> bool {
    matches!(
        heading.to_lowercase().as_str(),
        "external blueprint"
            | "update input"
            | "input"
            | "source file"
            | "stage document"
            | "stage"
            | "workflow overview"
            | "author report"
    )
}

fn extract_stage_metadata(path: &str, content: &str) -> Option<(String, String)> {
    let mut stage_id = None;
    let mut stage_name = None;

    for line in content.lines() {
        let sanitized = sanitize_line(line);
        let normalized = sanitized.trim_matches('`').trim();
        let lowered = normalized.to_lowercase();

        if let Some((left, right)) = normalized.split_once(':') {
            let key = left.trim().trim_matches('`').to_lowercase();
            let value = right.trim().trim_matches('`').trim().to_string();
            if key == "stage_id" && !value.is_empty() {
                stage_id = Some(value);
                continue;
            }
            if key == "stage_name" && !value.is_empty() {
                stage_name = Some(value);
                continue;
            }
        }

        if stage_id.is_none() && looks_like_stage_id(&lowered) {
            stage_id = Some(normalized.trim_matches('`').to_string());
        }
    }

    let file_stem = Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let mut stem_parts = file_stem.splitn(2, '-');
    let stem_prefix = stem_parts.next().unwrap_or_default();
    let stem_tail = stem_parts.next().unwrap_or_default();

    if stage_id.is_none()
        && !stem_prefix.is_empty()
        && stem_prefix.chars().all(|ch| ch.is_ascii_digit())
    {
        stage_id = Some(format!("stage-{stem_prefix}"));
    }

    if stage_name.is_none() && !stem_tail.is_empty() {
        let fallback_name = stem_tail
            .split('-')
            .filter(|part| !part.is_empty() && !looks_like_stage_id(part))
            .collect::<Vec<_>>()
            .join(" ");
        if !fallback_name.is_empty() {
            stage_name = Some(fallback_name);
        }
    }

    match (stage_id, stage_name) {
        (Some(stage_id), Some(stage_name)) => Some((stage_id, stage_name)),
        _ => None,
    }
}

fn worktree_protocol_is_empty(protocol: &WorktreeProtocol) -> bool {
    protocol.model.trim().is_empty()
        && protocol.parallel_worktree_policy.is_empty()
        && protocol.shared_authority_paths.is_empty()
        && protocol.sync_rule.is_empty()
        && protocol.merge_back_rule.is_empty()
        && protocol.cleanup_rule.is_empty()
        && protocol.roles.is_empty()
}

fn effective_worktree_protocol(
    primary: &WorktreeProtocol,
    fallback: &WorktreeProtocol,
) -> WorktreeProtocol {
    let mut protocol = if worktree_protocol_is_empty(primary) {
        fallback.clone()
    } else {
        primary.clone()
    };
    normalize_worktree_protocol(&mut protocol);
    protocol
}

fn build_worktree_protocol(
    sections: &BTreeMap<String, Vec<String>>,
    stages: &[ContractStage],
    stage_documents: &[StageDocument],
    module_catalog: &ModuleCatalog,
    paths: &ContractPaths,
) -> Result<WorktreeProtocol, CoreError> {
    let mut protocol = WorktreeProtocol {
        schema_version: WORKTREE_PROTOCOL_SCHEMA_VERSION.to_string(),
        model: section_first_value(sections, "worktree_model")
            .unwrap_or_else(|| "stage-isolated-worktree".to_string()),
        parallel_worktree_policy: sections
            .get("parallel_worktree_policy")
            .cloned()
            .unwrap_or_else(|| default_parallel_worktree_policy_for_model("stage-isolated-worktree")),
        shared_authority_paths: sections
            .get("shared_authority_paths")
            .cloned()
            .unwrap_or_else(|| default_shared_authority_paths(paths)),
        sync_rule: sections
            .get("worktree_sync_rule")
            .cloned()
            .unwrap_or_else(|| default_worktree_sync_rule_for_model("stage-isolated-worktree")),
        merge_back_rule: sections
            .get("worktree_merge_back_rule")
            .cloned()
            .unwrap_or_else(|| default_worktree_merge_back_rule_for_model("stage-isolated-worktree")),
        cleanup_rule: sections
            .get("worktree_cleanup_rule")
            .cloned()
            .unwrap_or_else(|| default_worktree_cleanup_rule_for_model("stage-isolated-worktree")),
        roles: parse_worktree_role_lines(
            sections.get("worktree_roles"),
            stages,
            stage_documents,
            module_catalog,
        )?,
    };

    if !sections.contains_key("parallel_worktree_policy") {
        protocol.parallel_worktree_policy =
            default_parallel_worktree_policy_for_model(&protocol.model);
    }
    if !sections.contains_key("worktree_sync_rule") {
        protocol.sync_rule = default_worktree_sync_rule_for_model(&protocol.model);
    }
    if !sections.contains_key("worktree_merge_back_rule") {
        protocol.merge_back_rule = default_worktree_merge_back_rule_for_model(&protocol.model);
    }
    if !sections.contains_key("worktree_cleanup_rule") {
        protocol.cleanup_rule = default_worktree_cleanup_rule_for_model(&protocol.model);
    }

    if protocol.roles.is_empty() {
        protocol.roles = default_worktree_roles(stages, stage_documents);
    }

    normalize_worktree_protocol(&mut protocol);
    Ok(protocol)
}

fn sections_with_worktree_protocol(
    sections: &BTreeMap<String, Vec<String>>,
    protocol: &WorktreeProtocol,
) -> BTreeMap<String, Vec<String>> {
    let mut merged = sections.clone();
    merged.insert("worktree_model".to_string(), vec![protocol.model.clone()]);
    merged.insert(
        "parallel_worktree_policy".to_string(),
        protocol.parallel_worktree_policy.clone(),
    );
    merged.insert(
        "shared_authority_paths".to_string(),
        protocol.shared_authority_paths.clone(),
    );
    merged.insert("worktree_roles".to_string(), render_worktree_role_lines(protocol));
    merged.insert("worktree_sync_rule".to_string(), protocol.sync_rule.clone());
    merged.insert(
        "worktree_merge_back_rule".to_string(),
        protocol.merge_back_rule.clone(),
    );
    merged.insert(
        "worktree_cleanup_rule".to_string(),
        protocol.cleanup_rule.clone(),
    );
    merged
}

fn normalize_worktree_protocol(protocol: &mut WorktreeProtocol) {
    protocol.schema_version = WORKTREE_PROTOCOL_SCHEMA_VERSION.to_string();
    protocol.model = protocol.model.trim().to_string();
    protocol.parallel_worktree_policy = normalize_unique_lines(&protocol.parallel_worktree_policy);
    protocol.shared_authority_paths = normalize_unique_paths(&protocol.shared_authority_paths);
    protocol.sync_rule = normalize_unique_lines(&protocol.sync_rule);
    protocol.merge_back_rule = normalize_unique_lines(&protocol.merge_back_rule);
    protocol.cleanup_rule = normalize_unique_lines(&protocol.cleanup_rule);
    for role in &mut protocol.roles {
        role.role_id = role.role_id.trim().to_string();
        role.branch_pattern = role.branch_pattern.trim().to_string();
        role.stage_ids = normalize_unique_lines(&role.stage_ids);
        role.module_ids = normalize_unique_lines(&role.module_ids);
        role.exclusive_paths = normalize_unique_paths(&role.exclusive_paths);
    }
}

fn normalize_unique_lines(values: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            normalized.push(trimmed.to_string());
        }
    }
    normalized
}

fn normalize_unique_paths(values: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let trimmed = value.trim().trim_matches('`');
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            continue;
        }
        let path = normalize_repo_relative_path(trimmed);
        if seen.insert(path.clone()) {
            normalized.push(path);
        }
    }
    normalized
}

fn section_first_value(sections: &BTreeMap<String, Vec<String>>, key: &str) -> Option<String> {
    sections
        .get(key)
        .and_then(|values| values.first())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn default_parallel_worktree_policy_for_model(model: &str) -> Vec<String> {
    let first_rule = match model {
        "module-isolated-worktree" => {
            "Use one active implementation worktree per module role when parallel execution is necessary."
        }
        _ => "Use one active implementation worktree per stage role when parallel execution is necessary.",
    };
    vec![
        first_rule.to_string(),
        "Shared authority and workflow changes must be synchronized before merge-back."
            .to_string(),
    ]
}

fn default_worktree_sync_rule_for_model(model: &str) -> Vec<String> {
    let first_rule = match model {
        "module-isolated-worktree" => {
            "Sync each active worktree from the main integration branch before starting a new module task."
        }
        _ => {
            "Sync each active worktree from the main integration branch before starting a new stage task."
        }
    };
    vec![
        first_rule.to_string(),
        "Re-run blueprint validation after any shared authority or workflow sync.".to_string(),
    ]
}

fn default_worktree_merge_back_rule_for_model(model: &str) -> Vec<String> {
    let first_rule = match model {
        "module-isolated-worktree" => {
            "Only merge a worktree back after its module-scoped changes pass validation and contract recheck."
        }
        _ => {
            "Only merge a worktree back after its stage-scoped changes pass validation and contract recheck."
        }
    };
    vec![
        first_rule.to_string(),
        "Shared authority and workflow updates must merge before downstream worktrees rebase."
            .to_string(),
    ]
}

fn default_worktree_cleanup_rule_for_model(model: &str) -> Vec<String> {
    let first_rule = match model {
        "module-isolated-worktree" => {
            "Delete or recycle a worktree after its module-scoped changes are merged and the next handoff is written."
        }
        _ => {
            "Delete or recycle a worktree after its stage changes are merged and the next-stage handoff is written."
        }
    };
    vec![
        first_rule.to_string(),
        "Only clean up a worktree after shared authority and workflow updates are merged or handed off to the coordinating integration path."
            .to_string(),
    ]
}

fn default_shared_authority_paths(paths: &ContractPaths) -> Vec<String> {
    vec![
        paths.authority_root.clone(),
        paths.workflow_root.clone(),
        paths.contract_root.clone(),
    ]
}

fn default_worktree_roles(
    stages: &[ContractStage],
    stage_documents: &[StageDocument],
) -> Vec<WorktreeRoleSpec> {
    stages
        .iter()
        .map(|stage| {
            let role_id = format!(
                "{}-{}",
                slugify_for_path(&stage.stage_id),
                slugify_for_path(&stage.stage_name)
            );
            let exclusive_paths = stage_documents
                .iter()
                .find(|document| document.stage_id == stage.stage_id)
                .map(|document| vec![document.path.clone()])
                .unwrap_or_default();
            WorktreeRoleSpec {
                role_id: role_id.clone(),
                branch_pattern: format!("codex/{role_id}"),
                stage_ids: vec![stage.stage_id.clone()],
                module_ids: Vec::new(),
                exclusive_paths,
            }
        })
        .collect()
}

fn render_worktree_role_lines(protocol: &WorktreeProtocol) -> Vec<String> {
    protocol
        .roles
        .iter()
        .map(|role| {
            format!(
                "{} | branch={} | stages={} | modules={} | paths={}",
                role.role_id,
                role.branch_pattern,
                csv_or_none(&role.stage_ids),
                csv_or_none(&role.module_ids),
                csv_or_none(&role.exclusive_paths)
            )
        })
        .collect()
}

fn csv_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(",")
    }
}

fn parse_worktree_role_lines(
    lines: Option<&Vec<String>>,
    stages: &[ContractStage],
    stage_documents: &[StageDocument],
    _module_catalog: &ModuleCatalog,
) -> Result<Vec<WorktreeRoleSpec>, CoreError> {
    let Some(lines) = lines else {
        return Ok(Vec::new());
    };
    let known_stage_ids = stages
        .iter()
        .map(|stage| stage.stage_id.clone())
        .collect::<BTreeSet<_>>();
    let stage_paths = stage_documents
        .iter()
        .map(|document| (document.stage_id.clone(), document.path.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut roles = Vec::new();

    for line in lines {
        let sanitized = sanitize_line(line);
        if sanitized.trim().is_empty() {
            continue;
        }
        let segments = sanitized
            .split('|')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        let Some(first_segment) = segments.first() else {
            continue;
        };
        let mut role = WorktreeRoleSpec {
            role_id: first_segment.trim().to_string(),
            branch_pattern: String::new(),
            stage_ids: Vec::new(),
            module_ids: Vec::new(),
            exclusive_paths: Vec::new(),
        };

        for segment in segments.iter().skip(1) {
            let Some((raw_key, raw_value)) = segment.split_once('=') else {
                continue;
            };
            let key = raw_key.trim().to_lowercase().replace(' ', "_");
            let values = split_csv_field(raw_value);
            match key.as_str() {
                "branch" | "branch_pattern" => {
                    role.branch_pattern = raw_value.trim().to_string();
                }
                "stages" | "stage_ids" => {
                    role.stage_ids = values;
                }
                "modules" | "module_ids" => {
                    role.module_ids = values;
                }
                "paths" | "exclusive_paths" => {
                    role.exclusive_paths = values
                        .iter()
                        .map(|value| normalize_repo_relative_path(value))
                        .collect();
                }
                _ => {}
            }
        }

        if role.branch_pattern.trim().is_empty() {
            role.branch_pattern = format!("codex/{}", slugify_for_path(&role.role_id));
        }
        if role.stage_ids.is_empty() {
            if known_stage_ids.contains(&role.role_id) {
                role.stage_ids.push(role.role_id.clone());
            } else if let Some(stage) = stages.iter().find(|stage| {
                slugify_for_path(&stage.stage_name) == slugify_for_path(&role.role_id)
            }) {
                role.stage_ids.push(stage.stage_id.clone());
            }
        }
        if role.exclusive_paths.is_empty() && role.stage_ids.len() == 1 {
            if let Some(path) = stage_paths.get(&role.stage_ids[0]) {
                role.exclusive_paths.push(path.clone());
            }
        }
        roles.push(role);
    }

    Ok(roles)
}

fn split_csv_field(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
        .map(str::to_string)
        .collect()
}

fn stage_doc_path(index: usize, stage: &ContractStage, stages_root: &str) -> String {
    format!(
        "{}/{:02}-{}.md",
        stages_root,
        index + 1,
        slugify_for_path(&stage.stage_name)
    )
}

fn unique_stage_doc_path(index: usize, stage: &ContractStage, stages_root: &str) -> String {
    format!(
        "{}/{:02}-{}-{}.md",
        stages_root,
        index + 1,
        slugify_for_path(&stage.stage_id),
        slugify_for_path(&stage.stage_name)
    )
}

fn unique_stage_doc_path_with_suffix(
    index: usize,
    stage: &ContractStage,
    suffix: usize,
    stages_root: &str,
) -> String {
    format!(
        "{}/{:02}-{}-{}-{}.md",
        stages_root,
        index + 1,
        slugify_for_path(&stage.stage_id),
        slugify_for_path(&stage.stage_name),
        suffix
    )
}

fn slugify_for_path(input: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_dash = false;

    for ch in input.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_was_dash = false;
        } else if !previous_was_dash {
            slug.push('-');
            previous_was_dash = true;
        }
    }

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "stage".to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_sections(raw_source: &str) -> BTreeMap<String, Vec<String>> {
    let mut sections = BTreeMap::<String, Vec<String>>::new();
    let mut current_key: Option<String> = None;

    for line in raw_source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('#') {
            current_key = heading_key(trimmed.trim_start_matches('#').trim()).map(str::to_string);
            if let Some(key) = &current_key {
                sections.entry(key.clone()).or_default();
            }
            continue;
        }

        if let Some(key) = &current_key {
            sections
                .entry(key.clone())
                .or_default()
                .push(sanitize_line(trimmed));
        }
    }

    sections.retain(|_, value| !value.is_empty());
    if sections.is_empty() && !raw_source.trim().is_empty() {
        sections.insert(
            "purpose".to_string(),
            raw_source
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(sanitize_line)
                .take(3)
                .collect(),
        );
    }

    sections
}

fn heading_key(heading: &str) -> Option<&'static str> {
    let lowered = heading.to_lowercase();
    if lowered.contains("purpose") || lowered.contains("goal") || lowered == "intent" {
        return Some("purpose");
    }
    if lowered.contains("authority scope") || lowered == "authority" {
        return Some("authority_scope");
    }
    if lowered.contains("truth rule") || lowered == "truth rules" {
        return Some("truth_rules");
    }
    if lowered.contains("conflict") {
        return Some("conflict_resolution");
    }
    if lowered.contains("non-goal") || lowered.contains("non goal") {
        return Some("non_goals");
    }
    if lowered.contains("stage order")
        || lowered.contains("phase order")
        || lowered == "workflow"
        || lowered == "workflow overview"
        || lowered == "phases"
        || lowered == "milestones"
    {
        return Some("stage_order");
    }
    if lowered.contains("cross-stage split rule") || lowered.contains("cross stage split rule") {
        return Some("cross_stage_split_rule");
    }
    if lowered.contains("stop conditions") || lowered.contains("stop condition") {
        return Some("stop_conditions");
    }
    if lowered.contains("entry rule") {
        return Some("entry_rule");
    }
    if lowered.contains("exit gate") {
        return Some("exit_gate");
    }
    if lowered.contains("worktree model") {
        return Some("worktree_model");
    }
    if lowered.contains("parallel worktree policy") {
        return Some("parallel_worktree_policy");
    }
    if lowered.contains("shared authority paths") {
        return Some("shared_authority_paths");
    }
    if lowered.contains("worktree roles") {
        return Some("worktree_roles");
    }
    if lowered.contains("worktree sync rule") {
        return Some("worktree_sync_rule");
    }
    if lowered.contains("worktree merge back rule") {
        return Some("worktree_merge_back_rule");
    }
    if lowered.contains("worktree cleanup rule") {
        return Some("worktree_cleanup_rule");
    }
    if lowered.contains("allowed scope") {
        return Some("allowed_scope");
    }
    if lowered == "scope" || lowered.contains("in scope") || lowered.contains("included scope") {
        return Some("allowed_scope");
    }
    if lowered.contains("forbidden scope") {
        return Some("forbidden_scope");
    }
    if lowered.contains("out of scope")
        || lowered.contains("excluded scope")
        || lowered.contains("exclusions")
    {
        return Some("forbidden_scope");
    }
    if lowered.contains("deliverable") {
        return Some("deliverables");
    }
    if lowered.contains("verification")
        || lowered.contains("acceptance criteria")
        || lowered.contains("success criteria")
        || lowered.contains("validation plan")
        || lowered.contains("test plan")
    {
        return Some("required_verification");
    }
    if lowered.contains("review focus")
        || lowered.contains("assumption")
        || lowered.contains("dependency")
        || lowered.contains("risk")
    {
        return Some("review_focus");
    }
    if lowered.contains("constraint") || lowered.contains("guardrail") {
        return Some("truth_rules");
    }
    if lowered.contains("advance rule") {
        return Some("advance_rule");
    }
    if lowered.contains("repair routing") {
        return Some("repair_routing");
    }
    if lowered.contains("module language") {
        return Some("module_language_policy");
    }
    None
}

fn mode_from_source_type(source_type: &str) -> Result<AuthorMode, CoreError> {
    match source_type {
        "product-idea" => Ok(AuthorMode::NewProject),
        "external-blueprint" => Ok(AuthorMode::ImportBlueprint),
        "existing-blueprint" => Ok(AuthorMode::UpdateBlueprint),
        "existing-contract" => Ok(AuthorMode::RecompileContract),
        _ => Err(CoreError::NormalizationEvidenceMissing {
            field: "source_type".to_string(),
        }),
    }
}

fn validate_contract_alignment(
    project_contract: &ProjectContractToml,
    resolved_contract: &ResolvedContract,
) -> Result<(), CoreError> {
    if project_contract.project_name != resolved_contract.project_name {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "project_name".to_string(),
        });
    }
    if project_contract.workflow_mode != resolved_contract.workflow_mode {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "workflow_mode".to_string(),
        });
    }
    if project_contract.paths != resolved_contract.paths {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "paths".to_string(),
        });
    }
    if project_contract.stages != resolved_contract.stages {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "stages".to_string(),
        });
    }
    let project_worktree = effective_worktree_protocol(&project_contract.worktree, &resolved_contract.worktree);
    let resolved_worktree =
        effective_worktree_protocol(&resolved_contract.worktree, &project_contract.worktree);
    if project_worktree != resolved_worktree {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "worktree".to_string(),
        });
    }
    Ok(())
}

fn validate_workflow_alignment(
    workflow_doc: &str,
    resolved_contract: &ResolvedContract,
    stage_documents: &[StageDocument],
    module_catalog: &ModuleCatalog,
) -> Result<(), CoreError> {
    let workflow_sections = parse_sections(workflow_doc);
    let Some(stage_lines) = workflow_sections.get("stage_order") else {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "workflow.stage_order".to_string(),
        });
    };
    let workflow_stages = build_stages(
        Some(stage_lines),
        AuthorMode::RecompileContract,
        &default_normalization_policy()?,
    )?;
    if workflow_stages.len() != resolved_contract.stages.len() {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "workflow.stage_order".to_string(),
        });
    }
    for (workflow_stage, resolved_stage) in workflow_stages.iter().zip(&resolved_contract.stages) {
        if workflow_stage.stage_id != resolved_stage.stage_id
            || workflow_stage.stage_name != resolved_stage.stage_name
        {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("workflow.stage_order.{}", resolved_stage.stage_id),
            });
        }
    }
    let workflow_worktree = build_worktree_protocol(
        &workflow_sections,
        &resolved_contract.stages,
        stage_documents,
        module_catalog,
        &resolved_contract.paths,
    )?;
    let resolved_worktree =
        effective_worktree_protocol(&resolved_contract.worktree, &workflow_worktree);
    if workflow_worktree != resolved_worktree {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "workflow.worktree".to_string(),
        });
    }
    Ok(())
}

fn validate_worktree_protocol_alignment(
    protocol: &WorktreeProtocol,
    resolved_contract: &ResolvedContract,
    module_catalog: &ModuleCatalog,
) -> Result<(), CoreError> {
    validate_schema_version(
        "worktree-protocol",
        &protocol.schema_version,
        WORKTREE_PROTOCOL_SCHEMA_VERSION,
    )?;
    if protocol.model.trim().is_empty() {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "worktree.model".to_string(),
        });
    }
    let parallel_worktree_policy = normalize_unique_lines(&protocol.parallel_worktree_policy);
    let sync_rule = normalize_unique_lines(&protocol.sync_rule);
    let merge_back_rule = normalize_unique_lines(&protocol.merge_back_rule);
    let cleanup_rule = normalize_unique_lines(&protocol.cleanup_rule);
    if !protocol.roles.is_empty() {
        if !matches!(
            protocol.model.as_str(),
            "stage-isolated-worktree" | "module-isolated-worktree"
        ) {
            return Err(CoreError::ResolvedContractInconsistent {
                field: "worktree.model".to_string(),
            });
        }
        for (field, values) in [
            ("worktree.parallel_worktree_policy", &parallel_worktree_policy),
            ("worktree.sync_rule", &sync_rule),
            ("worktree.merge_back_rule", &merge_back_rule),
            ("worktree.cleanup_rule", &cleanup_rule),
        ] {
            if values.is_empty() {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: field.to_string(),
                });
            }
            if values.iter().any(|value| worktree_rule_is_placeholder(value)) {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: field.to_string(),
                });
            }
            if !worktree_rule_set_has_actionable_entry(field, values, protocol) {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: field.to_string(),
                });
            }
            if !worktree_rule_set_mentions_model_scope(field, &protocol.model, values)
                || values
                    .iter()
                    .any(|value| worktree_rule_conflicts_with_model(field, &protocol.model, value))
            {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: field.to_string(),
                });
            }
        }
        let shared_authority_paths = protocol
            .shared_authority_paths
            .iter()
            .map(|path| normalize_repo_relative_path(path))
            .collect::<Vec<_>>();
        if !shared_authority_paths.is_empty() {
            for (field, values) in [
                ("worktree.parallel_worktree_policy", &parallel_worktree_policy),
                ("worktree.sync_rule", &sync_rule),
                ("worktree.merge_back_rule", &merge_back_rule),
                ("worktree.cleanup_rule", &cleanup_rule),
            ] {
                if !worktree_rule_set_mentions_shared_authority(values, &shared_authority_paths) {
                    return Err(CoreError::ResolvedContractInconsistent {
                        field: field.to_string(),
                    });
                }
            }
        }
    }
    let mut role_ids = BTreeSet::new();
    let stage_ids = resolved_contract
        .stages
        .iter()
        .map(|stage| stage.stage_id.clone())
        .collect::<BTreeSet<_>>();
    let module_ids = module_catalog
        .modules
        .iter()
        .map(|module| module.module_id.clone())
        .collect::<BTreeSet<_>>();
    let mut coverage = BTreeMap::<String, usize>::new();
    let shared_authority_paths = protocol
        .shared_authority_paths
        .iter()
        .map(|path| normalize_repo_relative_path(path))
        .collect::<Vec<_>>();
    let mut branch_patterns = BTreeMap::<String, String>::new();
    let mut module_owners = BTreeMap::<String, String>::new();
    let mut exclusive_path_owners = Vec::<(String, String)>::new();

    for path in &protocol.shared_authority_paths {
        if path.contains('\\') {
            return Err(CoreError::ResolvedContractInconsistent {
                field: "worktree.shared_authority_paths".to_string(),
            });
        }
    }
    if !protocol.roles.is_empty() && shared_authority_paths.is_empty() {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "worktree.shared_authority_paths".to_string(),
        });
    }
    for role in &protocol.roles {
        if role.role_id.trim().is_empty() || !role_ids.insert(role.role_id.clone()) {
            return Err(CoreError::ResolvedContractInconsistent {
                field: "worktree.roles.role_id".to_string(),
            });
        }
        if role.branch_pattern.trim().is_empty() || !worktree_branch_pattern_is_valid(&role.branch_pattern) {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("worktree.roles.{}.branch_pattern", role.role_id),
            });
        }
        if let Some(existing_role) =
            branch_patterns.insert(role.branch_pattern.clone(), role.role_id.clone())
        {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!(
                    "worktree.roles.{}.branch_pattern_duplicate:{}",
                    role.role_id, existing_role
                ),
            });
        }
        if let Some((existing_pattern, existing_role)) = branch_patterns.iter().find(|(existing_pattern, existing_role)| {
            *existing_role != &role.role_id
                && worktree_branch_patterns_overlap(existing_pattern, &role.branch_pattern)
        }) {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!(
                    "worktree.roles.{}.branch_pattern_overlap:{}:{}",
                    role.role_id, existing_role, existing_pattern
                ),
            });
        }
        if role.stage_ids.is_empty() && role.module_ids.is_empty() && role.exclusive_paths.is_empty()
        {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("worktree.roles.{}.scope", role.role_id),
            });
        }
        if protocol.model == "stage-isolated-worktree" && role.stage_ids.len() != 1 {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("worktree.roles.{}.stage_scope", role.role_id),
            });
        }
        if protocol.model == "module-isolated-worktree" && role.module_ids.is_empty() {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("worktree.roles.{}.module_scope", role.role_id),
            });
        }
        for path in &role.exclusive_paths {
            if path.contains('\\') {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: format!("worktree.roles.{}.exclusive_paths", role.role_id),
                });
            }
            let normalized_path = normalize_repo_relative_path(path);
            if shared_authority_paths
                .iter()
                .any(|shared| worktree_paths_overlap(&normalized_path, shared))
            {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: format!(
                        "worktree.roles.{}.exclusive_paths_shared_conflict",
                        role.role_id
                    ),
                });
            }
            if let Some((existing_role, existing_path)) = exclusive_path_owners.iter().find(|(existing_role, existing_path)| {
                existing_role != &role.role_id && worktree_paths_overlap(&normalized_path, existing_path)
            }) {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: format!(
                        "worktree.roles.{}.exclusive_paths_overlap:{}:{}",
                        role.role_id, existing_role, existing_path
                    ),
                });
            }
            exclusive_path_owners.push((role.role_id.clone(), normalized_path));
        }
        for stage_id in &role.stage_ids {
            if !stage_ids.contains(stage_id) {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: format!("worktree.roles.{}.stage_ids", role.role_id),
                });
            }
            *coverage.entry(stage_id.clone()).or_insert(0) += 1;
        }
        for module_id in &role.module_ids {
            if !module_ids.contains(module_id) {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: format!("worktree.roles.{}.module_ids", role.role_id),
                });
            }
            if let Some(existing_role) = module_owners.insert(module_id.clone(), role.role_id.clone())
            {
                if existing_role != role.role_id {
                    return Err(CoreError::ResolvedContractInconsistent {
                        field: format!(
                            "worktree.roles.{}.module_ids_duplicate:{}:{}",
                            role.role_id, existing_role, module_id
                        ),
                    });
                }
            }
        }
    }
    if protocol.model == "stage-isolated-worktree" {
        for stage_id in stage_ids {
            if coverage.get(&stage_id).copied().unwrap_or_default() != 1 {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: format!("worktree.roles.stage_coverage.{stage_id}"),
                });
            }
        }
    }

    let resolved_worktree =
        effective_worktree_protocol(&resolved_contract.worktree, protocol);
    if &resolved_worktree != protocol {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "worktree.contract".to_string(),
        });
    }

    Ok(())
}

fn validate_stage_document_alignment(
    stage_documents: &[StageDocument],
    resolved_contract: &ResolvedContract,
) -> Result<(), CoreError> {
    for stage_document in stage_documents {
        let Some((stage_id, stage_name)) =
            extract_stage_metadata(&stage_document.path, &stage_document.content)
        else {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("stage_document:{}", stage_document.path),
            });
        };
        if stage_id != stage_document.stage_id {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("stage_document.stage_id:{}", stage_document.path),
            });
        }
        let Some(resolved_stage) = resolved_contract
            .stages
            .iter()
            .find(|stage| stage.stage_id == stage_document.stage_id)
        else {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("stage_document.unresolved_stage:{}", stage_document.path),
            });
        };
        if stage_name != resolved_stage.stage_name {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("stage_document.stage_name:{}", stage_document.path),
            });
        }
    }
    Ok(())
}

fn build_task_progress(stages: &[ContractStage]) -> TaskProgressReport {
    let progress_stages = stages
        .iter()
        .enumerate()
        .map(|(index, stage)| TaskProgressStage {
            stage_id: stage.stage_id.clone(),
            stage_name: stage.stage_name.clone(),
            status: if index == 0 {
                TaskStageStatus::InProgress.as_str().to_string()
            } else {
                TaskStageStatus::Pending.as_str().to_string()
            },
            stage_progress_percent: if index == 0 { 0 } else { 0 },
        })
        .collect::<Vec<_>>();

    TaskProgressReport {
        schema_version: TASK_PROGRESS_SCHEMA_VERSION.to_string(),
        total_stages: stages.len(),
        completed_stages: 0,
        current_stage_id: stages.first().map(|stage| stage.stage_id.clone()),
        overall_progress_percent: 0,
        stages: progress_stages,
    }
}

fn build_agent_brief(
    project_name: &str,
    mode: AuthorMode,
    readiness: &ReadinessReport,
    task_progress: &TaskProgressReport,
    decision_summary: &DecisionSummary,
) -> AgentBrief {
    let current_stage_name = task_progress
        .current_stage_id
        .as_ref()
        .and_then(|stage_id| {
            task_progress
                .stages
                .iter()
                .find(|stage| &stage.stage_id == stage_id)
                .map(|stage| stage.stage_name.clone())
        });
    let next_actions = decision_summary
        .primary_recommended_action
        .iter()
        .cloned()
        .chain(decision_summary.recommended_actions.iter().cloned())
        .collect::<Vec<_>>();
    let next_actions = normalize_unique_strings(&next_actions);

    AgentBrief {
        schema_version: AGENT_BRIEF_SCHEMA_VERSION.to_string(),
        project_name: project_name.to_string(),
        mode: mode.as_str().to_string(),
        readiness_state: readiness.state.clone(),
        reason: decision_summary.reason.clone(),
        current_stage_id: task_progress.current_stage_id.clone(),
        current_stage_name,
        total_stages: task_progress.total_stages,
        completed_stages: task_progress.completed_stages,
        overall_progress_percent: task_progress.overall_progress_percent,
        blocking: decision_summary.blocking,
        review_required: decision_summary.review_required,
        primary_blocker_kind: decision_summary.primary_blocker_kind.clone(),
        primary_blocker_scope: decision_summary.primary_blocker_scope.clone(),
        primary_blocker_target_id: decision_summary.primary_blocker_target_id.clone(),
        primary_blocker_summary: decision_summary.primary_blocker_summary.clone(),
        primary_recommended_action: decision_summary.primary_recommended_action.clone(),
        top_blockers: decision_summary.top_blockers.clone(),
        top_review_items: decision_summary.top_review_items.clone(),
        scoped_worktree_roles: decision_summary.scoped_worktree_roles.clone(),
        gate_holds: decision_summary.gate_holds.clone(),
        next_actions,
    }
}

fn validate_task_progress_alignment(
    task_progress: &TaskProgressReport,
    resolved_contract: &ResolvedContract,
) -> Result<(), CoreError> {
    if task_progress.total_stages != resolved_contract.stages.len() {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "task_progress.total_stages".to_string(),
        });
    }
    if task_progress.stages.len() != resolved_contract.stages.len() {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "task_progress.stages".to_string(),
        });
    }

    let mut completed_stages = 0usize;
    let mut first_incomplete_stage = None;
    for (progress_stage, resolved_stage) in
        task_progress.stages.iter().zip(&resolved_contract.stages)
    {
        if progress_stage.stage_id != resolved_stage.stage_id {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("task_progress.stage_id:{}", resolved_stage.stage_id),
            });
        }
        if progress_stage.stage_name != resolved_stage.stage_name {
            return Err(CoreError::ResolvedContractInconsistent {
                field: format!("task_progress.stage_name:{}", resolved_stage.stage_id),
            });
        }

        match progress_stage.status.as_str() {
            "completed" => {
                if progress_stage.stage_progress_percent != 100 {
                    return Err(CoreError::ResolvedContractInconsistent {
                        field: format!("task_progress.stage_percent:{}", resolved_stage.stage_id),
                    });
                }
                completed_stages += 1;
            }
            "in-progress" => {
                if progress_stage.stage_progress_percent > 99 {
                    return Err(CoreError::ResolvedContractInconsistent {
                        field: format!("task_progress.stage_percent:{}", resolved_stage.stage_id),
                    });
                }
                if first_incomplete_stage.is_none() {
                    first_incomplete_stage = Some(progress_stage.stage_id.clone());
                }
            }
            "pending" => {
                if progress_stage.stage_progress_percent != 0 {
                    return Err(CoreError::ResolvedContractInconsistent {
                        field: format!("task_progress.stage_percent:{}", resolved_stage.stage_id),
                    });
                }
                if first_incomplete_stage.is_none() {
                    first_incomplete_stage = Some(progress_stage.stage_id.clone());
                }
            }
            _ => {
                return Err(CoreError::ResolvedContractInconsistent {
                    field: format!("task_progress.status:{}", resolved_stage.stage_id),
                });
            }
        }
    }

    if task_progress.completed_stages != completed_stages {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "task_progress.completed_stages".to_string(),
        });
    }
    let expected_overall = if task_progress.total_stages == 0 {
        0
    } else {
        ((completed_stages * 100) / task_progress.total_stages) as u8
    };
    if task_progress.overall_progress_percent != expected_overall {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "task_progress.overall_progress_percent".to_string(),
        });
    }
    let expected_current = if completed_stages == task_progress.total_stages {
        None
    } else {
        first_incomplete_stage
    };
    if task_progress.current_stage_id != expected_current {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "task_progress.current_stage_id".to_string(),
        });
    }

    Ok(())
}

fn validate_change_report_alignment(
    change_report: &ChangeReport,
    mode: AuthorMode,
) -> Result<(), CoreError> {
    if change_report.mode != mode.as_str() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "mode".to_string(),
        });
    }
    if change_report.operation_count != change_report.operations.len() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "operation_count".to_string(),
        });
    }
    let expected_conflict_count = change_report
        .operations
        .iter()
        .filter(|operation| operation.action == "retained-conflict")
        .count();
    if change_report.conflict_count != expected_conflict_count {
        return Err(CoreError::ChangeReportInconsistent {
            field: "conflict_count".to_string(),
        });
    }
    if change_report.operations.is_empty() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "operations".to_string(),
        });
    }
    if change_report.patch_operation_count != change_report.patch_operations.len() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_operation_count".to_string(),
        });
    }
    if matches!(mode, AuthorMode::UpdateBlueprint) && change_report.patch_operations.is_empty() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_operations".to_string(),
        });
    }
    if change_report.operations.iter().any(|operation| {
        operation.target_kind.trim().is_empty()
            || operation.target_id.trim().is_empty()
            || operation.action.trim().is_empty()
            || operation.details.trim().is_empty()
    }) {
        return Err(CoreError::ChangeReportInconsistent {
            field: "operations.fields".to_string(),
        });
    }
    if change_report.patch_operations.iter().any(|operation| {
        operation.scope.trim().is_empty()
            || operation.target_id.trim().is_empty()
            || operation.strategy.trim().is_empty()
            || operation.status.trim().is_empty()
            || operation.details.trim().is_empty()
            || operation.affected_paths.is_empty()
    }) {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_operations.fields".to_string(),
        });
    }
    Ok(())
}

fn validate_patch_base_alignment(
    patch_base: &PatchBase,
    patch_plan: &PatchPlan,
    mode: AuthorMode,
) -> Result<(), CoreError> {
    if patch_base.mode != mode.as_str() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_base.mode".to_string(),
        });
    }
    if patch_base.base_fingerprint.trim().is_empty() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_base.base_fingerprint".to_string(),
        });
    }

    match patch_base.artifact_status.as_str() {
        "emitted" | "legacy-derived-current-state" => {
            let Some(source) = patch_base_to_source(patch_base) else {
                return Err(CoreError::ChangeReportInconsistent {
                    field: "patch_base.sections".to_string(),
                });
            };
            if parsed_source_fingerprint(&source) != patch_base.base_fingerprint {
                return Err(CoreError::ChangeReportInconsistent {
                    field: "patch_base.base_fingerprint".to_string(),
                });
            }
        }
        "legacy-unavailable" => {
            if !matches!(mode, AuthorMode::UpdateBlueprint) {
                return Err(CoreError::ChangeReportInconsistent {
                    field: "patch_base.artifact_status".to_string(),
                });
            }
            if !patch_base.sections.is_empty() || !patch_base.stage_sections.is_empty() {
                return Err(CoreError::ChangeReportInconsistent {
                    field: "patch_base.sections".to_string(),
                });
            }
        }
        _ => {
            return Err(CoreError::ChangeReportInconsistent {
                field: "patch_base.artifact_status".to_string(),
            });
        }
    }

    if patch_base.artifact_status != "legacy-derived-current-state"
        && !patch_plan.base_fingerprint.trim().is_empty()
        && patch_base.base_fingerprint != patch_plan.base_fingerprint
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_base.base_fingerprint".to_string(),
        });
    }

    Ok(())
}

fn validate_patch_plan_alignment(
    patch_plan: &PatchPlan,
    change_report: &ChangeReport,
    mode: AuthorMode,
) -> Result<(), CoreError> {
    if patch_plan.mode != mode.as_str() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.mode".to_string(),
        });
    }
    if patch_plan.operation_count != patch_plan.operations.len() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.operation_count".to_string(),
        });
    }
    let expected_conflict_count = patch_plan
        .operations
        .iter()
        .filter(|operation| operation.status == "retained-conflict")
        .count();
    if patch_plan.conflict_count != expected_conflict_count {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.conflict_count".to_string(),
        });
    }
    if matches!(mode, AuthorMode::UpdateBlueprint) && patch_plan.operations.is_empty() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.operations".to_string(),
        });
    }
    if patch_plan.operations.iter().any(|operation| {
        operation.scope.trim().is_empty()
            || operation.target_id.trim().is_empty()
            || operation.strategy.trim().is_empty()
            || operation.status.trim().is_empty()
            || operation.details.trim().is_empty()
    }) {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.operations.fields".to_string(),
        });
    }
    if patch_plan.operation_count != change_report.patch_operation_count {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.operation_count_vs_change_report".to_string(),
        });
    }
    if patch_plan.operations.len() != change_report.patch_operations.len()
        || patch_plan
            .operations
            .iter()
            .zip(change_report.patch_operations.iter())
            .any(|(patch_plan_op, change_report_op)| {
                patch_plan_op.scope != change_report_op.scope
                    || patch_plan_op.target_id != change_report_op.target_id
                    || patch_plan_op.strategy != change_report_op.strategy
                    || patch_plan_op.status != change_report_op.status
                    || patch_plan_op.details != change_report_op.details
                    || patch_plan_op.affected_paths != change_report_op.affected_paths
                    || patch_plan_op.target_worktree_roles
                        != change_report_op.target_worktree_roles
                    || patch_plan_op.value_lines != change_report_op.value_lines
                    || patch_plan_op.stage_name != change_report_op.stage_name
                    || patch_plan_op.previous_value_lines != change_report_op.previous_value_lines
                    || patch_plan_op.previous_stage_name != change_report_op.previous_stage_name
                    || patch_plan_op.reverse_strategy != change_report_op.reverse_strategy
                    || patch_plan_op.strategy_metadata != change_report_op.strategy_metadata
            })
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.operations_vs_change_report".to_string(),
        });
    }
    let change_report_conflicts = change_report
        .patch_operations
        .iter()
        .filter(|operation| operation.status == "retained-conflict")
        .count();
    if patch_plan.conflict_count != change_report_conflicts {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_plan.conflict_count_vs_change_report".to_string(),
        });
    }
    if patch_plan.operation_count > 0 {
        if patch_plan.base_fingerprint.trim().is_empty() {
            return Err(CoreError::ChangeReportInconsistent {
                field: "patch_plan.base_fingerprint".to_string(),
            });
        }
        if patch_plan.result_fingerprint.trim().is_empty() {
            return Err(CoreError::ChangeReportInconsistent {
                field: "patch_plan.result_fingerprint".to_string(),
            });
        }
        if patch_plan
            .operations
            .iter()
            .any(|operation| operation.affected_paths.is_empty())
        {
            return Err(CoreError::ChangeReportInconsistent {
                field: "patch_plan.affected_paths".to_string(),
            });
        }
    }
    Ok(())
}

fn validate_patch_execution_report_alignment(
    patch_execution_report: &PatchExecutionReport,
    patch_plan: &PatchPlan,
    mode: AuthorMode,
    package: &BlueprintPackage,
) -> Result<(), CoreError> {
    if patch_execution_report.mode != mode.as_str() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.mode".to_string(),
        });
    }
    if patch_execution_report.operation_count != patch_plan.operation_count {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.operation_count".to_string(),
        });
    }
    if patch_execution_report.base_fingerprint != patch_plan.base_fingerprint {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.base_fingerprint".to_string(),
        });
    }
    if patch_execution_report.expected_result_fingerprint != patch_plan.result_fingerprint {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.expected_result_fingerprint".to_string(),
        });
    }

    if matches!(
        patch_execution_report.replay_status.as_str(),
        "replayed" | "not-applicable" | "legacy-derived"
    ) && patch_execution_report.replayed_result_fingerprint
        != patch_execution_report.expected_result_fingerprint
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.replayed_result_fingerprint".to_string(),
        });
    }
    if patch_execution_report.replay_status == "mismatch"
        && patch_execution_report.mismatch_count == 0
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.mismatch_count".to_string(),
        });
    }
    if matches!(
        patch_execution_report.replay_status.as_str(),
        "replayed" | "not-applicable" | "legacy-derived"
    ) && patch_execution_report.mismatch_count != 0
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.replay_status".to_string(),
        });
    }
    if patch_execution_report.applied_operation_count > patch_execution_report.operation_count {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.applied_operation_count".to_string(),
        });
    }
    if patch_execution_report.replay_status == "not-applicable"
        && patch_execution_report.applied_operation_count != 0
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.applied_operation_count".to_string(),
        });
    }
    if patch_execution_report.mismatch_count != patch_execution_report.mismatches.len() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.mismatch_count".to_string(),
        });
    }
    if matches!(
        patch_execution_report.reversibility_status.as_str(),
        "reversible" | "not-applicable" | "legacy-derived"
    ) && patch_execution_report.reverse_replayed_base_fingerprint
        != patch_execution_report.base_fingerprint
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.reverse_replayed_base_fingerprint".to_string(),
        });
    }
    if patch_execution_report.reversibility_status == "irreversible"
        && patch_execution_report.reverse_mismatch_count == 0
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.reverse_mismatch_count".to_string(),
        });
    }
    if matches!(
        patch_execution_report.reversibility_status.as_str(),
        "reversible" | "not-applicable" | "legacy-derived"
    ) && patch_execution_report.reverse_mismatch_count != 0
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.reversibility_status".to_string(),
        });
    }
    if patch_execution_report.reverse_mismatch_count
        != patch_execution_report.reverse_mismatches.len()
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.reverse_mismatch_count".to_string(),
        });
    }
    let expected_scope_mismatches = collect_patch_operation_scope_mismatches(
        &patch_plan.operations,
        &package.resolved_contract,
        &package.worktree_protocol,
        &package.stage_documents,
    )?;
    if patch_execution_report.scope_mismatch_count != patch_execution_report.scope_mismatches.len() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.scope_mismatch_count".to_string(),
        });
    }
    if patch_execution_report.scope_mismatches != expected_scope_mismatches {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.scope_mismatches".to_string(),
        });
    }
    let expected_scope_status = if !matches!(mode, AuthorMode::UpdateBlueprint)
        || patch_plan.operations.is_empty()
    {
        "not-applicable"
    } else if expected_scope_mismatches.is_empty() {
        "valid"
    } else {
        "mismatch"
    };
    if patch_execution_report.scope_validation_status == "legacy-derived" {
        if !expected_scope_mismatches.is_empty() {
            return Err(CoreError::ChangeReportInconsistent {
                field: "patch_execution_report.scope_validation_status".to_string(),
            });
        }
    } else if patch_execution_report.scope_validation_status != expected_scope_status {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.scope_validation_status".to_string(),
        });
    }
    if patch_execution_report.replay_status.trim().is_empty() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.replay_status".to_string(),
        });
    }
    if patch_execution_report
        .reversibility_status
        .trim()
        .is_empty()
    {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.reversibility_status".to_string(),
        });
    }
    if patch_execution_report.scope_validation_status.trim().is_empty() {
        return Err(CoreError::ChangeReportInconsistent {
            field: "patch_execution_report.scope_validation_status".to_string(),
        });
    }
    Ok(())
}

fn validate_semantic_ir_alignment(package: &BlueprintPackage) -> Result<(), CoreError> {
    let expected = derive_semantic_ir_from_package(package)?;
    if package.semantic_ir != expected {
        return Err(CoreError::ResolvedContractInconsistent {
            field: "semantic_ir".to_string(),
        });
    }
    Ok(())
}

fn build_semantic_ir(
    mode: AuthorMode,
    project_name: &str,
    source_provenance: &str,
    source_type: &str,
    derivation: &str,
    source_files: &[String],
    preserved_sections: &[String],
    inferred_sections: &[String],
    normalized_source: &ParsedBlueprintSource,
    projection_source: &ParsedBlueprintSource,
    stages: &[ContractStage],
    semantic_hints: &[String],
    semantic_risks: &[String],
    semantic_conflicts: &[SemanticConflict],
    unresolved_ambiguities: &[String],
    semantic_frames: &[SemanticFrame],
) -> SemanticIr {
    SemanticIr {
        schema_version: SEMANTIC_IR_SCHEMA_VERSION.to_string(),
        mode: mode.as_str().to_string(),
        project_name: project_name.to_string(),
        source_type: source_type.to_string(),
        source_provenance: source_provenance.to_string(),
        derivation: derivation.to_string(),
        source_files: source_files.to_vec(),
        source_fingerprint: parsed_source_fingerprint(normalized_source),
        projection_fingerprint: parsed_source_fingerprint(projection_source),
        normalized_sections: normalized_source.sections.clone(),
        normalized_stage_sections: normalized_source.stage_sections.clone(),
        normalized_section_origins: semantic_section_origins_from_source(
            &normalized_source.sections,
            preserved_sections,
            inferred_sections,
        ),
        normalized_stage_section_origins: semantic_stage_section_origins_from_source(
            &normalized_source.stage_sections,
        ),
        semantic_frames: dedupe_semantic_frames(semantic_frames.to_vec()),
        semantic_clusters: semantic_clusters_from_frames(semantic_frames),
        sections: semantic_sections_from_map(&projection_source.sections),
        stages: semantic_stages_from_maps(&projection_source.stage_sections, stages),
        preserved_sections: preserved_sections.to_vec(),
        inferred_sections: inferred_sections.to_vec(),
        semantic_hints: semantic_hints.to_vec(),
        semantic_risks: semantic_risks.to_vec(),
        semantic_conflicts: semantic_conflicts.to_vec(),
        unresolved_ambiguities: unresolved_ambiguities.to_vec(),
    }
}

fn derive_semantic_ir_from_package(package: &BlueprintPackage) -> Result<SemanticIr, CoreError> {
    let projection_source = parse_workspace_source(&workspace_bundle_text_from_docs(
        &format!(
            "{}/00-authority-root.md",
            package.resolved_contract.paths.authority_root
        ),
        &package.authority_doc,
        &format!(
            "{}/00-workflow-overview.md",
            package.resolved_contract.paths.workflow_root
        ),
        &package.workflow_doc,
        &package.stage_documents,
    ));
    let normalized_source = if package.semantic_ir.normalized_sections.is_empty()
        && package.semantic_ir.normalized_stage_sections.is_empty()
    {
        reconstruct_result_source(
            package.mode,
            &package.patch_base,
            &package.patch_plan,
            &projection_source,
        )?
    } else {
        ParsedBlueprintSource {
            sections: package.semantic_ir.normalized_sections.clone(),
            stage_sections: package.semantic_ir.normalized_stage_sections.clone(),
            semantic_frames: package.semantic_ir.semantic_frames.clone(),
        }
    };
    let semantic_frames = if package.semantic_ir.semantic_frames.is_empty() {
        fallback_semantic_frames_for_source(
            &normalized_source.sections,
            &normalized_source.stage_sections,
            &package.normalization_report.preserved_sections,
            &package.normalization_report.inferred_sections,
        )
    } else {
        package.semantic_ir.semantic_frames.clone()
    };
    Ok(build_semantic_ir(
        package.mode,
        &package.project_name,
        &package.source_provenance,
        &package.normalization_report.source_type,
        &package.semantic_ir.derivation,
        &package.normalization_report.source_files,
        &package.normalization_report.preserved_sections,
        &package.normalization_report.inferred_sections,
        &normalized_source,
        &projection_source,
        &package.resolved_contract.stages,
        &package.normalization_report.semantic_hints,
        &package.normalization_report.semantic_risks,
        &package.normalization_report.semantic_conflicts,
        &package.normalization_report.unresolved_ambiguities,
        &semantic_frames,
    ))
}

fn semantic_sections_from_map(sections: &BTreeMap<String, Vec<String>>) -> Vec<SemanticSection> {
    sections
        .iter()
        .map(|(key, values)| SemanticSection {
            key: key.clone(),
            values: values.clone(),
        })
        .collect()
}

fn semantic_section_origins_from_source(
    sections: &BTreeMap<String, Vec<String>>,
    preserved_sections: &[String],
    inferred_sections: &[String],
) -> BTreeMap<String, String> {
    let preserved = preserved_sections.iter().cloned().collect::<BTreeSet<_>>();
    let inferred = inferred_sections.iter().cloned().collect::<BTreeSet<_>>();
    sections
        .keys()
        .map(|key| {
            let origin = if inferred.contains(key) {
                "inferred"
            } else if preserved.contains(key) {
                "preserved"
            } else {
                "generated"
            };
            (key.clone(), origin.to_string())
        })
        .collect()
}

fn semantic_stage_section_origins_from_source(
    stage_sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
) -> BTreeMap<String, BTreeMap<String, String>> {
    stage_sections
        .iter()
        .map(|(stage_id, sections)| {
            (
                stage_id.clone(),
                sections
                    .keys()
                    .map(|key| (key.clone(), "stage-normalized".to_string()))
                    .collect(),
            )
        })
        .collect()
}

fn fallback_semantic_frames_for_source(
    sections: &BTreeMap<String, Vec<String>>,
    stage_sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
    preserved_sections: &[String],
    inferred_sections: &[String],
) -> Vec<SemanticFrame> {
    let section_origins =
        semantic_section_origins_from_source(sections, preserved_sections, inferred_sections);
    let stage_origins = semantic_stage_section_origins_from_source(stage_sections);
    let mut frames = Vec::new();
    let mut covered = BTreeSet::new();

    for (section_key, values) in sections {
        if values.is_empty() {
            continue;
        }
        if covered.insert(("package".to_string(), section_key.clone())) {
            let origin = section_origins
                .get(section_key)
                .cloned()
                .unwrap_or_else(|| "generated".to_string());
            frames.push(semantic_frame(
                "package",
                section_key,
                section_key,
                &format!("normalized.{section_key}"),
                "normalized-fallback",
                if origin == "inferred" {
                    "fallback"
                } else {
                    "preserved"
                },
                values.clone(),
            ));
        }
    }

    for (stage_id, stage_map) in stage_sections {
        for (section_key, values) in stage_map {
            if values.is_empty() {
                continue;
            }
            let scope = format!("stage:{stage_id}");
            if covered.insert((scope.clone(), section_key.clone())) {
                let origin = stage_origins
                    .get(stage_id)
                    .and_then(|origins| origins.get(section_key))
                    .cloned()
                    .unwrap_or_else(|| "stage-normalized".to_string());
                frames.push(semantic_frame(
                    &scope,
                    section_key,
                    section_key,
                    &format!("normalized.{stage_id}.{section_key}"),
                    "normalized-fallback",
                    if origin == "stage-normalized" {
                        "preserved"
                    } else {
                        "fallback"
                    },
                    values.clone(),
                ));
            }
        }
    }

    dedupe_semantic_frames(frames)
}

fn complete_semantic_frames_with_fallback(
    existing_frames: &[SemanticFrame],
    sections: &BTreeMap<String, Vec<String>>,
    stage_sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
    preserved_sections: &[String],
    inferred_sections: &[String],
) -> Vec<SemanticFrame> {
    let mut completed = dedupe_semantic_frames(existing_frames.to_vec());
    let mut covered = completed
        .iter()
        .filter(|frame| !frame.values.is_empty())
        .map(|frame| (frame.scope.clone(), frame.canonical_section.clone()))
        .collect::<BTreeSet<_>>();

    for frame in fallback_semantic_frames_for_source(
        sections,
        stage_sections,
        preserved_sections,
        inferred_sections,
    ) {
        if covered.insert((frame.scope.clone(), frame.canonical_section.clone())) {
            completed.push(frame);
        }
    }

    dedupe_semantic_frames(completed)
}

fn semantic_stages_from_maps(
    stage_sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
    stages: &[ContractStage],
) -> Vec<SemanticStage> {
    stages
        .iter()
        .map(|stage| SemanticStage {
            stage_id: stage.stage_id.clone(),
            stage_name: stage.stage_name.clone(),
            sections: stage_sections
                .get(&stage.stage_id)
                .map(semantic_sections_from_map)
                .unwrap_or_default(),
        })
        .collect()
}

fn workspace_bundle_text_from_docs(
    authority_path: &str,
    authority_doc: &str,
    workflow_path: &str,
    workflow_doc: &str,
    stage_documents: &[StageDocument],
) -> String {
    let mut chunks = vec![
        format!(
            "# Source File\n\n{}\n\n{}",
            authority_path,
            authority_doc.trim()
        ),
        format!(
            "# Source File\n\n{}\n\n{}",
            workflow_path,
            workflow_doc.trim()
        ),
    ];

    for stage_document in stage_documents {
        chunks.push(format!(
            "# Source File\n\n{}\n\n{}",
            stage_document.path,
            stage_document.content.trim()
        ));
    }

    chunks.join("\n\n")
}

fn validate_schema_versions(
    package: &BlueprintPackage,
    parsed_contract: &ProjectContractToml,
) -> Result<(), CoreError> {
    let default_contract = default_project_contract_template()?;

    validate_schema_version(
        "module-catalog",
        &package.module_catalog.schema_version,
        MODULE_CATALOG_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "blueprint-manifest",
        &package.manifest.schema_version,
        BLUEPRINT_MANIFEST_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "worktree-protocol",
        &package.worktree_protocol.schema_version,
        WORKTREE_PROTOCOL_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "semantic-ir",
        &package.semantic_ir.schema_version,
        SEMANTIC_IR_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "normalization-report",
        &package.normalization_report.schema_version,
        NORMALIZATION_REPORT_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "change-report",
        &package.change_report.schema_version,
        CHANGE_REPORT_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "patch-base",
        &package.patch_base.schema_version,
        PATCH_BASE_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "patch-plan",
        &package.patch_plan.schema_version,
        PATCH_PLAN_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "patch-execution-report",
        &package.patch_execution_report.schema_version,
        PATCH_EXECUTION_REPORT_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "readiness",
        &package.readiness.schema_version,
        READINESS_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "decision-summary",
        &package.decision_summary.schema_version,
        DECISION_SUMMARY_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "agent-brief",
        &package.agent_brief.schema_version,
        AGENT_BRIEF_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "task-progress",
        &package.task_progress.schema_version,
        TASK_PROGRESS_SCHEMA_VERSION,
    )?;
    validate_schema_version(
        "resolved-contract",
        &package.resolved_contract.schema_version,
        RESOLVED_CONTRACT_SCHEMA_VERSION,
    )?;
    if !worktree_protocol_is_empty(&package.resolved_contract.worktree) {
        validate_schema_version(
            "resolved-contract.worktree",
            &package.resolved_contract.worktree.schema_version,
            WORKTREE_PROTOCOL_SCHEMA_VERSION,
        )?;
    }
    validate_schema_version(
        "project-contract",
        &parsed_contract.schema_version,
        &default_contract.schema_version,
    )?;
    if !worktree_protocol_is_empty(&parsed_contract.worktree) {
        validate_schema_version(
            "project-contract.worktree",
            &parsed_contract.worktree.schema_version,
            WORKTREE_PROTOCOL_SCHEMA_VERSION,
        )?;
    }

    Ok(())
}

fn upgrade_package_schema_versions(
    workspace_root: &Path,
    package: &mut BlueprintPackage,
    parsed_contract: &mut ProjectContractToml,
) -> Result<MigrationReport, CoreError> {
    let blueprint_policy = default_blueprint_policy()?;
    let default_contract = default_project_contract_template()?;
    let mut artifacts = Vec::new();

    artifacts.push(migration_artifact(
        "module-catalog",
        format!(
            "{}/00-module-catalog.json",
            package.resolved_contract.paths.modules_root
        ),
        &package.module_catalog.schema_version,
        MODULE_CATALOG_SCHEMA_VERSION,
    ));
    package.module_catalog.schema_version = MODULE_CATALOG_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "blueprint-manifest",
        blueprint_policy.package.manifest_file.clone(),
        &package.manifest.schema_version,
        BLUEPRINT_MANIFEST_SCHEMA_VERSION,
    ));
    package.manifest.schema_version = BLUEPRINT_MANIFEST_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "worktree-protocol",
        format!(
            "{}/worktree-protocol.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.worktree_protocol.schema_version,
        WORKTREE_PROTOCOL_SCHEMA_VERSION,
    ));
    package.worktree_protocol.schema_version = WORKTREE_PROTOCOL_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "semantic-ir",
        format!(
            "{}/semantic-ir.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.semantic_ir.schema_version,
        SEMANTIC_IR_SCHEMA_VERSION,
    ));
    package.semantic_ir.schema_version = SEMANTIC_IR_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "normalization-report",
        format!(
            "{}/normalization-report.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.normalization_report.schema_version,
        NORMALIZATION_REPORT_SCHEMA_VERSION,
    ));
    package.normalization_report.schema_version = NORMALIZATION_REPORT_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "change-report",
        format!(
            "{}/change-report.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.change_report.schema_version,
        CHANGE_REPORT_SCHEMA_VERSION,
    ));
    package.change_report.schema_version = CHANGE_REPORT_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "patch-base",
        format!(
            "{}/patch-base.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.patch_base.schema_version,
        PATCH_BASE_SCHEMA_VERSION,
    ));
    package.patch_base.schema_version = PATCH_BASE_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "patch-plan",
        format!(
            "{}/patch-plan.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.patch_plan.schema_version,
        PATCH_PLAN_SCHEMA_VERSION,
    ));
    package.patch_plan.schema_version = PATCH_PLAN_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "patch-execution-report",
        format!(
            "{}/patch-execution-report.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.patch_execution_report.schema_version,
        PATCH_EXECUTION_REPORT_SCHEMA_VERSION,
    ));
    package.patch_execution_report.schema_version =
        PATCH_EXECUTION_REPORT_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "readiness",
        format!(
            "{}/readiness.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.readiness.schema_version,
        READINESS_SCHEMA_VERSION,
    ));
    package.readiness.schema_version = READINESS_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "decision-summary",
        format!(
            "{}/decision-summary.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.decision_summary.schema_version,
        DECISION_SUMMARY_SCHEMA_VERSION,
    ));
    package.decision_summary.schema_version = DECISION_SUMMARY_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "agent-brief",
        format!(
            "{}/agent-brief.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.agent_brief.schema_version,
        AGENT_BRIEF_SCHEMA_VERSION,
    ));
    package.agent_brief.schema_version = AGENT_BRIEF_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "task-progress",
        format!(
            "{}/task-progress.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.task_progress.schema_version,
        TASK_PROGRESS_SCHEMA_VERSION,
    ));
    package.task_progress.schema_version = TASK_PROGRESS_SCHEMA_VERSION.to_string();

    artifacts.push(migration_artifact(
        "resolved-contract",
        format!(
            "{}/resolved-contract.json",
            package.resolved_contract.paths.contract_root
        ),
        &package.resolved_contract.schema_version,
        RESOLVED_CONTRACT_SCHEMA_VERSION,
    ));
    package.resolved_contract.schema_version = RESOLVED_CONTRACT_SCHEMA_VERSION.to_string();
    package.resolved_contract.worktree = effective_worktree_protocol(
        &package.resolved_contract.worktree,
        &package.worktree_protocol,
    );

    artifacts.push(migration_artifact(
        "project-contract",
        format!(
            "{}/project-contract.toml",
            package.resolved_contract.paths.contract_root
        ),
        &parsed_contract.schema_version,
        &default_contract.schema_version,
    ));
    parsed_contract.schema_version = default_contract.schema_version.clone();
    parsed_contract.worktree = effective_worktree_protocol(&parsed_contract.worktree, &package.worktree_protocol);

    let migrated_artifacts = artifacts
        .iter()
        .filter(|artifact| artifact.action == "migrated")
        .count();
    let unchanged_artifacts = artifacts.len().saturating_sub(migrated_artifacts);

    Ok(MigrationReport {
        schema_version: MIGRATION_REPORT_SCHEMA_VERSION.to_string(),
        mode: "migrate-workspace".to_string(),
        workspace: normalize_repo_relative_path(&workspace_root.display().to_string()),
        migrated_artifacts,
        unchanged_artifacts,
        artifacts,
    })
}

fn validate_schema_version(artifact: &str, found: &str, expected: &str) -> Result<(), CoreError> {
    if found == expected {
        Ok(())
    } else {
        Err(CoreError::SchemaVersionMismatch {
            artifact: artifact.to_string(),
            expected: expected.to_string(),
            found: found.to_string(),
        })
    }
}

fn migration_artifact(
    artifact: &str,
    path: String,
    previous_schema_version: &str,
    current_schema_version: &str,
) -> MigrationArtifact {
    MigrationArtifact {
        artifact: artifact.to_string(),
        path,
        previous_schema_version: previous_schema_version.to_string(),
        current_schema_version: current_schema_version.to_string(),
        action: if previous_schema_version == current_schema_version {
            "already-current".to_string()
        } else {
            "migrated".to_string()
        },
    }
}

fn default_project_contract_template() -> Result<ProjectContractToml, CoreError> {
    Ok(toml::from_str(DEFAULT_PROJECT_CONTRACT_TEMPLATE)?)
}

fn default_blueprint_policy() -> Result<BlueprintPolicyToml, CoreError> {
    Ok(toml::from_str(DEFAULT_BLUEPRINT_POLICY)?)
}

fn default_normalization_policy() -> Result<NormalizationPolicyToml, CoreError> {
    Ok(toml::from_str(DEFAULT_NORMALIZATION_POLICY)?)
}

fn default_module_language_policy() -> Result<ModuleLanguagePolicyToml, CoreError> {
    Ok(toml::from_str(DEFAULT_MODULE_LANGUAGE_POLICY)?)
}

fn apply_module_language_policy(
    module: &mut ModuleSpec,
    policy: &ModuleLanguagePolicyToml,
) -> Result<(), CoreError> {
    let rule = policy
        .rules
        .iter()
        .find(|rule| rule.layer == module.layer)
        .ok_or_else(|| CoreError::InvalidModuleLanguage {
            module_id: module.module_id.clone(),
        })?;

    module.recommended_language = rule.recommended_language;
    module.allowed_languages = rule.allowed_languages.clone();
    module.forbidden_languages = rule.forbidden_languages.clone();
    Ok(())
}

fn validate_source_inputs(
    mode: AuthorMode,
    input: &BlueprintAuthorInput,
    policy: &NormalizationPolicyToml,
) -> Result<(), CoreError> {
    if !matches!(
        mode,
        AuthorMode::ImportBlueprint | AuthorMode::UpdateBlueprint
    ) {
        return Ok(());
    }

    for candidate in source_candidates(input) {
        let normalized = normalize_repo_relative_path(&candidate);
        if normalized.starts_with("workspace:")
            || normalized.starts_with("<")
            || normalized.contains("<workspace-blueprint-bundle>")
            || normalized.contains("<source-summary>")
            || normalized.contains("<inline-source>")
        {
            continue;
        }

        let Some(source_format) = detect_source_format(&normalized) else {
            return Err(CoreError::UnsupportedImportSourceFormat {
                input_source: normalized,
            });
        };
        if !policy
            .import
            .supported_sources
            .iter()
            .any(|item| item == source_format)
        {
            return Err(CoreError::UnsupportedImportSourceFormat {
                input_source: normalized,
            });
        }
    }

    Ok(())
}

fn source_candidates(input: &BlueprintAuthorInput) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(source_path) = &input.source_path {
        for raw_part in source_path.split(" + ") {
            let trimmed = raw_part.trim();
            if trimmed.is_empty() {
                continue;
            }
            let normalized = if let Some(value) = trimmed.strip_prefix("workspace:") {
                format!("workspace:{}", value.trim())
            } else if let Some(value) = trimmed.strip_prefix("source:") {
                value.trim().to_string()
            } else {
                trimmed.to_string()
            };
            if !normalized.is_empty() {
                candidates.push(normalized);
            }
        }
    }
    candidates
}

fn detect_source_format(path: &str) -> Option<&'static str> {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())?;

    match extension.as_str() {
        "md" | "markdown" => Some("markdown"),
        "json" => Some("json"),
        "toml" => Some("toml"),
        _ => None,
    }
}

fn ensure_section(
    sections: &mut BTreeMap<String, Vec<String>>,
    inferred_sections: &mut Vec<String>,
    key: &str,
    default_value: Vec<String>,
) {
    if !sections.contains_key(key) {
        sections.insert(key.to_string(), default_value);
        inferred_sections.push(key.to_string());
    }
}

fn merge_update_sections(
    workspace_sections: Option<ParsedBlueprintSource>,
    update_sections: Option<ParsedBlueprintSource>,
    input: &BlueprintAuthorInput,
    policy: &NormalizationPolicyToml,
) -> Result<
    (
        BTreeMap<String, Vec<String>>,
        BTreeMap<String, BTreeMap<String, Vec<String>>>,
        Option<ParsedBlueprintSource>,
        Vec<String>,
        Vec<String>,
        Vec<ChangeOperation>,
        Vec<PatchOperation>,
        String,
    ),
    CoreError,
> {
    match (workspace_sections, update_sections) {
        (Some(existing), Some(update)) => {
            let base = existing.clone();
            let mut merged = existing.sections.clone();
            let mut merged_stage_sections = existing.stage_sections.clone();
            let mut preserved = existing.sections.keys().cloned().collect::<Vec<_>>();
            if !existing.stage_sections.is_empty() {
                preserved.push("stage_details".to_string());
            }
            let mut unresolved = Vec::new();
            let mut operations = vec![change_operation(
                "package",
                input.project_name.trim(),
                "merged-update",
                "merged update input into the existing workspace blueprint package",
            )];
            let mut patch_operations = vec![patch_operation(
                "package",
                input.project_name.trim(),
                "apply-update-delta",
                "applied",
                "applied an explicit update delta to the existing workspace blueprint package",
            )];

            for (key, update_value) in update.sections {
                match merged.get(&key) {
                    None => {
                        let patch_values = update_value.clone();
                        merged.insert(key.clone(), update_value);
                        operations.push(change_operation(
                            "section",
                            &key,
                            "added",
                            "update introduced a new blueprint section",
                        ));
                        patch_operations.push(patch_operation_with_values(
                            "section",
                            &key,
                            "set-section",
                            "applied",
                            "update introduced a new blueprint section",
                            patch_values,
                            None,
                        ));
                        preserved.push(key);
                    }
                    Some(existing_value) if *existing_value == update_value => {}
                    Some(existing_value) => {
                        let combined = if key == "stage_order" {
                            operations.push(change_operation(
                                "section",
                                &key,
                                "merged",
                                "merged update stage order with the existing workspace stage graph",
                            ));
                            patch_operations.push(patch_operation_with_values(
                                "section",
                                &key,
                                "merge-stage-order",
                                "applied",
                                "merged update stage order with the existing workspace stage graph",
                                update_value.clone(),
                                None,
                            ));
                            merge_stage_order(existing_value, &update_value)?
                        } else if is_union_section(&key) {
                            operations.push(change_operation(
                                "section",
                                &key,
                                "merged",
                                "union-merged list-like section values from workspace and update",
                            ));
                            patch_operations.push(patch_operation_with_values(
                                "section",
                                &key,
                                "union-merge",
                                "applied",
                                "union-merged list-like section values from workspace and update",
                                update_value.clone(),
                                None,
                            ));
                            merge_unique_lines(existing_value, &update_value)
                        } else if is_hard_conflict_section(&key, policy) {
                            return Err(CoreError::AuthorityConflict { section: key });
                        } else {
                            unresolved.push(format!("update modified `{key}` and both variants were retained for review"));
                            operations.push(change_operation(
                                "section",
                                &key,
                                "retained-conflict",
                                "retained both workspace and update variants for manual review",
                            ));
                            patch_operations.push(patch_operation_with_values(
                                "section",
                                &key,
                                "retain-conflict",
                                "applied",
                                "retained both workspace and update variants for manual review",
                                update_value.clone(),
                                None,
                            ));
                            merge_unique_lines(existing_value, &update_value)
                        };
                        merged.insert(key, combined);
                    }
                }
            }

            merge_stage_section_maps(
                &mut merged_stage_sections,
                update.stage_sections,
                policy,
                &mut unresolved,
                &mut operations,
                &mut patch_operations,
            )?;
            if !merged_stage_sections.is_empty() {
                preserved.push("stage_details".to_string());
            }

            let merged_source = ParsedBlueprintSource {
                sections: merged.clone(),
                stage_sections: merged_stage_sections.clone(),
                semantic_frames: Vec::new(),
            };
            let patch_plan = build_patch_plan(
                AuthorMode::UpdateBlueprint,
                &patch_operations,
                &base,
                &merged_source,
            );
            let replayed = apply_patch_plan_to_source(&base, &patch_plan)?;
            if replayed.sections != merged || replayed.stage_sections != merged_stage_sections {
                return Err(CoreError::ChangeReportInconsistent {
                    field: "patch_plan.apply".to_string(),
                });
            }

            preserved.sort();
            preserved.dedup();
            Ok((
                merged,
                merged_stage_sections,
                Some(base),
                preserved,
                unresolved,
                operations,
                patch_operations,
                "workspace-update".to_string(),
            ))
        }
        (Some(existing), None) => {
            let base = existing.clone();
            let mut preserved = existing.sections.keys().cloned().collect::<Vec<_>>();
            if !existing.stage_sections.is_empty() {
                preserved.push("stage_details".to_string());
            }
            preserved.sort();
            Ok((
                existing.sections,
                existing.stage_sections,
                Some(base),
                preserved,
                Vec::new(),
                vec![change_operation(
                    "package",
                    input.project_name.trim(),
                    "reused-workspace",
                    "reused the existing workspace blueprint package without update deltas",
                )],
                vec![patch_operation(
                    "package",
                    input.project_name.trim(),
                    "reuse-workspace",
                    "applied",
                    "reused the existing workspace blueprint package without update deltas",
                )],
                "workspace-update".to_string(),
            ))
        }
        (None, Some(update)) => {
            let base = update.clone();
            let mut preserved = update.sections.keys().cloned().collect::<Vec<_>>();
            if !update.stage_sections.is_empty() {
                preserved.push("stage_details".to_string());
            }
            preserved.sort();
            Ok((
                update.sections,
                update.stage_sections,
                Some(base),
                preserved,
                vec!["update mode did not receive a workspace blueprint; update input was treated as the new source".to_string()],
                vec![change_operation(
                    "package",
                    input.project_name.trim(),
                    "normalized-without-workspace",
                    "treated update input as the new source because no workspace blueprint bundle was available",
                )],
                vec![patch_operation(
                    "package",
                    input.project_name.trim(),
                    "normalize-without-workspace",
                    "applied",
                    "treated update input as the new source because no workspace blueprint bundle was available",
                )],
                "normalized-import".to_string(),
            ))
        }
        (None, None) => Err(CoreError::MissingWorkspaceBlueprint {
            workspace: input
                .source_path
                .clone()
                .unwrap_or_else(|| "<none>".to_string()),
        }),
    }
}

fn merge_stage_section_maps(
    merged_stage_sections: &mut BTreeMap<String, BTreeMap<String, Vec<String>>>,
    update_stage_sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    policy: &NormalizationPolicyToml,
    unresolved: &mut Vec<String>,
    operations: &mut Vec<ChangeOperation>,
    patch_operations: &mut Vec<PatchOperation>,
) -> Result<(), CoreError> {
    for (stage_id, update_sections) in update_stage_sections {
        let stage_exists = merged_stage_sections.contains_key(&stage_id);
        let stage_name_hint = update_sections
            .get("stage_name")
            .and_then(|values| values.first())
            .cloned();
        let existing_sections = merged_stage_sections.entry(stage_id.clone()).or_default();
        if !stage_exists {
            operations.push(change_operation(
                "stage",
                &stage_id,
                "added",
                "update introduced stage-specific detail for a new stage",
            ));
            patch_operations.push(patch_operation_with_values(
                "stage",
                &stage_id,
                "add-stage",
                "applied",
                "update introduced stage-specific detail for a new stage",
                Vec::new(),
                stage_name_hint.clone(),
            ));
        }
        for (key, update_value) in update_sections {
            match existing_sections.get(&key) {
                None => {
                    operations.push(change_operation(
                        "stage-section",
                        format!("{stage_id}:{key}"),
                        "added",
                        "update introduced a new stage-specific section",
                    ));
                    patch_operations.push(patch_operation_with_values(
                        "stage-section",
                        format!("{stage_id}:{key}"),
                        "set-stage-section",
                        "applied",
                        "update introduced a new stage-specific section",
                        update_value.clone(),
                        stage_name_hint.clone(),
                    ));
                    existing_sections.insert(key, update_value);
                }
                Some(existing_value) if *existing_value == update_value => {}
                Some(existing_value) => {
                    let combined = if is_union_section(&key) {
                        operations.push(change_operation(
                            "stage-section",
                            format!("{stage_id}:{key}"),
                            "merged",
                            "union-merged stage-specific list values from workspace and update",
                        ));
                        patch_operations.push(patch_operation_with_values(
                            "stage-section",
                            format!("{stage_id}:{key}"),
                            "union-merge-stage-section",
                            "applied",
                            "union-merged stage-specific list values from workspace and update",
                            update_value.clone(),
                            stage_name_hint.clone(),
                        ));
                        merge_unique_lines(existing_value, &update_value)
                    } else if key == "stage_name" {
                        if existing_value == &update_value {
                            existing_value.clone()
                        } else {
                            unresolved.push(format!(
                                "update modified stage metadata `{}` for `{}` and both variants were retained for review",
                                key, stage_id
                            ));
                            operations.push(change_operation(
                                "stage-section",
                                format!("{stage_id}:{key}"),
                                "retained-conflict",
                                "retained conflicting stage metadata variants for review",
                            ));
                            patch_operations.push(patch_operation_with_values(
                                "stage-section",
                                format!("{stage_id}:{key}"),
                                "retain-conflict",
                                "applied",
                                "retained conflicting stage metadata variants for review",
                                update_value.clone(),
                                stage_name_hint.clone(),
                            ));
                            merge_unique_lines(existing_value, &update_value)
                        }
                    } else if is_hard_conflict_section(&key, policy) {
                        return Err(CoreError::AuthorityConflict { section: key });
                    } else {
                        unresolved.push(format!(
                            "update modified `{}` for stage `{}` and both variants were retained for review",
                            key, stage_id
                        ));
                        operations.push(change_operation(
                            "stage-section",
                            format!("{stage_id}:{key}"),
                            "retained-conflict",
                            "retained both workspace and update stage-specific variants for review",
                        ));
                        patch_operations.push(patch_operation_with_values(
                            "stage-section",
                            format!("{stage_id}:{key}"),
                            "retain-conflict",
                            "applied",
                            "retained both workspace and update stage-specific variants for review",
                            update_value.clone(),
                            stage_name_hint.clone(),
                        ));
                        merge_unique_lines(existing_value, &update_value)
                    };
                    existing_sections.insert(key, combined);
                }
            }
        }
    }

    Ok(())
}

fn change_operation(
    target_kind: &str,
    target_id: impl Into<String>,
    action: &str,
    details: impl Into<String>,
) -> ChangeOperation {
    ChangeOperation {
        target_kind: target_kind.to_string(),
        target_id: target_id.into(),
        action: action.to_string(),
        details: details.into(),
    }
}

fn patch_operation(
    scope: &str,
    target_id: impl Into<String>,
    strategy: &str,
    status: &str,
    details: impl Into<String>,
) -> PatchOperation {
    PatchOperation {
        scope: scope.to_string(),
        target_id: target_id.into(),
        strategy: strategy.to_string(),
        status: status.to_string(),
        details: details.into(),
        affected_paths: Vec::new(),
        target_worktree_roles: Vec::new(),
        value_lines: Vec::new(),
        stage_name: None,
        previous_value_lines: Vec::new(),
        previous_stage_name: None,
        reverse_strategy: None,
        strategy_metadata: BTreeMap::new(),
    }
}

fn patch_operation_with_values(
    scope: &str,
    target_id: impl Into<String>,
    strategy: &str,
    status: &str,
    details: impl Into<String>,
    value_lines: Vec<String>,
    stage_name: Option<String>,
) -> PatchOperation {
    PatchOperation {
        scope: scope.to_string(),
        target_id: target_id.into(),
        strategy: strategy.to_string(),
        status: status.to_string(),
        details: details.into(),
        affected_paths: Vec::new(),
        target_worktree_roles: Vec::new(),
        value_lines,
        stage_name,
        previous_value_lines: Vec::new(),
        previous_stage_name: None,
        reverse_strategy: None,
        strategy_metadata: BTreeMap::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchReplayDirection {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchStrategyKind {
    RecompileMachineOutputs,
    NormalizeExternalBlueprint,
    GeneratePackage,
    InferStageOrder,
    FillDefault,
    PreserveStageDetail,
    ApplyUpdateDelta,
    ReuseWorkspace,
    NormalizeWithoutWorkspace,
    MergeStageOrder,
    UnionMerge,
    SetSection,
    AddStage,
    SetStageSection,
    UnionMergeStageSection,
    RetainConflict,
    RemoveStage,
}

impl PatchStrategyKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "recompile-machine-outputs" => Some(Self::RecompileMachineOutputs),
            "normalize-external-blueprint" => Some(Self::NormalizeExternalBlueprint),
            "generate-package" => Some(Self::GeneratePackage),
            "infer-stage-order" => Some(Self::InferStageOrder),
            "fill-default" => Some(Self::FillDefault),
            "preserve-stage-detail" => Some(Self::PreserveStageDetail),
            "apply-update-delta" => Some(Self::ApplyUpdateDelta),
            "reuse-workspace" => Some(Self::ReuseWorkspace),
            "normalize-without-workspace" => Some(Self::NormalizeWithoutWorkspace),
            "merge-stage-order" => Some(Self::MergeStageOrder),
            "union-merge" => Some(Self::UnionMerge),
            "set-section" => Some(Self::SetSection),
            "add-stage" => Some(Self::AddStage),
            "set-stage-section" => Some(Self::SetStageSection),
            "union-merge-stage-section" => Some(Self::UnionMergeStageSection),
            "retain-conflict" => Some(Self::RetainConflict),
            "remove-stage" => Some(Self::RemoveStage),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::RecompileMachineOutputs => "recompile-machine-outputs",
            Self::NormalizeExternalBlueprint => "normalize-external-blueprint",
            Self::GeneratePackage => "generate-package",
            Self::InferStageOrder => "infer-stage-order",
            Self::FillDefault => "fill-default",
            Self::PreserveStageDetail => "preserve-stage-detail",
            Self::ApplyUpdateDelta => "apply-update-delta",
            Self::ReuseWorkspace => "reuse-workspace",
            Self::NormalizeWithoutWorkspace => "normalize-without-workspace",
            Self::MergeStageOrder => "merge-stage-order",
            Self::UnionMerge => "union-merge",
            Self::SetSection => "set-section",
            Self::AddStage => "add-stage",
            Self::SetStageSection => "set-stage-section",
            Self::UnionMergeStageSection => "union-merge-stage-section",
            Self::RetainConflict => "retain-conflict",
            Self::RemoveStage => "remove-stage",
        }
    }

    fn risk_level(self) -> &'static str {
        match self {
            Self::RetainConflict | Self::RemoveStage | Self::ApplyUpdateDelta => "high",
            Self::MergeStageOrder
            | Self::UnionMerge
            | Self::AddStage
            | Self::SetSection
            | Self::SetStageSection
            | Self::UnionMergeStageSection
            | Self::InferStageOrder
            | Self::FillDefault => "medium",
            Self::PreserveStageDetail
            | Self::ReuseWorkspace
            | Self::RecompileMachineOutputs
            | Self::NormalizeExternalBlueprint
            | Self::GeneratePackage
            | Self::NormalizeWithoutWorkspace => "low",
        }
    }

    fn review_required(self) -> bool {
        matches!(
            self,
            Self::RetainConflict | Self::RemoveStage | Self::ApplyUpdateDelta | Self::InferStageOrder
        )
    }

    fn apply_mode(self) -> &'static str {
        match self {
            Self::RetainConflict => "manual-review",
            Self::UnionMerge | Self::UnionMergeStageSection | Self::MergeStageOrder => "merge",
            Self::AddStage | Self::RemoveStage => "structural",
            Self::InferStageOrder
            | Self::FillDefault
            | Self::NormalizeExternalBlueprint
            | Self::GeneratePackage
            | Self::NormalizeWithoutWorkspace => "inference",
            Self::ApplyUpdateDelta => "delta-replay",
            Self::SetSection | Self::SetStageSection | Self::PreserveStageDetail => {
                "deterministic"
            }
            Self::ReuseWorkspace | Self::RecompileMachineOutputs => "reuse",
        }
    }
}

fn is_union_section(key: &str) -> bool {
    matches!(
        key,
        "authority_scope"
            | "truth_rules"
            | "non_goals"
            | "allowed_scope"
            | "forbidden_scope"
            | "deliverables"
            | "required_verification"
            | "review_focus"
            | "cross_stage_split_rule"
            | "stop_conditions"
    )
}

fn is_hard_conflict_section(key: &str, policy: &NormalizationPolicyToml) -> bool {
    !policy.allow_authority_rewrite
        && matches!(
            key,
            "purpose"
                | "conflict_resolution"
                | "entry_rule"
                | "exit_gate"
                | "advance_rule"
                | "repair_routing"
        )
}

fn merge_unique_lines(existing: &[String], update: &[String]) -> Vec<String> {
    let mut merged = existing.to_vec();
    for item in update {
        if !merged.contains(item) {
            merged.push(item.clone());
        }
    }
    merged
}

fn merge_stage_order(existing: &[String], update: &[String]) -> Result<Vec<String>, CoreError> {
    let existing_stages = existing
        .iter()
        .enumerate()
        .map(|(index, line)| parse_stage_line(line, index))
        .collect::<Vec<_>>();
    let update_stages = update
        .iter()
        .enumerate()
        .map(|(index, line)| parse_stage_line(line, index))
        .collect::<Vec<_>>();

    let mut existing_order = BTreeMap::new();
    for (index, stage) in existing_stages.iter().enumerate() {
        existing_order.insert(stage.stage_id.clone(), index);
    }

    let mut last_seen_existing_index = None;
    let mut merged = existing.to_vec();
    for stage in update_stages {
        if let Some(existing_index) = existing_order.get(&stage.stage_id) {
            let existing_stage = &existing_stages[*existing_index];
            if existing_stage.stage_name != stage.stage_name {
                return Err(CoreError::TooAmbiguousToNormalize {
                    reason: format!(
                        "update changed stage `{}` from `{}` to `{}`",
                        stage.stage_id, existing_stage.stage_name, stage.stage_name
                    ),
                });
            }
            if let Some(last_index) = last_seen_existing_index {
                if *existing_index < last_index {
                    return Err(CoreError::TooAmbiguousToNormalize {
                        reason: "update attempted to reorder existing stages".to_string(),
                    });
                }
            }
            last_seen_existing_index = Some(*existing_index);
        } else {
            merged.push(format!("{}: {}", stage.stage_id, stage.stage_name));
        }
    }

    Ok(merged)
}

fn hydrate_patch_plan_from_state(
    patch_operations: &mut [PatchOperation],
    sections: &BTreeMap<String, Vec<String>>,
    stage_sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
) {
    let stage_names = sections
        .get("stage_order")
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, line)| {
                    let stage = parse_stage_line(line, index);
                    (stage.stage_id, stage.stage_name)
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    for operation in patch_operations.iter_mut() {
        match operation.scope.as_str() {
            "section" => {
                if operation.value_lines.is_empty() {
                    operation.value_lines = sections
                        .get(&operation.target_id)
                        .cloned()
                        .unwrap_or_default();
                }
            }
            "stage" => {
                if operation.stage_name.is_none() {
                    operation.stage_name = stage_names.get(&operation.target_id).cloned();
                }
            }
            "stage-section" => {
                if let Some((stage_id, section_key)) = operation.target_id.split_once(':') {
                    if operation.value_lines.is_empty() {
                        operation.value_lines = stage_sections
                            .get(stage_id)
                            .and_then(|entries| entries.get(section_key))
                            .cloned()
                            .unwrap_or_default();
                    }
                    if operation.stage_name.is_none() {
                        operation.stage_name = stage_names.get(stage_id).cloned();
                    }
                }
            }
            _ => {}
        }
        if let Some(kind) = PatchStrategyKind::parse(&operation.strategy) {
            hydrate_patch_strategy_metadata(operation, kind, "forward", false);
        }
        if operation.reverse_strategy.is_none() {
            operation.reverse_strategy = Some(infer_reverse_strategy(operation));
        }
    }
}

fn hydrate_patch_plan_with_base_state(
    patch_operations: &mut [PatchOperation],
    sections: &BTreeMap<String, Vec<String>>,
    stage_sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
) {
    let stage_names = sections
        .get("stage_order")
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, line)| {
                    let stage = parse_stage_line(line, index);
                    (stage.stage_id, stage.stage_name)
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    for operation in patch_operations.iter_mut() {
        match operation.scope.as_str() {
            "section" => {
                operation.previous_value_lines = sections
                    .get(&operation.target_id)
                    .cloned()
                    .unwrap_or_default();
            }
            "stage" => {
                operation.previous_stage_name = stage_names.get(&operation.target_id).cloned();
            }
            "stage-section" => {
                if let Some((stage_id, section_key)) = operation.target_id.split_once(':') {
                    operation.previous_value_lines = stage_sections
                        .get(stage_id)
                        .and_then(|entries| entries.get(section_key))
                        .cloned()
                        .unwrap_or_default();
                    operation.previous_stage_name = stage_names.get(stage_id).cloned();
                }
            }
            _ => {}
        }
        if operation.reverse_strategy.is_none() {
            operation.reverse_strategy = Some(infer_reverse_strategy(operation));
        }
        if let Some(kind) = PatchStrategyKind::parse(&operation.strategy) {
            hydrate_patch_strategy_metadata(operation, kind, "forward", true);
        }
    }
}

fn hydrate_patch_strategy_metadata(
    operation: &mut PatchOperation,
    kind: PatchStrategyKind,
    replay_direction: &str,
    base_state_captured: bool,
) {
    operation
        .strategy_metadata
        .entry("strategy_family".to_string())
        .or_insert_with(|| kind.as_str().to_string());
    operation
        .strategy_metadata
        .entry("replay_direction".to_string())
        .or_insert_with(|| replay_direction.to_string());
    operation
        .strategy_metadata
        .entry("is_reversible".to_string())
        .or_insert_with(|| "true".to_string());
    operation
        .strategy_metadata
        .entry("risk_level".to_string())
        .or_insert_with(|| kind.risk_level().to_string());
    operation
        .strategy_metadata
        .entry("review_required".to_string())
        .or_insert_with(|| kind.review_required().to_string());
    operation
        .strategy_metadata
        .entry("apply_mode".to_string())
        .or_insert_with(|| kind.apply_mode().to_string());
    if base_state_captured {
        operation
            .strategy_metadata
            .entry("base_state_captured".to_string())
            .or_insert_with(|| "true".to_string());
    }
}

fn apply_patch_plan_to_source(
    base: &ParsedBlueprintSource,
    patch_plan: &PatchPlan,
) -> Result<ParsedBlueprintSource, CoreError> {
    apply_patch_plan_to_source_in_direction(base, patch_plan, PatchReplayDirection::Forward)
}

fn apply_patch_plan_to_source_in_direction(
    base: &ParsedBlueprintSource,
    patch_plan: &PatchPlan,
    direction: PatchReplayDirection,
) -> Result<ParsedBlueprintSource, CoreError> {
    let mut applied = base.clone();
    let operations = match direction {
        PatchReplayDirection::Forward => patch_plan.operations.iter().collect::<Vec<_>>(),
        PatchReplayDirection::Reverse => patch_plan.operations.iter().rev().collect::<Vec<_>>(),
    };

    for operation in operations {
        let strategy = resolve_patch_strategy(operation, direction)?;
        let value_lines = resolved_patch_value_lines(operation, direction);
        let stage_name = resolved_patch_stage_name(operation, direction);
        match operation.scope.as_str() {
            "package" => {}
            "section" => {
                let current = applied
                    .sections
                    .get(&operation.target_id)
                    .cloned()
                    .unwrap_or_default();
                let next = match strategy {
                    PatchStrategyKind::InferStageOrder | PatchStrategyKind::FillDefault => {
                        value_lines.to_vec()
                    }
                    PatchStrategyKind::MergeStageOrder => merge_stage_order(&current, value_lines)?,
                    PatchStrategyKind::UnionMerge | PatchStrategyKind::RetainConflict => {
                        merge_unique_lines(&current, value_lines)
                    }
                    PatchStrategyKind::SetSection => value_lines.to_vec(),
                    strategy_kind if legacy_patch_strategy_is_noop(strategy_kind) => current,
                    _ => {
                        return Err(CoreError::ChangeReportInconsistent {
                            field: "patch_plan.strategy".to_string(),
                        })
                    }
                };
                if next.is_empty() {
                    applied.sections.remove(&operation.target_id);
                } else {
                    applied.sections.insert(operation.target_id.clone(), next);
                }
            }
            "stage" => match strategy {
                PatchStrategyKind::AddStage => {
                    let stage_entry = applied
                        .stage_sections
                        .entry(operation.target_id.clone())
                        .or_default();
                    if let Some(stage_name) = &stage_name {
                        stage_entry.insert("stage_name".to_string(), vec![stage_name.clone()]);
                    }
                }
                PatchStrategyKind::RemoveStage => {
                    applied.stage_sections.remove(&operation.target_id);
                }
                strategy_kind if legacy_patch_strategy_is_noop(strategy_kind) => {}
                _ => {
                    return Err(CoreError::ChangeReportInconsistent {
                        field: "patch_plan.stage_strategy".to_string(),
                    })
                }
            },
            "stage-section" => {
                let Some((stage_id, section_key)) = operation.target_id.split_once(':') else {
                    return Err(CoreError::ChangeReportInconsistent {
                        field: "patch_plan.stage_section_target".to_string(),
                    });
                };
                let stage_entry = applied
                    .stage_sections
                    .entry(stage_id.to_string())
                    .or_default();
                let current = stage_entry.get(section_key).cloned().unwrap_or_default();
                let next = match strategy {
                    PatchStrategyKind::UnionMergeStageSection
                    | PatchStrategyKind::RetainConflict => {
                        merge_unique_lines(&current, value_lines)
                    }
                    PatchStrategyKind::SetStageSection => value_lines.to_vec(),
                    PatchStrategyKind::PreserveStageDetail => current,
                    _ => {
                        return Err(CoreError::ChangeReportInconsistent {
                            field: "patch_plan.stage_section_strategy".to_string(),
                        })
                    }
                };
                if next.is_empty() {
                    stage_entry.remove(section_key);
                } else {
                    stage_entry.insert(section_key.to_string(), next);
                }
                if let Some(stage_name) = &stage_name {
                    stage_entry.insert("stage_name".to_string(), vec![stage_name.clone()]);
                } else if section_key == "stage_name" && value_lines.is_empty() {
                    stage_entry.remove("stage_name");
                }
            }
            _ => {
                return Err(CoreError::ChangeReportInconsistent {
                    field: "patch_plan.scope".to_string(),
                });
            }
        }
    }

    applied
        .stage_sections
        .retain(|_, sections| !sections.is_empty());
    applied.semantic_frames =
        fallback_semantic_frames_for_source(&applied.sections, &applied.stage_sections, &[], &[]);

    Ok(applied)
}

fn resolve_patch_strategy(
    operation: &PatchOperation,
    direction: PatchReplayDirection,
) -> Result<PatchStrategyKind, CoreError> {
    let inferred_reverse_strategy;
    let raw = match direction {
        PatchReplayDirection::Forward => operation.strategy.as_str(),
        PatchReplayDirection::Reverse => match operation.reverse_strategy.as_deref() {
            Some(strategy) => strategy,
            None => {
                inferred_reverse_strategy = infer_reverse_strategy(operation);
                inferred_reverse_strategy.as_str()
            }
        },
    };
    PatchStrategyKind::parse(raw).ok_or_else(|| CoreError::ChangeReportInconsistent {
        field: "patch_plan.strategy".to_string(),
    })
}

fn resolved_patch_value_lines<'a>(
    operation: &'a PatchOperation,
    direction: PatchReplayDirection,
) -> &'a [String] {
    match direction {
        PatchReplayDirection::Forward => &operation.value_lines,
        PatchReplayDirection::Reverse => &operation.previous_value_lines,
    }
}

fn resolved_patch_stage_name(
    operation: &PatchOperation,
    direction: PatchReplayDirection,
) -> Option<String> {
    match direction {
        PatchReplayDirection::Forward => operation.stage_name.clone(),
        PatchReplayDirection::Reverse => operation.previous_stage_name.clone(),
    }
}

fn infer_reverse_strategy(operation: &PatchOperation) -> String {
    match (operation.scope.as_str(), operation.strategy.as_str()) {
        ("stage", "preserve-stage-detail") => {
            PatchStrategyKind::PreserveStageDetail.as_str().to_string()
        }
        ("package", strategy) => strategy.to_string(),
        ("section", _) => PatchStrategyKind::SetSection.as_str().to_string(),
        ("stage-section", _) => PatchStrategyKind::SetStageSection.as_str().to_string(),
        ("stage", _) => {
            if operation.previous_stage_name.is_none() {
                PatchStrategyKind::RemoveStage.as_str().to_string()
            } else {
                PatchStrategyKind::AddStage.as_str().to_string()
            }
        }
        _ => PatchStrategyKind::SetSection.as_str().to_string(),
    }
}

fn legacy_patch_strategy_is_noop(strategy: PatchStrategyKind) -> bool {
    matches!(
        strategy,
        PatchStrategyKind::RecompileMachineOutputs
            | PatchStrategyKind::NormalizeExternalBlueprint
            | PatchStrategyKind::GeneratePackage
            | PatchStrategyKind::ApplyUpdateDelta
            | PatchStrategyKind::ReuseWorkspace
            | PatchStrategyKind::NormalizeWithoutWorkspace
            | PatchStrategyKind::PreserveStageDetail
    )
}

fn render_authority_doc(
    input: &BlueprintAuthorInput,
    sections: &BTreeMap<String, Vec<String>>,
) -> String {
    format!(
        "# Authority Root\n\n## Purpose\n\n{}\n\n## Authority Scope\n\n{}\n\n## Truth Rules\n\n{}\n\n## Conflict Resolution\n\n{}\n\n## Non-Goals\n\n{}\n",
        section_paragraph(sections, "purpose", &format!("Deliver the blueprint package for `{}`.", input.project_name.trim())),
        section_list(sections, "authority_scope"),
        section_list(sections, "truth_rules"),
        section_list(sections, "conflict_resolution"),
        section_list(sections, "non_goals"),
    )
}

fn render_workflow_doc(
    sections: &BTreeMap<String, Vec<String>>,
    stages: &[ContractStage],
    worktree_protocol: &WorktreeProtocol,
) -> String {
    let stage_lines = stages
        .iter()
        .map(|stage| format!("1. `{}` {}", stage.stage_id, stage.stage_name))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "# Workflow Overview\n\n## Stage Order\n\n{}\n\n## Entry Rule\n\n{}\n\n## Exit Gate\n\n{}\n\n## Cross-Stage Split Rule\n\n{}\n\n## Stop Conditions\n\n{}\n\n## Worktree Model\n\n{}\n\n## Parallel Worktree Policy\n\n{}\n\n## Shared Authority Paths\n\n{}\n\n## Worktree Roles\n\n{}\n\n## Worktree Sync Rule\n\n{}\n\n## Worktree Merge Back Rule\n\n{}\n\n## Worktree Cleanup Rule\n\n{}\n",
        if stage_lines.is_empty() { "1. `stage-01` foundation".to_string() } else { stage_lines },
        section_list(sections, "entry_rule"),
        section_list(sections, "exit_gate"),
        section_list(sections, "cross_stage_split_rule"),
        section_list(sections, "stop_conditions"),
        section_paragraph(
            sections,
            "worktree_model",
            if worktree_protocol.model.trim().is_empty() {
                "stage-isolated-worktree"
            } else {
                &worktree_protocol.model
            },
        ),
        section_list(sections, "parallel_worktree_policy"),
        section_path_list(sections, "shared_authority_paths"),
        section_list(sections, "worktree_roles"),
        section_list(sections, "worktree_sync_rule"),
        section_list(sections, "worktree_merge_back_rule"),
        section_list(sections, "worktree_cleanup_rule"),
    )
}

fn render_stage_doc(sections: &BTreeMap<String, Vec<String>>, stage: &ContractStage) -> String {
    format!(
        "# Stage Document\n\n## Stage\n\n- `stage_id`: `{}`\n- `stage_name`: `{}`\n\n## Intent\n\n{}\n\n## Allowed Scope\n\n{}\n\n## Forbidden Scope\n\n{}\n\n## Deliverables\n\n{}\n\n## Required Verification\n\n{}\n\n## Review Focus\n\n{}\n\n## Advance Rule\n\n{}\n\n## Repair Routing\n\n{}\n",
        stage.stage_id,
        stage.stage_name,
        section_paragraph(sections, "purpose", "Deliver the blueprint package."),
        section_list(sections, "allowed_scope"),
        section_list(sections, "forbidden_scope"),
        section_list(sections, "deliverables"),
        section_list(sections, "required_verification"),
        section_list(sections, "review_focus"),
        section_list(sections, "advance_rule"),
        section_list(sections, "repair_routing"),
    )
}

fn render_module_policy_doc(catalog: &ModuleCatalog) -> String {
    let entries = catalog
        .modules
        .iter()
        .map(|module| {
            format!(
                "- `{}`: {} (allowed: {})",
                module.module_id,
                module.recommended_language.as_str(),
                module
                    .allowed_languages
                    .iter()
                    .map(|language| language.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("# Module Language Policy\n\n{}\n", entries)
}

fn render_author_report(
    mode: AuthorMode,
    input: &BlueprintAuthorInput,
    normalization_report: &NormalizationReport,
    change_report: &ChangeReport,
    module_catalog: &ModuleCatalog,
    readiness: &ReadinessReport,
    task_progress: &TaskProgressReport,
) -> String {
    let readiness_holds_block = if readiness.gate_holds.is_empty() {
        "none".to_string()
    } else {
        list_inline(&readiness.gate_holds)
    };
    let readiness_actions_block = if readiness.recommended_actions.is_empty() {
        "none".to_string()
    } else {
        list_inline(&readiness.recommended_actions)
    };
    let source_path_line = input
        .source_path
        .as_ref()
        .map(|path| format!("- Source path: `{}`\n", normalize_repo_relative_path(path)))
        .unwrap_or_default();
    let ambiguity_block = if normalization_report.unresolved_ambiguities.is_empty() {
        "- None\n".to_string()
    } else {
        normalization_report
            .unresolved_ambiguities
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };
    let source_files_block = if normalization_report.source_files.is_empty() {
        "none".to_string()
    } else {
        list_inline(&normalization_report.source_files)
    };
    let dropped_sections_block = if normalization_report.dropped_sections.is_empty() {
        "none".to_string()
    } else {
        list_inline(&normalization_report.dropped_sections)
    };
    let semantic_hints_block = if normalization_report.semantic_hints.is_empty() {
        "none".to_string()
    } else {
        list_inline(&normalization_report.semantic_hints)
    };
    let semantic_risks_block = if normalization_report.semantic_risks.is_empty() {
        "none".to_string()
    } else {
        list_inline(&normalization_report.semantic_risks)
    };
    format!(
        "# Author Report\n\n## Mode\n\n`{}`\n\n## Source Summary\n\n- Summary: {}\n{}## Normalization Summary\n\n- Source files: {}\n- Preserved sections: {}\n- Inferred sections: {}\n- Dropped sections: {}\n- Semantic hints: {}\n- Semantic risks: {}\n- Status: {}\n\n## Change Summary\n\n- Change report: `.codex/auto-dev/change-report.json`\n- Operation count: {}\n- Conflict count: {}\n- Patch operations: {}\n\n## Module Language Planning Summary\n\n- Module count: {}\n- Core runtime language: rust\n\n## Contract Summary\n\n- Human truth: `.codex/auto-dev/project-contract.toml`\n- Machine truth: `.codex/auto-dev/resolved-contract.json`\n- Task progress: `.codex/auto-dev/task-progress.json`\n\n## Task Progress\n\n- Total stages: {}\n- Completed stages: {}\n- Overall progress: {}%\n- Current stage: {}\n\n## Readiness\n\n- State: `{}`\n- Fingerprint: `{}`\n- Blocking semantic conflicts: {}\n- Review-required semantic conflicts: {}\n- Gate holds: {}\n- Recommended actions: {}\n\n## Blockers\n\n{}",
        mode.as_str(),
        input.source_summary,
        source_path_line,
        source_files_block,
        list_inline(&normalization_report.preserved_sections),
        list_inline(&normalization_report.inferred_sections),
        dropped_sections_block,
        semantic_hints_block,
        semantic_risks_block,
        normalization_report.status,
        change_report.operation_count,
        change_report.conflict_count,
        change_report.patch_operation_count,
        module_catalog.modules.len(),
        task_progress.total_stages,
        task_progress.completed_stages,
        task_progress.overall_progress_percent,
        task_progress.current_stage_id.as_deref().unwrap_or("none"),
        readiness.state,
        readiness.fingerprint,
        readiness.blocking_semantic_conflict_count,
        readiness.review_required_semantic_conflict_count,
        readiness_holds_block,
        readiness_actions_block,
        ambiguity_block
    )
}

fn manifest_entry(
    path: &str,
    doc_role: &str,
    canonical_id: &str,
    source_provenance: &str,
    content: &str,
) -> BlueprintManifestEntry {
    let normalized_path = normalize_repo_relative_path(path);
    BlueprintManifestEntry {
        fingerprint: fingerprint_text(&format!("{source_provenance}:{normalized_path}:{content}")),
        path: normalized_path,
        doc_role: doc_role.to_string(),
        canonical_id: canonical_id.to_string(),
        source_provenance: source_provenance.to_string(),
    }
}

fn package_manifest_fingerprint(manifest: &BlueprintManifest) -> String {
    manifest_fingerprint_from_entries(&manifest.files)
}

fn manifest_fingerprint_from_entries(entries: &[BlueprintManifestEntry]) -> String {
    let payload = entries
        .iter()
        .map(|entry| {
            format!(
                "{}:{}:{}:{}:{}",
                entry.path,
                entry.doc_role,
                entry.canonical_id,
                entry.source_provenance,
                entry.fingerprint
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    fingerprint_text(&payload)
}

fn package_contract_fingerprint(contract: &ResolvedContract) -> String {
    let payload = contract
        .stages
        .iter()
        .map(|stage| {
            format!(
                "{}:{}:{}",
                stage.stage_id, stage.stage_name, stage.default_next_goal
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let worktree_payload = package_worktree_protocol_fingerprint(&effective_worktree_protocol(
        &contract.worktree,
        &WorktreeProtocol::default(),
    ));
    fingerprint_text(&format!(
        "{}:{}:{}:{}:{}:{}:{}:{}",
        contract.project_name,
        contract.workflow_mode,
        contract.paths.blueprint_root,
        contract.paths.authority_root,
        contract.paths.workflow_root,
        contract.paths.stages_root,
        payload,
        worktree_payload
    ))
}

fn package_worktree_protocol_fingerprint(protocol: &WorktreeProtocol) -> String {
    let role_payload = protocol
        .roles
        .iter()
        .map(|role| {
            format!(
                "{}:{}:{}:{}:{}",
                role.role_id,
                role.branch_pattern,
                role.stage_ids.join(","),
                role.module_ids.join(","),
                role.exclusive_paths.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    fingerprint_text(&format!(
        "{}:{}:{}:{}:{}:{}:{}",
        protocol.model,
        protocol.parallel_worktree_policy.join("|"),
        protocol.shared_authority_paths.join("|"),
        protocol.sync_rule.join("|"),
        protocol.merge_back_rule.join("|"),
        protocol.cleanup_rule.join("|"),
        role_payload
    ))
}

fn package_normalization_fingerprint(report: &NormalizationReport) -> String {
    let payload = [
        report.source_type.clone(),
        report.source_files.join("|"),
        report.preserved_sections.join("|"),
        report.inferred_sections.join("|"),
        report.dropped_sections.join("|"),
        report.semantic_hints.join("|"),
        report.semantic_risks.join("|"),
        report.unresolved_ambiguities.join("|"),
        report.status.clone(),
    ]
    .join("::");
    fingerprint_text(&payload)
}

fn package_change_report_fingerprint(report: &ChangeReport) -> String {
    let payload = report
        .operations
        .iter()
        .map(|operation| {
            format!(
                "{}:{}:{}:{}",
                operation.target_kind, operation.target_id, operation.action, operation.details
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let patch_payload = report
        .patch_operations
        .iter()
        .map(|operation| {
            format!(
                "{}:{}:{}:{}:{}",
                operation.scope,
                operation.target_id,
                operation.strategy,
                operation.status,
                operation.details
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    fingerprint_text(&format!(
        "{}:{}:{}:{}:{}:{}",
        report.mode,
        report.operation_count,
        report.conflict_count,
        report.patch_operation_count,
        payload,
        patch_payload
    ))
}

fn parsed_source_fingerprint(source: &ParsedBlueprintSource) -> String {
    let section_payload = source
        .sections
        .iter()
        .map(|(key, values)| format!("{key}={}", values.join("\u{1f}")))
        .collect::<Vec<_>>()
        .join("|");
    let stage_payload = source
        .stage_sections
        .iter()
        .map(|(stage_id, sections)| {
            let payload = sections
                .iter()
                .map(|(key, values)| format!("{key}={}", values.join("\u{1f}")))
                .collect::<Vec<_>>()
                .join("&");
            format!("{stage_id}:{payload}")
        })
        .collect::<Vec<_>>()
        .join("|");
    fingerprint_text(&format!("{section_payload}::{stage_payload}"))
}

fn build_patch_base(mode: AuthorMode, base_source: &ParsedBlueprintSource) -> PatchBase {
    PatchBase {
        schema_version: PATCH_BASE_SCHEMA_VERSION.to_string(),
        mode: mode.as_str().to_string(),
        artifact_status: "emitted".to_string(),
        base_fingerprint: parsed_source_fingerprint(base_source),
        sections: base_source.sections.clone(),
        stage_sections: base_source.stage_sections.clone(),
    }
}

fn build_patch_base_fallback(
    mode: AuthorMode,
    current_source: &ParsedBlueprintSource,
    patch_plan: &PatchPlan,
) -> PatchBase {
    let current_source_fingerprint = parsed_source_fingerprint(current_source);
    let can_derive_current_state = !matches!(mode, AuthorMode::UpdateBlueprint)
        || patch_plan.base_fingerprint.trim().is_empty()
        || patch_plan.base_fingerprint == current_source_fingerprint;
    if can_derive_current_state {
        PatchBase {
            schema_version: PATCH_BASE_SCHEMA_VERSION.to_string(),
            mode: mode.as_str().to_string(),
            artifact_status: "legacy-derived-current-state".to_string(),
            base_fingerprint: current_source_fingerprint,
            sections: current_source.sections.clone(),
            stage_sections: current_source.stage_sections.clone(),
        }
    } else {
        PatchBase {
            schema_version: PATCH_BASE_SCHEMA_VERSION.to_string(),
            mode: mode.as_str().to_string(),
            artifact_status: "legacy-unavailable".to_string(),
            base_fingerprint: patch_plan.base_fingerprint.clone(),
            sections: BTreeMap::new(),
            stage_sections: BTreeMap::new(),
        }
    }
}

fn patch_operations_need_worktree_scope_hydration(patch_operations: &[PatchOperation]) -> bool {
    !patch_operations.is_empty()
        && patch_operations.iter().all(|operation| {
            operation.affected_paths.is_empty() && operation.target_worktree_roles.is_empty()
        })
}

fn hydrate_patch_operation_worktree_scope(
    patch_operations: &mut [PatchOperation],
    resolved_contract: &ResolvedContract,
    worktree_protocol: &WorktreeProtocol,
    stage_documents: &[StageDocument],
) -> Result<(), CoreError> {
    for operation in patch_operations.iter_mut() {
        let (affected_paths, target_worktree_roles) = derive_patch_operation_worktree_scope(
            operation,
            resolved_contract,
            worktree_protocol,
            stage_documents,
        )?;
        operation.affected_paths = affected_paths;
        operation.target_worktree_roles = target_worktree_roles;
    }
    Ok(())
}

fn derive_patch_operation_worktree_scope(
    operation: &PatchOperation,
    resolved_contract: &ResolvedContract,
    worktree_protocol: &WorktreeProtocol,
    stage_documents: &[StageDocument],
) -> Result<(Vec<String>, Vec<String>), CoreError> {
    let mut affected_paths = BTreeSet::new();
    let mut target_worktree_roles = BTreeSet::new();

    match operation.scope.as_str() {
        "package" => {
            for path in &worktree_protocol.shared_authority_paths {
                affected_paths.insert(normalize_repo_relative_path(path));
            }
            if affected_paths.is_empty() {
                affected_paths.insert(resolved_contract.paths.contract_root.clone());
            }
        }
        "section" => {
            let path = if is_workflow_patch_section(&operation.target_id) {
                format!(
                    "{}/00-workflow-overview.md",
                    resolved_contract.paths.workflow_root
                )
            } else {
                format!(
                    "{}/00-authority-root.md",
                    resolved_contract.paths.authority_root
                )
            };
            affected_paths.insert(normalize_repo_relative_path(&path));
        }
        "stage" => {
            let stage_path =
                stage_document_path_for_scope(&operation.target_id, stage_documents).ok_or_else(
                    || CoreError::ChangeReportInconsistent {
                        field: "patch_plan.affected_paths".to_string(),
                    },
                )?;
            affected_paths.insert(stage_path.clone());
            extend_role_scope_for_stage(
                &mut target_worktree_roles,
                &operation.target_id,
                &stage_path,
                worktree_protocol,
            );
        }
        "stage-section" => {
            let Some((stage_id, _section_key)) = operation.target_id.split_once(':') else {
                return Err(CoreError::ChangeReportInconsistent {
                    field: "patch_plan.stage_section_target".to_string(),
                });
            };
            let stage_path =
                stage_document_path_for_scope(stage_id, stage_documents).ok_or_else(|| {
                    CoreError::ChangeReportInconsistent {
                        field: "patch_plan.affected_paths".to_string(),
                    }
                })?;
            affected_paths.insert(stage_path.clone());
            extend_role_scope_for_stage(
                &mut target_worktree_roles,
                stage_id,
                &stage_path,
                worktree_protocol,
            );
        }
        _ => {
            return Err(CoreError::ChangeReportInconsistent {
                field: "patch_plan.scope".to_string(),
            });
        }
    }

    for path in &affected_paths {
        for role in worktree_roles_for_path(path, worktree_protocol) {
            target_worktree_roles.insert(role);
        }
    }

    Ok((
        affected_paths.into_iter().collect(),
        target_worktree_roles.into_iter().collect(),
    ))
}

fn is_workflow_patch_section(section_key: &str) -> bool {
    matches!(
        section_key,
        "stage_order"
            | "cross_stage_split_rule"
            | "stop_conditions"
            | "entry_rule"
            | "exit_gate"
            | "worktree_model"
            | "parallel_worktree_policy"
            | "shared_authority_paths"
            | "worktree_roles"
            | "worktree_sync_rule"
            | "worktree_merge_back_rule"
            | "worktree_cleanup_rule"
    )
}

fn stage_document_path_for_scope(
    stage_id: &str,
    stage_documents: &[StageDocument],
) -> Option<String> {
    stage_documents
        .iter()
        .find(|document| document.stage_id == stage_id)
        .map(|document| normalize_repo_relative_path(&document.path))
}

fn extend_role_scope_for_stage(
    target_worktree_roles: &mut BTreeSet<String>,
    stage_id: &str,
    stage_path: &str,
    worktree_protocol: &WorktreeProtocol,
) {
    for role in &worktree_protocol.roles {
        if role.stage_ids.iter().any(|value| value == stage_id)
            || role
                .exclusive_paths
                .iter()
                .any(|path| path_matches_worktree_scope(stage_path, path))
        {
            target_worktree_roles.insert(role.role_id.clone());
        }
    }
}

fn worktree_roles_for_path(path: &str, worktree_protocol: &WorktreeProtocol) -> Vec<String> {
    worktree_protocol
        .roles
        .iter()
        .filter(|role| {
            role.exclusive_paths
                .iter()
                .any(|scope_path| path_matches_worktree_scope(path, scope_path))
        })
        .map(|role| role.role_id.clone())
        .collect()
}

fn decision_scope_worktree_roles(
    scope: &str,
    resolved_contract: &ResolvedContract,
    worktree_protocol: &WorktreeProtocol,
    stage_documents: &[StageDocument],
) -> Result<Vec<String>, CoreError> {
    if let Some(stage_id) = scope.strip_prefix("stage:") {
        let stage_path =
            stage_document_path_for_scope(stage_id, stage_documents).ok_or_else(|| {
                CoreError::ReadinessInconsistent {
                    field: "decision_summary.worktree_roles".to_string(),
                }
            })?;
        let mut roles = BTreeSet::new();
        extend_role_scope_for_stage(&mut roles, stage_id, &stage_path, worktree_protocol);
        for role in worktree_roles_for_path(&stage_path, worktree_protocol) {
            roles.insert(role);
        }
        return Ok(roles.into_iter().collect());
    }

    let normalized_scope = normalize_repo_relative_path(scope);
    if normalized_scope == "package" {
        return Ok(Vec::new());
    }
    if normalized_scope == "workflow" {
        let workflow_path = format!(
            "{}/00-workflow-overview.md",
            resolved_contract.paths.workflow_root
        );
        return Ok(worktree_roles_for_path(&workflow_path, worktree_protocol));
    }
    if normalized_scope == "authority" {
        let authority_path = format!(
            "{}/00-authority-root.md",
            resolved_contract.paths.authority_root
        );
        return Ok(worktree_roles_for_path(&authority_path, worktree_protocol));
    }

    Ok(Vec::new())
}

fn path_matches_worktree_scope(path: &str, scope_path: &str) -> bool {
    let normalized_path = normalize_repo_relative_path(path);
    let normalized_scope = normalize_repo_relative_path(scope_path);
    normalized_path == normalized_scope
        || normalized_path.starts_with(&(normalized_scope.clone() + "/"))
}

fn worktree_branch_patterns_overlap(left: &str, right: &str) -> bool {
    let left = left.trim().trim_matches('/');
    let right = right.trim().trim_matches('/');
    if left.is_empty() || right.is_empty() {
        return false;
    }
    left == right
        || left.starts_with(&format!("{right}/"))
        || right.starts_with(&format!("{left}/"))
}

fn worktree_branch_pattern_is_valid(pattern: &str) -> bool {
    let trimmed = pattern.trim();
    if trimmed.is_empty()
        || trimmed.contains('\\')
        || trimmed.contains(' ')
        || trimmed.contains("..")
        || trimmed.contains("@{")
        || trimmed.ends_with('/')
        || trimmed.starts_with('/')
        || trimmed.ends_with(".lock")
        || trimmed.starts_with('.')
        || trimmed.ends_with('.')
    {
        return false;
    }
    !trimmed
        .chars()
        .any(|character| character.is_control() || matches!(character, '~' | '^' | ':' | '?' | '*' | '['))
}

fn worktree_rule_is_placeholder(value: &str) -> bool {
    let normalized = value
        .trim()
        .trim_matches(|character: char| character.is_ascii_punctuation() || character.is_whitespace())
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "todo" | "tbd" | "none" | "n/a" | "na" | "not set" | "unset" | "pending" | "later"
    )
}

fn worktree_rule_set_has_actionable_entry(
    field: &str,
    values: &[String],
    protocol: &WorktreeProtocol,
) -> bool {
    values
        .iter()
        .any(|value| worktree_rule_looks_actionable(field, value, protocol))
}

fn worktree_rule_set_mentions_shared_authority(
    values: &[String],
    shared_authority_paths: &[String],
) -> bool {
    values
        .iter()
        .any(|value| worktree_rule_mentions_shared_authority(value, shared_authority_paths))
}

fn worktree_rule_mentions_shared_authority(
    value: &str,
    shared_authority_paths: &[String],
) -> bool {
    let lowered = value.to_ascii_lowercase();
    lowered.contains("shared authority")
        || lowered.contains("shared-authority")
        || lowered.contains("workflow")
        || lowered.contains("contract root")
        || shared_authority_paths
            .iter()
            .any(|path| lowered.contains(&path.to_ascii_lowercase()))
}

fn worktree_rule_looks_actionable(
    field: &str,
    value: &str,
    protocol: &WorktreeProtocol,
) -> bool {
    let lowered = value.to_ascii_lowercase();
    let has_scope = lowered.contains("worktree")
        || lowered.contains("branch")
        || lowered.contains("stage")
        || lowered.contains("role")
        || lowered.contains("authority")
        || lowered.contains("module")
        || protocol
            .roles
            .iter()
            .any(|role| lowered.contains(&role.role_id.to_ascii_lowercase()));
    let has_action = match field {
        "worktree.parallel_worktree_policy" => [
            "parallel",
            "shared",
            "exclusive",
            "isolate",
            "serialize",
            "coordinate",
        ]
        .iter()
        .any(|token| lowered.contains(token)),
        "worktree.sync_rule" => ["sync", "rebase", "pull", "update", "refresh"]
            .iter()
            .any(|token| lowered.contains(token)),
        "worktree.merge_back_rule" => ["merge", "rebase", "squash", "fast-forward", "ff-only"]
            .iter()
            .any(|token| lowered.contains(token)),
        "worktree.cleanup_rule" => ["delete", "remove", "cleanup", "archive", "recycle", "prune"]
            .iter()
            .any(|token| lowered.contains(token)),
        _ => false,
    };
    has_scope && has_action
}

fn worktree_rule_set_mentions_model_scope(field: &str, model: &str, values: &[String]) -> bool {
    values
        .iter()
        .any(|value| worktree_rule_mentions_model_scope(field, model, value))
}

fn worktree_rule_mentions_model_scope(field: &str, model: &str, value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    match (field, model) {
        ("worktree.parallel_worktree_policy", "stage-isolated-worktree") => lowered.contains("stage"),
        ("worktree.parallel_worktree_policy", "module-isolated-worktree") => lowered.contains("module"),
        ("worktree.sync_rule", "stage-isolated-worktree") => lowered.contains("stage"),
        ("worktree.sync_rule", "module-isolated-worktree") => lowered.contains("module"),
        ("worktree.merge_back_rule", "stage-isolated-worktree") => lowered.contains("stage"),
        ("worktree.merge_back_rule", "module-isolated-worktree") => lowered.contains("module"),
        ("worktree.cleanup_rule", "stage-isolated-worktree") => lowered.contains("stage"),
        ("worktree.cleanup_rule", "module-isolated-worktree") => lowered.contains("module"),
        _ => false,
    }
}

fn worktree_rule_conflicts_with_model(field: &str, model: &str, value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    match (field, model) {
        ("worktree.parallel_worktree_policy", "stage-isolated-worktree")
        | ("worktree.sync_rule", "stage-isolated-worktree")
        | ("worktree.merge_back_rule", "stage-isolated-worktree")
        | ("worktree.cleanup_rule", "stage-isolated-worktree") => {
            lowered.contains("module-isolated")
                || lowered.contains("per module")
                || lowered.contains("module role")
                || lowered.contains("module-scoped")
        }
        ("worktree.parallel_worktree_policy", "module-isolated-worktree")
        | ("worktree.sync_rule", "module-isolated-worktree")
        | ("worktree.merge_back_rule", "module-isolated-worktree")
        | ("worktree.cleanup_rule", "module-isolated-worktree") => {
            lowered.contains("stage-isolated")
                || lowered.contains("per stage role")
                || lowered.contains("stage-scoped")
                || lowered.contains("stage task")
                || lowered.contains("next-stage")
        }
        _ => true,
    }
}

fn worktree_paths_overlap(left: &str, right: &str) -> bool {
    path_matches_worktree_scope(left, right) || path_matches_worktree_scope(right, left)
}

fn collect_patch_operation_scope_mismatches(
    patch_operations: &[PatchOperation],
    resolved_contract: &ResolvedContract,
    worktree_protocol: &WorktreeProtocol,
    stage_documents: &[StageDocument],
) -> Result<Vec<String>, CoreError> {
    let mut mismatches = Vec::new();
    for operation in patch_operations {
        let (expected_paths, expected_roles) = derive_patch_operation_worktree_scope(
            operation,
            resolved_contract,
            worktree_protocol,
            stage_documents,
        )?;
        let actual_paths = normalize_unique_paths(&operation.affected_paths);
        let actual_roles = normalize_unique_strings(&operation.target_worktree_roles);
        if actual_paths != expected_paths {
            mismatches.push(format!(
                "{}:{} affected_paths mismatch",
                operation.scope, operation.target_id
            ));
        }
        if actual_roles != expected_roles {
            mismatches.push(format!(
                "{}:{} target_worktree_roles mismatch",
                operation.scope, operation.target_id
            ));
        }
    }
    Ok(mismatches)
}

fn normalize_unique_strings(values: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

fn patch_base_to_source(patch_base: &PatchBase) -> Option<ParsedBlueprintSource> {
    if patch_base.artifact_status == "legacy-unavailable" {
        None
    } else {
        Some(ParsedBlueprintSource {
            sections: patch_base.sections.clone(),
            stage_sections: patch_base.stage_sections.clone(),
            semantic_frames: Vec::new(),
        })
    }
}

fn reconstruct_result_source(
    mode: AuthorMode,
    patch_base: &PatchBase,
    patch_plan: &PatchPlan,
    projection_fallback: &ParsedBlueprintSource,
) -> Result<ParsedBlueprintSource, CoreError> {
    let Some(base_source) = patch_base_to_source(patch_base) else {
        return Ok(projection_fallback.clone());
    };

    if !matches!(mode, AuthorMode::UpdateBlueprint) || patch_plan.operations.is_empty() {
        return Ok(base_source);
    }

    apply_patch_plan_to_source_in_direction(&base_source, patch_plan, PatchReplayDirection::Forward)
}

fn package_patch_plan_fingerprint(plan: &PatchPlan) -> String {
    let operations = plan
        .operations
        .iter()
        .map(|operation| {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                operation.scope,
                operation.target_id,
                operation.strategy,
                operation.status,
                operation.details,
                operation.affected_paths.join("\u{1d}"),
                operation.target_worktree_roles.join("\u{1d}"),
                operation.value_lines.join("\u{1f}"),
                operation.stage_name.clone().unwrap_or_default(),
                operation.previous_value_lines.join("\u{1f}"),
                operation.previous_stage_name.clone().unwrap_or_default(),
                operation.reverse_strategy.clone().unwrap_or_default(),
                operation
                    .strategy_metadata
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join("\u{1e}")
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    fingerprint_text(&format!(
        "{}:{}:{}:{}:{}:{}",
        plan.mode,
        plan.operation_count,
        plan.conflict_count,
        plan.base_fingerprint,
        plan.result_fingerprint,
        operations
    ))
}

fn package_patch_execution_report_fingerprint(report: &PatchExecutionReport) -> String {
    fingerprint_text(&format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        report.mode,
        report.base_fingerprint,
        report.expected_result_fingerprint,
        report.replayed_result_fingerprint,
        report.replay_status,
        report.operation_count,
        report.applied_operation_count,
        report.mismatches.join("|"),
        report.reverse_replayed_base_fingerprint,
        report.reversibility_status,
        report.reverse_mismatch_count,
        report.reverse_mismatches.join("|"),
        report.scope_validation_status,
        report.scope_mismatch_count,
        report.scope_mismatches.join("|")
    ))
}

fn package_decision_summary_fingerprint(summary: &DecisionSummary) -> String {
    let entries = summary
        .entries
        .iter()
        .map(|entry| {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}",
                entry.kind,
                entry.scope,
                entry.target_id,
                entry.severity,
                entry.blocking,
                entry.review_required,
                entry.worktree_roles.join("\u{1d}"),
                entry.recommended_action
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let top_blockers = summary
        .top_blockers
        .iter()
        .map(decision_summary_entry_fingerprint)
        .collect::<Vec<_>>()
        .join("|");
    let top_review_items = summary
        .top_review_items
        .iter()
        .map(decision_summary_entry_fingerprint)
        .collect::<Vec<_>>()
        .join("|");
    let blocking_kind_counts = summary
        .blocking_kind_counts
        .iter()
        .map(|(kind, count)| format!("{kind}:{count}"))
        .collect::<Vec<_>>()
        .join("|");
    let review_required_kind_counts = summary
        .review_required_kind_counts
        .iter()
        .map(|(kind, count)| format!("{kind}:{count}"))
        .collect::<Vec<_>>()
        .join("|");
    fingerprint_text(
        &[
            summary.mode.clone(),
            summary.readiness_state.clone(),
            summary.reason.clone(),
            summary.blocking.to_string(),
            summary.review_required.to_string(),
            summary.blocking_semantic_conflict_count.to_string(),
            summary.review_required_semantic_conflict_count.to_string(),
            summary.high_risk_patch_operation_count.to_string(),
            summary.review_required_patch_operation_count.to_string(),
            summary.blocking_kinds.join("|"),
            summary.review_required_kinds.join("|"),
            blocking_kind_counts,
            review_required_kind_counts,
            summary
                .primary_blocker_kind
                .clone()
                .unwrap_or_default(),
            summary
                .primary_blocker_scope
                .clone()
                .unwrap_or_default(),
            summary
                .primary_blocker_target_id
                .clone()
                .unwrap_or_default(),
            summary
                .primary_blocker_summary
                .clone()
                .unwrap_or_default(),
            summary
                .primary_recommended_action
                .clone()
                .unwrap_or_default(),
            top_blockers,
            top_review_items,
            summary.gate_holds.join("|"),
            summary.recommended_actions.join("|"),
            summary.scoped_worktree_roles.join("|"),
            entries,
        ]
        .join(":"),
    )
}

fn package_agent_brief_fingerprint(brief: &AgentBrief) -> String {
    let top_blockers = brief
        .top_blockers
        .iter()
        .map(decision_summary_entry_fingerprint)
        .collect::<Vec<_>>()
        .join("|");
    let top_review_items = brief
        .top_review_items
        .iter()
        .map(decision_summary_entry_fingerprint)
        .collect::<Vec<_>>()
        .join("|");
    fingerprint_text(
        &[
            brief.project_name.clone(),
            brief.mode.clone(),
            brief.readiness_state.clone(),
            brief.reason.clone(),
            brief.current_stage_id.clone().unwrap_or_default(),
            brief.current_stage_name.clone().unwrap_or_default(),
            brief.total_stages.to_string(),
            brief.completed_stages.to_string(),
            brief.overall_progress_percent.to_string(),
            brief.blocking.to_string(),
            brief.review_required.to_string(),
            brief.primary_blocker_kind.clone().unwrap_or_default(),
            brief.primary_blocker_scope.clone().unwrap_or_default(),
            brief.primary_blocker_target_id.clone().unwrap_or_default(),
            brief.primary_blocker_summary.clone().unwrap_or_default(),
            brief.primary_recommended_action.clone().unwrap_or_default(),
            top_blockers,
            top_review_items,
            brief.scoped_worktree_roles.join("|"),
            brief.gate_holds.join("|"),
            brief.next_actions.join("|"),
        ]
        .join(":"),
    )
}

fn build_patch_plan(
    mode: AuthorMode,
    patch_operations: &[PatchOperation],
    base_source: &ParsedBlueprintSource,
    result_source: &ParsedBlueprintSource,
) -> PatchPlan {
    PatchPlan {
        schema_version: PATCH_PLAN_SCHEMA_VERSION.to_string(),
        mode: mode.as_str().to_string(),
        operation_count: patch_operations.len(),
        conflict_count: patch_operations
            .iter()
            .filter(|operation| operation.status == "retained-conflict")
            .count(),
        base_fingerprint: parsed_source_fingerprint(base_source),
        result_fingerprint: parsed_source_fingerprint(result_source),
        operations: patch_operations.to_vec(),
    }
}

fn build_patch_plan_from_change_report(change_report: &ChangeReport) -> PatchPlan {
    PatchPlan {
        schema_version: PATCH_PLAN_SCHEMA_VERSION.to_string(),
        mode: change_report.mode.clone(),
        operation_count: change_report.patch_operation_count,
        conflict_count: change_report
            .patch_operations
            .iter()
            .filter(|operation| operation.status == "retained-conflict")
            .count(),
        base_fingerprint: String::new(),
        result_fingerprint: String::new(),
        operations: change_report.patch_operations.clone(),
    }
}

fn build_patch_execution_report(
    mode: AuthorMode,
    patch_plan: &PatchPlan,
    base_source: &ParsedBlueprintSource,
    resolved_contract: &ResolvedContract,
    worktree_protocol: &WorktreeProtocol,
    stage_documents: &[StageDocument],
) -> Result<PatchExecutionReport, CoreError> {
    if !matches!(mode, AuthorMode::UpdateBlueprint) {
        return Ok(PatchExecutionReport {
            schema_version: PATCH_EXECUTION_REPORT_SCHEMA_VERSION.to_string(),
            mode: mode.as_str().to_string(),
            base_fingerprint: patch_plan.base_fingerprint.clone(),
            expected_result_fingerprint: patch_plan.result_fingerprint.clone(),
            replayed_result_fingerprint: patch_plan.result_fingerprint.clone(),
            replay_status: "not-applicable".to_string(),
            operation_count: patch_plan.operation_count,
            applied_operation_count: 0,
            mismatch_count: 0,
            mismatches: Vec::new(),
            reverse_replayed_base_fingerprint: patch_plan.base_fingerprint.clone(),
            reversibility_status: "not-applicable".to_string(),
            reverse_mismatch_count: 0,
            reverse_mismatches: Vec::new(),
            scope_validation_status: "not-applicable".to_string(),
            scope_mismatch_count: 0,
            scope_mismatches: Vec::new(),
        });
    }

    let scope_mismatches = collect_patch_operation_scope_mismatches(
        &patch_plan.operations,
        resolved_contract,
        worktree_protocol,
        stage_documents,
    )?;
    let replayed = apply_patch_plan_to_source_in_direction(
        base_source,
        patch_plan,
        PatchReplayDirection::Forward,
    )?;
    let replayed_result_fingerprint = parsed_source_fingerprint(&replayed);
    let mut mismatches = Vec::new();
    if patch_plan.result_fingerprint != replayed_result_fingerprint {
        mismatches.push(
            "replayed result fingerprint did not match patch plan result fingerprint".to_string(),
        );
    }
    let reversed = apply_patch_plan_to_source_in_direction(
        &replayed,
        patch_plan,
        PatchReplayDirection::Reverse,
    )?;
    let reverse_replayed_base_fingerprint = parsed_source_fingerprint(&reversed);
    let mut reverse_mismatches = Vec::new();
    if patch_plan.base_fingerprint != reverse_replayed_base_fingerprint {
        reverse_mismatches.push(
            "reverse replay did not return to the original patch base fingerprint".to_string(),
        );
    }
    Ok(PatchExecutionReport {
        schema_version: PATCH_EXECUTION_REPORT_SCHEMA_VERSION.to_string(),
        mode: mode.as_str().to_string(),
        base_fingerprint: patch_plan.base_fingerprint.clone(),
        expected_result_fingerprint: patch_plan.result_fingerprint.clone(),
        replayed_result_fingerprint,
        replay_status: if mismatches.is_empty() {
            "replayed".to_string()
        } else {
            "mismatch".to_string()
        },
        operation_count: patch_plan.operation_count,
        applied_operation_count: patch_plan.operations.len(),
        mismatch_count: mismatches.len(),
        mismatches,
        reverse_replayed_base_fingerprint,
        reversibility_status: if reverse_mismatches.is_empty() {
            "reversible".to_string()
        } else {
            "irreversible".to_string()
        },
        reverse_mismatch_count: reverse_mismatches.len(),
        reverse_mismatches,
        scope_validation_status: if scope_mismatches.is_empty() {
            "valid".to_string()
        } else {
            "mismatch".to_string()
        },
        scope_mismatch_count: scope_mismatches.len(),
        scope_mismatches,
    })
}

fn build_patch_execution_report_fallback(
    mode: AuthorMode,
    patch_plan: &PatchPlan,
    current_result_fingerprint: &str,
) -> PatchExecutionReport {
    PatchExecutionReport {
        schema_version: PATCH_EXECUTION_REPORT_SCHEMA_VERSION.to_string(),
        mode: mode.as_str().to_string(),
        base_fingerprint: if patch_plan.base_fingerprint.is_empty() {
            current_result_fingerprint.to_string()
        } else {
            patch_plan.base_fingerprint.clone()
        },
        expected_result_fingerprint: if patch_plan.result_fingerprint.is_empty() {
            current_result_fingerprint.to_string()
        } else {
            patch_plan.result_fingerprint.clone()
        },
        replayed_result_fingerprint: current_result_fingerprint.to_string(),
        replay_status: "legacy-derived".to_string(),
        operation_count: patch_plan.operation_count,
        applied_operation_count: patch_plan.operations.len(),
        mismatch_count: 0,
        mismatches: Vec::new(),
        reverse_replayed_base_fingerprint: if patch_plan.base_fingerprint.is_empty() {
            current_result_fingerprint.to_string()
        } else {
            patch_plan.base_fingerprint.clone()
        },
        reversibility_status: "legacy-derived".to_string(),
        reverse_mismatch_count: 0,
        reverse_mismatches: Vec::new(),
        scope_validation_status: if patch_plan.operations.is_empty() {
            "not-applicable".to_string()
        } else {
            "legacy-derived".to_string()
        },
        scope_mismatch_count: 0,
        scope_mismatches: Vec::new(),
    }
}

fn refresh_readiness_fingerprint(package: &mut BlueprintPackage) -> Result<(), CoreError> {
    let expected_manifest_entries = expected_manifest_entries(package)?;
    let readiness_summary =
        summarize_readiness_gate_holds(&package.normalization_report, &package.patch_plan);
    package.readiness.reason = readiness_reason(&readiness_summary);
    package.readiness.blocking_semantic_conflict_count =
        readiness_summary.blocking_semantic_conflict_count;
    package.readiness.review_required_semantic_conflict_count =
        readiness_summary.review_required_semantic_conflict_count;
    package.readiness.high_risk_patch_operation_count =
        readiness_summary.high_risk_patch_operation_count;
    package.readiness.review_required_patch_operation_count =
        readiness_summary.review_required_patch_operation_count;
    package.readiness.gate_holds = readiness_summary.gate_holds;
    package.readiness.recommended_actions = readiness_summary.recommended_actions;
    package.decision_summary = build_decision_summary(
        package.mode,
        &package.readiness,
        &package.normalization_report,
        &package.patch_plan,
        &package.resolved_contract,
        &package.worktree_protocol,
        &package.stage_documents,
    )?;
    package.agent_brief = build_agent_brief(
        &package.project_name,
        package.mode,
        &package.readiness,
        &package.task_progress,
        &package.decision_summary,
    );
    package.readiness.fingerprint = readiness_fingerprint_for(
        &package.project_name,
        package.mode,
        &package.source_provenance,
        &manifest_fingerprint_from_entries(&expected_manifest_entries),
        &package.resolved_contract,
        &package.worktree_protocol,
        &package.normalization_report,
        &package.change_report,
        &package.patch_plan,
        &package.patch_execution_report,
        &package.decision_summary,
        &package.agent_brief,
    );
    Ok(())
}

fn patch_operation_risk_level(operation: &PatchOperation) -> String {
    operation
        .strategy_metadata
        .get("risk_level")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            PatchStrategyKind::parse(&operation.strategy).map(|kind| kind.risk_level().to_string())
        })
        .unwrap_or_else(|| "low".to_string())
}

fn patch_operation_review_required(operation: &PatchOperation) -> bool {
    operation
        .strategy_metadata
        .get("review_required")
        .map(|value| value.trim().eq_ignore_ascii_case("true"))
        .unwrap_or_else(|| {
            PatchStrategyKind::parse(&operation.strategy)
                .map(|kind| kind.review_required())
                .unwrap_or(false)
        })
}

fn patch_operation_recommended_action(operation: &PatchOperation) -> String {
    format!(
        "review patch operation `{}` for {} `{}` before runtime handoff",
        operation.strategy, operation.scope, operation.target_id
    )
}

fn summarize_readiness_gate_holds(
    report: &NormalizationReport,
    patch_plan: &PatchPlan,
) -> ReadinessGateSummary {
    let blocking_semantic_conflict_count = report
        .semantic_conflicts
        .iter()
        .filter(|conflict| conflict.blocking)
        .count();
    let review_required_semantic_conflict_count = report
        .semantic_conflicts
        .iter()
        .filter(|conflict| conflict.review_required)
        .count();
    let high_risk_patch_operation_count = patch_plan
        .operations
        .iter()
        .filter(|operation| patch_operation_risk_level(operation) == "high")
        .count();
    let review_required_patch_operation_count = patch_plan
        .operations
        .iter()
        .filter(|operation| patch_operation_review_required(operation))
        .count();
    let mut gate_holds = report
        .semantic_conflicts
        .iter()
        .filter(|conflict| conflict.blocking)
        .map(|conflict| {
            format!(
                "semantic-conflict:{}:{}:{}",
                conflict.scope, conflict.canonical_section, conflict.severity
            )
        })
        .collect::<Vec<_>>();
    gate_holds.extend(
        patch_plan
            .operations
            .iter()
            .filter(|operation| patch_operation_risk_level(operation) == "high")
            .map(|operation| {
                format!(
                    "patch-risk:{}:{}:{}",
                    operation.scope, operation.target_id, operation.strategy
                )
            }),
    );
    gate_holds.sort();
    gate_holds.dedup();

    let mut recommended_actions = report
        .semantic_conflicts
        .iter()
        .filter(|conflict| conflict.review_required)
        .map(|conflict| conflict.recommended_action.clone())
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    recommended_actions.extend(
        patch_plan
            .operations
            .iter()
            .filter(|operation| patch_operation_review_required(operation))
            .map(patch_operation_recommended_action),
    );
    recommended_actions.sort();
    recommended_actions.dedup();

    ReadinessGateSummary {
        blocking_semantic_conflict_count,
        review_required_semantic_conflict_count,
        high_risk_patch_operation_count,
        review_required_patch_operation_count,
        gate_holds,
        recommended_actions,
    }
}

fn build_decision_summary(
    mode: AuthorMode,
    readiness: &ReadinessReport,
    normalization_report: &NormalizationReport,
    patch_plan: &PatchPlan,
    resolved_contract: &ResolvedContract,
    worktree_protocol: &WorktreeProtocol,
    stage_documents: &[StageDocument],
) -> Result<DecisionSummary, CoreError> {
    let mut entries = Vec::new();
    let mut scoped_worktree_roles = BTreeSet::new();

    for conflict in normalization_report
        .semantic_conflicts
        .iter()
        .filter(|conflict| conflict.blocking || conflict.review_required)
    {
        let worktree_roles = decision_scope_worktree_roles(
            &conflict.scope,
            resolved_contract,
            worktree_protocol,
            stage_documents,
        )?;
        for role in &worktree_roles {
            scoped_worktree_roles.insert(role.clone());
        }
        entries.push(DecisionSummaryEntry {
            kind: "semantic-conflict".to_string(),
            scope: conflict.scope.clone(),
            target_id: conflict.canonical_section.clone(),
            severity: conflict.severity.clone(),
            blocking: conflict.blocking,
            review_required: conflict.review_required,
            worktree_roles,
            summary: format!(
                "{} semantic conflict for `{}` in `{}`",
                conflict.severity, conflict.canonical_section, conflict.scope
            ),
            recommended_action: conflict.recommended_action.clone(),
        });
    }

    for operation in patch_plan.operations.iter().filter(|operation| {
        patch_operation_risk_level(operation) == "high" || patch_operation_review_required(operation)
    }) {
        let worktree_roles = normalize_unique_strings(&operation.target_worktree_roles);
        for role in &worktree_roles {
            scoped_worktree_roles.insert(role.clone());
        }
        let severity = patch_operation_risk_level(operation).to_string();
        entries.push(DecisionSummaryEntry {
            kind: "patch-risk".to_string(),
            scope: operation.scope.clone(),
            target_id: operation.target_id.clone(),
            severity: severity.clone(),
            blocking: severity == "high",
            review_required: patch_operation_review_required(operation),
            worktree_roles,
            summary: format!(
                "{} patch operation `{}` for `{}` in `{}`",
                severity, operation.strategy, operation.target_id, operation.scope
            ),
            recommended_action: patch_operation_recommended_action(operation),
        });
    }

    entries.sort_by(|left, right| {
        (
            left.kind.as_str(),
            left.scope.as_str(),
            left.target_id.as_str(),
            left.severity.as_str(),
        )
            .cmp(&(
                right.kind.as_str(),
                right.scope.as_str(),
                right.target_id.as_str(),
                right.severity.as_str(),
            ))
    });

    let mut blocking_kind_counts = BTreeMap::new();
    let mut review_required_kind_counts = BTreeMap::new();
    for entry in &entries {
        if entry.blocking {
            *blocking_kind_counts.entry(entry.kind.clone()).or_insert(0) += 1;
        }
        if entry.review_required {
            *review_required_kind_counts
                .entry(entry.kind.clone())
                .or_insert(0) += 1;
        }
    }
    let blocking_kinds = blocking_kind_counts.keys().cloned().collect::<Vec<_>>();
    let review_required_kinds = review_required_kind_counts
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let primary_blocker = primary_decision_summary_blocker(&entries);
    let top_blockers = top_decision_summary_entries(&entries, |entry| entry.blocking);
    let top_review_items =
        top_decision_summary_entries(&entries, |entry| entry.review_required && !entry.blocking);

    Ok(DecisionSummary {
        schema_version: DECISION_SUMMARY_SCHEMA_VERSION.to_string(),
        mode: mode.as_str().to_string(),
        readiness_state: readiness.state.clone(),
        reason: readiness.reason.clone(),
        blocking: readiness.blocking_semantic_conflict_count > 0
            || readiness.high_risk_patch_operation_count > 0
            || !readiness.gate_holds.is_empty(),
        review_required: readiness.review_required_semantic_conflict_count > 0
            || readiness.review_required_patch_operation_count > 0,
        blocking_semantic_conflict_count: readiness.blocking_semantic_conflict_count,
        review_required_semantic_conflict_count: readiness
            .review_required_semantic_conflict_count,
        high_risk_patch_operation_count: readiness.high_risk_patch_operation_count,
        review_required_patch_operation_count: readiness.review_required_patch_operation_count,
        gate_hold_count: readiness.gate_holds.len(),
        recommended_action_count: readiness.recommended_actions.len(),
        blocking_kinds,
        review_required_kinds,
        blocking_kind_counts,
        review_required_kind_counts,
        primary_blocker_kind: primary_blocker.map(|entry| entry.kind.clone()),
        primary_blocker_scope: primary_blocker.map(|entry| entry.scope.clone()),
        primary_blocker_target_id: primary_blocker.map(|entry| entry.target_id.clone()),
        primary_blocker_summary: primary_blocker.map(|entry| entry.summary.clone()),
        primary_recommended_action: primary_blocker
            .map(|entry| entry.recommended_action.clone())
            .or_else(|| readiness.recommended_actions.first().cloned()),
        top_blockers,
        top_review_items,
        scoped_worktree_roles: scoped_worktree_roles.into_iter().collect(),
        entries,
        gate_holds: readiness.gate_holds.clone(),
        recommended_actions: readiness.recommended_actions.clone(),
    })
}

fn decision_summary_entry_fingerprint(entry: &DecisionSummaryEntry) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}",
        entry.kind,
        entry.scope,
        entry.target_id,
        entry.severity,
        entry.blocking,
        entry.review_required,
        entry.worktree_roles.join("\u{1d}"),
        entry.recommended_action
    )
}

fn primary_decision_summary_blocker(entries: &[DecisionSummaryEntry]) -> Option<&DecisionSummaryEntry> {
    entries
        .iter()
        .filter(|entry| entry.blocking)
        .min_by(|left, right| {
            (
                decision_entry_kind_rank(&left.kind),
                decision_entry_severity_rank(&left.severity),
                left.scope.as_str(),
                left.target_id.as_str(),
                left.summary.as_str(),
            )
                .cmp(&(
                    decision_entry_kind_rank(&right.kind),
                    decision_entry_severity_rank(&right.severity),
                    right.scope.as_str(),
                    right.target_id.as_str(),
                    right.summary.as_str(),
                ))
        })
}

fn top_decision_summary_entries<F>(
    entries: &[DecisionSummaryEntry],
    predicate: F,
) -> Vec<DecisionSummaryEntry>
where
    F: Fn(&DecisionSummaryEntry) -> bool,
{
    let mut ranked = entries
        .iter()
        .filter(|entry| predicate(entry))
        .cloned()
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        (
            decision_entry_kind_rank(&left.kind),
            decision_entry_severity_rank(&left.severity),
            left.scope.as_str(),
            left.target_id.as_str(),
            left.summary.as_str(),
        )
            .cmp(&(
                decision_entry_kind_rank(&right.kind),
                decision_entry_severity_rank(&right.severity),
                right.scope.as_str(),
                right.target_id.as_str(),
                right.summary.as_str(),
            ))
    });
    ranked.truncate(3);
    ranked
}

fn decision_entry_kind_rank(kind: &str) -> u8 {
    match kind {
        "semantic-conflict" => 0,
        "patch-risk" => 1,
        _ => 2,
    }
}

fn decision_entry_severity_rank(severity: &str) -> u8 {
    match severity.to_ascii_lowercase().as_str() {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

fn readiness_reason(summary: &ReadinessGateSummary) -> String {
    if summary.blocking_semantic_conflict_count > 0 && summary.high_risk_patch_operation_count > 0
    {
        "Authoring completed with blocking semantic conflicts and high-risk patch operations; package is ready for aegiscore-rust-runtime blueprint gate review."
            .to_string()
    } else if summary.blocking_semantic_conflict_count > 0 {
        "Authoring completed with blocking semantic conflicts; package is ready for aegiscore-rust-runtime blueprint gate review."
            .to_string()
    } else if summary.high_risk_patch_operation_count > 0 {
        "Authoring completed with high-risk patch operations; package is ready for aegiscore-rust-runtime blueprint gate review."
            .to_string()
    } else if summary.review_required_semantic_conflict_count > 0
        || summary.review_required_patch_operation_count > 0
    {
        "Authoring completed with review-required semantic conflicts; package is ready for aegiscore-rust-runtime blueprint gate review."
            .to_string()
    } else {
        "Authoring completed and package is ready for aegiscore-rust-runtime blueprint gate."
            .to_string()
    }
}

fn validate_readiness_alignment(
    readiness: &ReadinessReport,
    normalization_report: &NormalizationReport,
    patch_plan: &PatchPlan,
) -> Result<(), CoreError> {
    let expected = summarize_readiness_gate_holds(normalization_report, patch_plan);
    if readiness.blocking_semantic_conflict_count
        != expected.blocking_semantic_conflict_count
    {
        return Err(CoreError::ReadinessInconsistent {
            field: "blocking_semantic_conflict_count".to_string(),
        });
    }
    if readiness.review_required_semantic_conflict_count
        != expected.review_required_semantic_conflict_count
    {
        return Err(CoreError::ReadinessInconsistent {
            field: "review_required_semantic_conflict_count".to_string(),
        });
    }
    if readiness.high_risk_patch_operation_count != expected.high_risk_patch_operation_count {
        return Err(CoreError::ReadinessInconsistent {
            field: "high_risk_patch_operation_count".to_string(),
        });
    }
    if readiness.review_required_patch_operation_count
        != expected.review_required_patch_operation_count
    {
        return Err(CoreError::ReadinessInconsistent {
            field: "review_required_patch_operation_count".to_string(),
        });
    }
    if readiness.gate_holds != expected.gate_holds {
        return Err(CoreError::ReadinessInconsistent {
            field: "gate_holds".to_string(),
        });
    }
    if readiness.recommended_actions != expected.recommended_actions {
        return Err(CoreError::ReadinessInconsistent {
            field: "recommended_actions".to_string(),
        });
    }
    if readiness.reason != readiness_reason(&expected) {
        return Err(CoreError::ReadinessInconsistent {
            field: "reason".to_string(),
        });
    }
    Ok(())
}

fn validate_decision_summary_alignment(
    decision_summary: &DecisionSummary,
    mode: AuthorMode,
    readiness: &ReadinessReport,
    normalization_report: &NormalizationReport,
    patch_plan: &PatchPlan,
    resolved_contract: &ResolvedContract,
    worktree_protocol: &WorktreeProtocol,
    stage_documents: &[StageDocument],
) -> Result<(), CoreError> {
    let expected = build_decision_summary(
        mode,
        readiness,
        normalization_report,
        patch_plan,
        resolved_contract,
        worktree_protocol,
        stage_documents,
    )?;
    if decision_summary != &expected {
        return Err(CoreError::ReadinessInconsistent {
            field: "decision_summary".to_string(),
        });
    }
    Ok(())
}

fn validate_agent_brief_alignment(
    agent_brief: &AgentBrief,
    project_name: &str,
    mode: AuthorMode,
    readiness: &ReadinessReport,
    task_progress: &TaskProgressReport,
    decision_summary: &DecisionSummary,
) -> Result<(), CoreError> {
    let expected = build_agent_brief(
        project_name,
        mode,
        readiness,
        task_progress,
        decision_summary,
    );
    if agent_brief != &expected {
        return Err(CoreError::ReadinessInconsistent {
            field: "agent_brief".to_string(),
        });
    }
    Ok(())
}

fn readiness_fingerprint_for(
    project_name: &str,
    mode: AuthorMode,
    source_provenance: &str,
    manifest_fingerprint: &str,
    contract: &ResolvedContract,
    worktree_protocol: &WorktreeProtocol,
    report: &NormalizationReport,
    change_report: &ChangeReport,
    patch_plan: &PatchPlan,
    patch_execution_report: &PatchExecutionReport,
    decision_summary: &DecisionSummary,
    agent_brief: &AgentBrief,
) -> String {
    let readiness_summary = summarize_readiness_gate_holds(report, patch_plan);
    fingerprint_text(&format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        project_name,
        mode.as_str(),
        source_provenance,
        manifest_fingerprint,
        package_contract_fingerprint(contract),
        package_worktree_protocol_fingerprint(worktree_protocol),
        package_normalization_fingerprint(report),
        package_change_report_fingerprint(change_report),
        package_patch_plan_fingerprint(patch_plan),
        package_patch_execution_report_fingerprint(patch_execution_report),
        package_decision_summary_fingerprint(decision_summary),
        package_agent_brief_fingerprint(agent_brief),
        readiness_summary.blocking_semantic_conflict_count,
        readiness_summary.review_required_semantic_conflict_count,
        readiness_summary.high_risk_patch_operation_count,
        readiness_summary.review_required_patch_operation_count,
        readiness_summary.gate_holds.join("|"),
        readiness_summary.recommended_actions.join("|")
    ))
}

fn expected_manifest_entries(
    package: &BlueprintPackage,
) -> Result<Vec<BlueprintManifestEntry>, CoreError> {
    let module_catalog_json = to_pretty_json(&package.module_catalog)?;
    let mut entries = vec![
        manifest_entry(
            &format!(
                "{}/00-authority-root.md",
                package.resolved_contract.paths.authority_root
            ),
            "authority",
            "authority-root",
            &package.source_provenance,
            &package.authority_doc,
        ),
        manifest_entry(
            &format!(
                "{}/00-workflow-overview.md",
                package.resolved_contract.paths.workflow_root
            ),
            "workflow",
            "workflow-overview",
            &package.source_provenance,
            &package.workflow_doc,
        ),
        manifest_entry(
            &format!(
                "{}/01-language-policy.md",
                package.resolved_contract.paths.modules_root
            ),
            "module-policy",
            "module-language-policy",
            &package.source_provenance,
            &package.module_policy_doc,
        ),
        manifest_entry(
            &format!(
                "{}/00-module-catalog.json",
                package.resolved_contract.paths.modules_root
            ),
            "module-catalog",
            "module-catalog",
            &package.source_provenance,
            &module_catalog_json,
        ),
    ];
    for stage_document in &package.stage_documents {
        entries.push(manifest_entry(
            &stage_document.path,
            "stage",
            &stage_document.stage_id,
            &package.source_provenance,
            &stage_document.content,
        ));
    }
    Ok(entries)
}

fn validate_module_catalog(
    catalog: &ModuleCatalog,
    policy: &ModuleLanguagePolicyToml,
    worktree_protocol: Option<&WorktreeProtocol>,
) -> Result<(), CoreError> {
    let mut expected_modules = default_modules(policy)?;
    if let Some(protocol) = worktree_protocol {
        apply_worktree_bindings_to_modules(&mut expected_modules, protocol);
    }
    let expected_by_id = expected_modules
        .iter()
        .map(|module| (module.module_id.clone(), module))
        .collect::<BTreeMap<_, _>>();

    for module in &catalog.modules {
        if !module
            .allowed_languages
            .contains(&module.recommended_language)
        {
            return Err(CoreError::InvalidModuleLanguage {
                module_id: module.module_id.clone(),
            });
        }
        if module
            .forbidden_languages
            .contains(&module.recommended_language)
        {
            return Err(CoreError::InvalidModuleLanguage {
                module_id: module.module_id.clone(),
            });
        }

        let Some(rule) = policy.rules.iter().find(|rule| rule.layer == module.layer) else {
            return Err(CoreError::InvalidModuleLanguage {
                module_id: module.module_id.clone(),
            });
        };

        if module.recommended_language != rule.recommended_language
            || module.allowed_languages != rule.allowed_languages
            || module.forbidden_languages != rule.forbidden_languages
        {
            return Err(CoreError::InvalidModuleLanguage {
                module_id: module.module_id.clone(),
            });
        }

        let Some(expected_module) = expected_by_id.get(&module.module_id) else {
            return Err(CoreError::ModuleCatalogMismatch {
                module_id: module.module_id.clone(),
            });
        };

        if module != *expected_module {
            return Err(CoreError::ModuleCatalogMismatch {
                module_id: module.module_id.clone(),
            });
        }
    }

    for expected_module in &expected_modules {
        if !catalog
            .modules
            .iter()
            .any(|module| module.module_id == expected_module.module_id)
        {
            return Err(CoreError::ModuleCatalogMismatch {
                module_id: expected_module.module_id.clone(),
            });
        }
    }

    Ok(())
}

fn apply_worktree_bindings_to_module_catalog(
    catalog: &mut ModuleCatalog,
    protocol: &WorktreeProtocol,
) {
    apply_worktree_bindings_to_modules(&mut catalog.modules, protocol);
}

fn apply_worktree_bindings_to_modules(
    modules: &mut [ModuleSpec],
    protocol: &WorktreeProtocol,
) {
    let mut module_to_roles = BTreeMap::<String, Vec<String>>::new();
    for role in &protocol.roles {
        for module_id in &role.module_ids {
            module_to_roles
                .entry(module_id.clone())
                .or_default()
                .push(role.role_id.clone());
        }
    }

    for module in modules {
        let Some(role_ids) = module_to_roles.get(&module.module_id) else {
            module.preferred_worktree_role = None;
            module.allowed_worktree_roles.clear();
            continue;
        };
        let mut unique_roles = role_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        unique_roles.sort();
        module.allowed_worktree_roles = unique_roles.clone();
        module.preferred_worktree_role = if unique_roles.len() == 1 {
            unique_roles.first().cloned()
        } else {
            None
        };
    }
}

fn module_catalog_needs_worktree_bindings(
    catalog: &ModuleCatalog,
    protocol: &WorktreeProtocol,
) -> bool {
    let protocol_has_module_bindings = protocol.roles.iter().any(|role| !role.module_ids.is_empty());
    protocol_has_module_bindings
        && catalog.modules.iter().all(|module| {
            module.preferred_worktree_role.is_none() && module.allowed_worktree_roles.is_empty()
        })
}

fn section_list(sections: &BTreeMap<String, Vec<String>>, key: &str) -> String {
    sections
        .get(key)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|line| format!("- {}", line.trim_start_matches("- ").trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn section_path_list(sections: &BTreeMap<String, Vec<String>>, key: &str) -> String {
    sections
        .get(key)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|line| format!("- `{}`", line.trim_start_matches("- ").trim().trim_matches('`')))
        .collect::<Vec<_>>()
        .join("\n")
}

fn section_paragraph(
    sections: &BTreeMap<String, Vec<String>>,
    key: &str,
    fallback: &str,
) -> String {
    sections
        .get(key)
        .map(|lines| lines.join("\n"))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn sanitize_line(line: &str) -> String {
    line.trim()
        .trim_start_matches("- ")
        .trim_start_matches("* ")
        .trim_start_matches(|ch: char| ch.is_ascii_digit() || ch == '.' || ch == ')' || ch == ' ')
        .trim()
        .to_string()
}

fn list_inline(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn default_modules(policy: &ModuleLanguagePolicyToml) -> Result<Vec<ModuleSpec>, CoreError> {
    let mut modules = vec![
        ModuleSpec {
            module_id: "ara-schemas".to_string(),
            layer: "schemas".to_string(),
            responsibility: "Define shared schema structs, enums, schema versions, and machine-readable outputs.".to_string(),
            recommended_language: ModuleLanguage::Rust,
            allowed_languages: vec![ModuleLanguage::Rust],
            forbidden_languages: vec![ModuleLanguage::Powershell, ModuleLanguage::Sh, ModuleLanguage::Bat],
            reason: "Schemas are part of the runtime truth surface and must remain strongly typed.".to_string(),
            hot_path: true,
            cross_platform_requirement: "required".to_string(),
            boundary_type: "library".to_string(),
            owned_artifacts: vec!["resolved-contract.json".to_string()],
            preferred_worktree_role: None,
            allowed_worktree_roles: Vec::new(),
        },
        ModuleSpec {
            module_id: "ara-core".to_string(),
            layer: "core".to_string(),
            responsibility: "Normalize external blueprints, generate packages, and compile resolved contracts.".to_string(),
            recommended_language: ModuleLanguage::Rust,
            allowed_languages: vec![ModuleLanguage::Rust],
            forbidden_languages: vec![ModuleLanguage::Powershell, ModuleLanguage::Sh, ModuleLanguage::Bat],
            reason: "Core authoring logic must stay deterministic and portable.".to_string(),
            hot_path: true,
            cross_platform_requirement: "required".to_string(),
            boundary_type: "library".to_string(),
            owned_artifacts: vec![".codex/auto-dev/project-contract.toml".to_string(), ".codex/auto-dev/resolved-contract.json".to_string()],
            preferred_worktree_role: None,
            allowed_worktree_roles: Vec::new(),
        },
        ModuleSpec {
            module_id: "ara-runtime".to_string(),
            layer: "runtime".to_string(),
            responsibility: "Provide path normalization, atomic writes, and fingerprint helpers.".to_string(),
            recommended_language: ModuleLanguage::Rust,
            allowed_languages: vec![ModuleLanguage::Rust],
            forbidden_languages: vec![ModuleLanguage::Powershell, ModuleLanguage::Sh, ModuleLanguage::Bat],
            reason: "Runtime guarantees require stable file handling across platforms.".to_string(),
            hot_path: true,
            cross_platform_requirement: "required".to_string(),
            boundary_type: "library".to_string(),
            owned_artifacts: vec![".codex/auto-dev/blueprint-manifest.json".to_string()],
            preferred_worktree_role: None,
            allowed_worktree_roles: Vec::new(),
        },
        ModuleSpec {
            module_id: "ara-cli".to_string(),
            layer: "core".to_string(),
            responsibility: "Expose the official command surface for authoring and emission.".to_string(),
            recommended_language: ModuleLanguage::Rust,
            allowed_languages: vec![ModuleLanguage::Rust],
            forbidden_languages: vec![ModuleLanguage::Powershell, ModuleLanguage::Sh, ModuleLanguage::Bat],
            reason: "The CLI is the only official entry point and must share the same typed runtime.".to_string(),
            hot_path: false,
            cross_platform_requirement: "required".to_string(),
            boundary_type: "cli".to_string(),
            owned_artifacts: vec![],
            preferred_worktree_role: None,
            allowed_worktree_roles: Vec::new(),
        },
        ModuleSpec {
            module_id: "ara-host-api".to_string(),
            layer: "host-api".to_string(),
            responsibility: "Expose authoring entry points to other Rust hosts without shell wrappers.".to_string(),
            recommended_language: ModuleLanguage::Rust,
            allowed_languages: vec![ModuleLanguage::Rust],
            forbidden_languages: vec![ModuleLanguage::Powershell, ModuleLanguage::Sh, ModuleLanguage::Bat],
            reason: "Embedded hosts should depend on typed Rust APIs instead of shell contracts.".to_string(),
            hot_path: false,
            cross_platform_requirement: "required".to_string(),
            boundary_type: "library".to_string(),
            owned_artifacts: vec![],
            preferred_worktree_role: None,
            allowed_worktree_roles: Vec::new(),
        },
        ModuleSpec {
            module_id: "wrapper-windows".to_string(),
            layer: "wrapper-windows".to_string(),
            responsibility: "Provide a convenience launcher for Windows users.".to_string(),
            recommended_language: ModuleLanguage::Powershell,
            allowed_languages: vec![ModuleLanguage::Powershell, ModuleLanguage::Bat],
            forbidden_languages: vec![ModuleLanguage::Rust],
            reason: "Windows convenience wrappers should remain thin and avoid business logic.".to_string(),
            hot_path: false,
            cross_platform_requirement: "optional".to_string(),
            boundary_type: "wrapper".to_string(),
            owned_artifacts: vec!["wrappers/ara.ps1".to_string(), "wrappers/ara.bat".to_string()],
            preferred_worktree_role: None,
            allowed_worktree_roles: Vec::new(),
        },
        ModuleSpec {
            module_id: "wrapper-unix".to_string(),
            layer: "wrapper-unix".to_string(),
            responsibility: "Provide a convenience launcher for POSIX hosts.".to_string(),
            recommended_language: ModuleLanguage::Sh,
            allowed_languages: vec![ModuleLanguage::Sh],
            forbidden_languages: vec![ModuleLanguage::Rust],
            reason: "POSIX wrappers should remain minimal and forward only to the Rust CLI.".to_string(),
            hot_path: false,
            cross_platform_requirement: "optional".to_string(),
            boundary_type: "wrapper".to_string(),
            owned_artifacts: vec!["wrappers/ara.sh".to_string()],
            preferred_worktree_role: None,
            allowed_worktree_roles: Vec::new(),
        },
    ];

    for module in &mut modules {
        apply_module_language_policy(module, policy)?;
    }

    Ok(modules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn import_mode_preserves_recognized_sections() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n## Allowed Scope\n\n- Blueprint docs\n- Contract outputs\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/demo.md".to_string()),
            },
        )
        .expect("package should build");

        assert!(package
            .normalization_report
            .preserved_sections
            .contains(&"purpose".to_string()));
        assert!(package
            .normalization_report
            .preserved_sections
            .contains(&"stage_order".to_string()));
        assert!(package
            .normalization_report
            .source_files
            .contains(&"imports/demo.md".to_string()));
        assert!(package.normalization_report.dropped_sections.is_empty());
        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert_eq!(package.stage_documents.len(), 2);
        assert_eq!(
            package.stage_documents[1].path,
            "blueprint/stages/02-hardening.md"
        );
        assert!(package
            .authority_doc
            .contains("Build a staged automation tool."));
    }

    #[test]
    fn import_mode_preserves_cross_stage_split_and_stop_conditions() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Stage Order\n\n- stage-01: foundation\n\n## Cross-Stage Split Rule\n\n- Keep authoring isolated from implementation.\n\n## Stop Conditions\n\n- Contract mismatch\n- Authority drift\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/workflow-rules.md".to_string()),
            },
        )
        .expect("package should build");

        assert!(package
            .workflow_doc
            .contains("Keep authoring isolated from implementation."));
        assert!(package.workflow_doc.contains("Contract mismatch"));
        assert!(package.workflow_doc.contains("Authority drift"));
        assert!(!package
            .normalization_report
            .dropped_sections
            .contains(&"Cross-Stage Split Rule".to_string()));
        assert!(!package
            .normalization_report
            .dropped_sections
            .contains(&"Stop Conditions".to_string()));
    }

    #[test]
    fn import_mode_reports_dropped_unknown_sections() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Surprise Section\n\n- Unexpected material\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/unknown-section.md".to_string()),
            },
        )
        .expect("package should build");

        assert!(package
            .normalization_report
            .dropped_sections
            .contains(&"Surprise Section".to_string()));
    }

    #[test]
    fn import_mode_accepts_json_blueprint_sources() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "json blueprint".to_string(),
                source_text: Some(
                    r#"{
  "purpose": "Build a staged automation tool.",
  "stages": [
    {
      "stage_id": "stage-01",
      "stage_name": "foundation",
      "deliverables": ["Foundation bundle"],
      "required_verification": ["Foundation validation"]
    },
    {
      "stage_id": "stage-02",
      "stage_name": "hardening",
      "deliverables": ["Hardening bundle"],
      "required_verification": ["Hardening validation"]
    }
  ],
  "deliverables": ["Blueprint package", "Contract bundle"]
}"#
                    .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/blueprint.json".to_string()),
            },
        )
        .expect("json package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert!(package
            .authority_doc
            .contains("Build a staged automation tool."));
        assert!(package
            .stage_documents
            .iter()
            .any(|doc| doc.path.ends_with("02-hardening.md")));
        let foundation_doc = package
            .stage_documents
            .iter()
            .find(|doc| doc.stage_id == "stage-01")
            .expect("foundation stage doc should exist");
        let hardening_doc = package
            .stage_documents
            .iter()
            .find(|doc| doc.stage_id == "stage-02")
            .expect("hardening stage doc should exist");
        assert!(foundation_doc.content.contains("Foundation bundle"));
        assert!(foundation_doc.content.contains("Foundation validation"));
        assert!(hardening_doc.content.contains("Hardening bundle"));
        assert!(hardening_doc.content.contains("Hardening validation"));
    }

    #[test]
    fn import_mode_accepts_toml_blueprint_sources() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "toml blueprint".to_string(),
                source_text: Some(
                    r#"purpose = "Build a staged automation tool."
deliverables = ["Blueprint package", "Contract bundle"]

[[stage_order]]
stage_id = "stage-01"
stage_name = "foundation"

[[stage_order]]
stage_id = "stage-02"
stage_name = "hardening"
"#
                    .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/blueprint.toml".to_string()),
            },
        )
        .expect("toml package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert!(package
            .authority_doc
            .contains("Build a staged automation tool."));
        assert!(package
            .stage_documents
            .iter()
            .any(|doc| doc.path.ends_with("02-hardening.md")));
    }

    #[test]
    fn import_mode_maps_semantic_aliases_and_phase_collections() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "semantic alias blueprint".to_string(),
                source_text: Some(
                    r#"{
  "purpose": "Normalize richer external blueprint semantics.",
  "constraints": ["Contracts must stay deterministic."],
  "acceptance_criteria": ["Emit a validated contract bundle."],
  "assumptions": ["External blueprints may omit some defaults."],
  "out_of_scope": ["Target project feature code."],
  "phases": [
    {
      "stage_id": "stage-01",
      "stage_name": "foundation",
      "deliverables": ["Foundation bundle"]
    },
    {
      "stage_id": "stage-02",
      "stage_name": "hardening",
      "required_verification": ["Hardening review"]
    }
  ]
}"#
                    .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/semantic-blueprint.json".to_string()),
            },
        )
        .expect("semantic alias package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert!(package
            .normalization_report
            .semantic_hints
            .iter()
            .any(|hint| hint.contains("Acceptance Criteria")));
        assert!(package
            .normalization_report
            .semantic_hints
            .iter()
            .any(|hint| hint.contains("phase or milestone")));
        assert!(package.workflow_doc.contains("stage-01"));
        assert!(package.stage_documents[0]
            .content
            .contains("Foundation bundle"));
        assert!(package.stage_documents[1]
            .content
            .contains("Hardening review"));
    }

    #[test]
    fn import_mode_rejects_stage_order_without_explicit_stage_ids() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Stage Order\n\n- foundation\n- hardening\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/ambiguous-stage-order.md".to_string()),
            },
        )
        .expect_err("package should fail without deterministic stage ids");

        assert!(matches!(error, CoreError::TooAmbiguousToNormalize { .. }));
    }

    #[test]
    fn import_mode_rejects_unsupported_source_format() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged tool.\n".to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/unsupported.yaml".to_string()),
            },
        )
        .expect_err("package should fail for unsupported source format");

        assert!(matches!(
            error,
            CoreError::UnsupportedImportSourceFormat { .. }
        ));
    }

    #[test]
    fn validate_rejects_missing_manifest_entry() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package
            .manifest
            .files
            .retain(|entry| entry.path != "blueprint/modules/00-module-catalog.json");

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(error, CoreError::MissingManifestEntry { .. }));
    }

    #[test]
    fn validate_rejects_unexpected_manifest_entry() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.manifest.files.push(BlueprintManifestEntry {
            path: "blueprint/stages/99-unexpected.md".to_string(),
            doc_role: "stage".to_string(),
            canonical_id: "stage-99".to_string(),
            source_provenance: package.source_provenance.clone(),
            fingerprint: "unexpected".to_string(),
        });

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(error, CoreError::UnexpectedManifestEntry { .. }));
    }

    #[test]
    fn validate_rejects_tampered_manifest_fingerprint() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        let authority_entry = package
            .manifest
            .files
            .iter_mut()
            .find(|entry| entry.path == "blueprint/authority/00-authority-root.md")
            .expect("authority entry should exist");
        authority_entry.fingerprint = "tampered".to_string();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::ManifestFingerprintMismatch { .. }
        ));
    }

    #[test]
    fn validate_rejects_tampered_readiness_fingerprint() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.readiness.fingerprint = "tampered".to_string();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(error, CoreError::ReadinessFingerprintMismatch));
    }

    #[test]
    fn validate_rejects_schema_version_mismatch() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.resolved_contract.schema_version = "ara.resolved-contract.v999".to_string();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(error, CoreError::SchemaVersionMismatch { .. }));
    }

    #[test]
    fn migrate_workspace_updates_drifted_schema_versions_and_writes_report() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-core-migrate-{unique}"));

        let package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        emit_package(&workspace, &package).expect("emit should succeed");

        let contract_root = default_contract_root().expect("contract root should resolve");
        let resolved_contract_path =
            workspace.join(format!("{contract_root}/resolved-contract.json"));
        let mut resolved_contract: serde_json::Value = serde_json::from_str(
            &read_utf8(&resolved_contract_path).expect("read resolved contract"),
        )
        .expect("parse resolved contract");
        resolved_contract["schema_version"] =
            serde_json::Value::String("ara.resolved-contract.v0".to_string());
        write_utf8_atomic(
            &resolved_contract_path,
            &(serde_json::to_string_pretty(&resolved_contract)
                .expect("serialize resolved contract")
                + "\n"),
        )
        .expect("write drifted resolved contract");

        let readiness_path = workspace.join(format!("{contract_root}/readiness.json"));
        let mut readiness: serde_json::Value =
            serde_json::from_str(&read_utf8(&readiness_path).expect("read readiness"))
                .expect("parse readiness");
        readiness["schema_version"] = serde_json::Value::String("ara.readiness.v0".to_string());
        write_utf8_atomic(
            &readiness_path,
            &(serde_json::to_string_pretty(&readiness).expect("serialize readiness") + "\n"),
        )
        .expect("write drifted readiness");

        let project_contract_path =
            workspace.join(format!("{contract_root}/project-contract.toml"));
        let drifted_project_contract = read_utf8(&project_contract_path)
            .expect("read project contract")
            .replace("ara.project-contract.v1", "ara.project-contract.v0");
        write_utf8_atomic(&project_contract_path, &(drifted_project_contract + "\n"))
            .expect("write drifted project contract");

        let semantic_ir_path = workspace.join(format!("{contract_root}/semantic-ir.json"));
        let mut semantic_ir: serde_json::Value =
            serde_json::from_str(&read_utf8(&semantic_ir_path).expect("read semantic ir"))
                .expect("parse semantic ir");
        semantic_ir["schema_version"] = serde_json::Value::String("ara.semantic-ir.v2".to_string());
        semantic_ir["source_fingerprint"] = serde_json::Value::String(String::new());
        write_utf8_atomic(
            &semantic_ir_path,
            &(serde_json::to_string_pretty(&semantic_ir).expect("serialize semantic ir") + "\n"),
        )
        .expect("write drifted semantic ir");

        let report = migrate_workspace_package(&workspace).expect("migration should succeed");
        assert!(report.migrated_artifacts >= 4);
        assert!(report
            .artifacts
            .iter()
            .any(|artifact| artifact.artifact == "resolved-contract"
                && artifact.action == "migrated"));

        let migrated =
            validate_workspace_package(&workspace).expect("migrated workspace should validate");
        assert_eq!(
            migrated.resolved_contract.schema_version,
            RESOLVED_CONTRACT_SCHEMA_VERSION
        );
        assert_eq!(migrated.readiness.schema_version, READINESS_SCHEMA_VERSION);
        assert_eq!(
            migrated.semantic_ir.schema_version,
            SEMANTIC_IR_SCHEMA_VERSION
        );
        assert!(!migrated.semantic_ir.source_fingerprint.is_empty());

        let migration_report_path =
            workspace.join(format!("{contract_root}/migration-report.json"));
        let migration_report: MigrationReport = serde_json::from_str(
            &read_utf8(&migration_report_path).expect("read migration report"),
        )
        .expect("parse migration report");
        assert_eq!(
            migration_report.schema_version,
            MIGRATION_REPORT_SCHEMA_VERSION
        );
        assert_eq!(migration_report.mode, "migrate-workspace");

        if workspace.exists() {
            fs::remove_dir_all(workspace).expect("cleanup temp workspace");
        }
    }

    #[test]
    fn validate_rejects_contract_toml_that_disagrees_with_resolved_contract() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.project_contract_toml = package
            .project_contract_toml
            .replace("stage-01", "stage-99");

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn build_package_uses_embedded_project_contract_defaults() {
        let package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");

        assert_eq!(package.resolved_contract.workflow_mode, "staged");
        assert_eq!(
            package.resolved_contract.paths.contract_root,
            ".codex/auto-dev"
        );
        assert!(package
            .project_contract_toml
            .contains("schema_version = \"ara.project-contract.v1\""));
    }

    #[test]
    fn build_package_uses_embedded_module_language_policy() {
        let package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");

        let wrapper_windows = package
            .module_catalog
            .modules
            .iter()
            .find(|module| module.module_id == "wrapper-windows")
            .expect("wrapper-windows module should exist");

        assert_eq!(
            wrapper_windows.recommended_language,
            ModuleLanguage::Powershell
        );
        assert!(wrapper_windows
            .allowed_languages
            .contains(&ModuleLanguage::Bat));
        assert!(wrapper_windows
            .forbidden_languages
            .contains(&ModuleLanguage::Python));
        assert!(wrapper_windows
            .forbidden_languages
            .contains(&ModuleLanguage::Nodejs));
    }

    #[test]
    fn build_package_emits_source_first_semantic_ir() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Deliverables\n\n- Foundation bundle\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/demo.md".to_string()),
            },
        )
        .expect("package should build");

        let rendered_source = parse_workspace_source(&workspace_bundle_text_from_docs(
            &format!(
                "{}/00-authority-root.md",
                package.resolved_contract.paths.authority_root
            ),
            &package.authority_doc,
            &format!(
                "{}/00-workflow-overview.md",
                package.resolved_contract.paths.workflow_root
            ),
            &package.workflow_doc,
            &package.stage_documents,
        ));

        assert_eq!(
            package.semantic_ir.derivation,
            "source-first-normalized-projection"
        );
        assert_eq!(
            package.semantic_ir.source_fingerprint,
            package.patch_plan.result_fingerprint
        );
        assert_eq!(
            package.semantic_ir.projection_fingerprint,
            parsed_source_fingerprint(&rendered_source)
        );
        assert_eq!(
            package.semantic_ir.normalized_sections.get("deliverables"),
            Some(&vec!["Foundation bundle".to_string()])
        );
        assert_eq!(
            package
                .semantic_ir
                .normalized_section_origins
                .get("deliverables")
                .map(String::as_str),
            Some("preserved")
        );
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "package"
                && frame.canonical_section == "purpose"
                && frame.origin_kind == "heading"
        }));
        assert!(package.semantic_ir.semantic_clusters.iter().any(|cluster| {
            cluster.scope == "package"
                && cluster.canonical_section == "purpose"
                && cluster.merge_pattern == "single-source"
        }));
        assert_ne!(
            package.semantic_ir.source_fingerprint,
            parsed_source_fingerprint(&rendered_source)
        );
    }

    #[test]
    fn import_blueprint_extracts_nested_structured_semantic_frames() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "nested json".to_string(),
                source_text: Some(
                    r#"{
  "requirements": {
    "acceptance_criteria": ["Emit a validated contract bundle."],
    "validation_plan": ["Run workspace validation."]
  },
  "governance": {
    "constraints": ["Authority docs outrank workflow docs."],
    "assumptions": ["External blueprints may omit defaults."],
    "out_of_scope": ["Target project implementation."]
  },
  "workflow": {
    "phases": [
      {
        "stage_id": "stage-01",
        "stage_name": "foundation",
        "verification": {
          "test_plan": ["Run foundation verification."]
        },
        "deliverables": ["Foundation bundle"]
      }
    ]
  }
}"#
                    .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/nested.json".to_string()),
            },
        )
        .expect("package should build");

        assert_eq!(
            package
                .semantic_ir
                .normalized_sections
                .get("required_verification"),
            Some(&vec![
                "Emit a validated contract bundle.".to_string(),
                "Run workspace validation.".to_string()
            ])
        );
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "package"
                && frame.canonical_section == "required_verification"
                && frame.source_locator == "requirements.acceptance_criteria"
                && frame.origin_kind == "nested-structured-key"
        }));
        assert!(package.semantic_ir.semantic_clusters.iter().any(|cluster| {
            cluster.scope == "package"
                && cluster.canonical_section == "required_verification"
                && cluster.merge_pattern == "multi-source"
                && cluster
                    .source_labels
                    .contains(&"acceptance_criteria".to_string())
                && cluster
                    .source_labels
                    .contains(&"validation_plan".to_string())
        }));
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-01"
                && frame.canonical_section == "required_verification"
                && frame.source_locator == "workflow.phases.0.verification.test_plan"
        }));
    }

    #[test]
    fn import_blueprint_extracts_inline_semantic_frames_from_markdown() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "inline markdown".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Notes\n\nAcceptance Criteria: Emit a validated contract bundle.\nConstraints: Authority docs outrank workflow docs.\nAssumptions: External blueprints may omit defaults.\nOut of Scope: Target project implementation.\n\n## Phases\n\n- stage-01: foundation\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/inline.md".to_string()),
            },
        )
        .expect("package should build");

        assert_eq!(
            package
                .semantic_ir
                .normalized_sections
                .get("required_verification"),
            Some(&vec!["Emit a validated contract bundle.".to_string()])
        );
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "package"
                && frame.canonical_section == "required_verification"
                && frame.origin_kind == "inline-label"
                && frame.source_label == "Acceptance Criteria"
                && frame.confidence == "inline-heuristic"
        }));
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "package"
                && frame.canonical_section == "truth_rules"
                && frame.source_label == "Constraints"
        }));
        assert!(package.semantic_ir.semantic_clusters.iter().any(|cluster| {
            cluster.scope == "package"
                && cluster.canonical_section == "required_verification"
                && cluster.merge_pattern == "single-source-heuristic"
        }));
    }

    #[test]
    fn import_blueprint_extracts_multiline_inline_semantic_blocks_from_markdown() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "multiline inline markdown".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Notes\n\nAcceptance Criteria:\n- Emit a validated contract bundle.\n- Preserve readiness evidence.\nValidation Plan:\n- Run cargo check.\n- Run workspace validation.\nConstraints:\n- Authority docs outrank workflow docs.\n\n## Phases\n\n- stage-01: foundation\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/inline-multiline.md".to_string()),
            },
        )
        .expect("package should build");

        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "package"
                && frame.canonical_section == "required_verification"
                && frame.source_label == "Acceptance Criteria"
                && frame
                    .values
                    .contains(&"Emit a validated contract bundle.".to_string())
                && frame
                    .values
                    .contains(&"Preserve readiness evidence.".to_string())
        }));
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "package"
                && frame.canonical_section == "required_verification"
                && frame.source_label == "Validation Plan"
                && frame.values.contains(&"Run cargo check.".to_string())
                && frame
                    .values
                    .contains(&"Run workspace validation.".to_string())
        }));
        assert!(package.semantic_ir.semantic_clusters.iter().any(|cluster| {
            cluster.scope == "package"
                && cluster.canonical_section == "required_verification"
                && cluster.merge_pattern == "multi-source-heuristic"
                && cluster
                    .merged_values
                    .contains(&"Emit a validated contract bundle.".to_string())
                && cluster
                    .merged_values
                    .contains(&"Run workspace validation.".to_string())
        }));
    }

    #[test]
    fn import_blueprint_extracts_stage_scoped_details_from_markdown_phase_blocks() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "markdown phase blocks".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Phases\n\n- stage-01: foundation\n- Deliverables:\n  - Foundation bundle\n- Validation Plan:\n  - Run foundation validation.\n- stage-02: hardening\n- Deliverables:\n  - Hardening bundle\n- Validation Plan:\n  - Run hardening validation.\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/phase-blocks.md".to_string()),
            },
        )
        .expect("package should build");

        assert_eq!(package.resolved_contract.stages[0].stage_id, "stage-01");
        assert_eq!(package.resolved_contract.stages[1].stage_id, "stage-02");
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-01"
                && frame.canonical_section == "deliverables"
                && frame.values.contains(&"Foundation bundle".to_string())
        }));
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-02"
                && frame.canonical_section == "required_verification"
                && frame
                    .values
                    .contains(&"Run hardening validation.".to_string())
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-01"
                && document.content.contains("Foundation bundle")
                && document.content.contains("Run foundation validation.")
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-02"
                && document.content.contains("Hardening bundle")
                && document.content.contains("Run hardening validation.")
        }));
    }

    #[test]
    fn import_blueprint_extracts_stage_scoped_details_from_markdown_phase_headings() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "markdown phase headings".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Phases\n\n### stage-01: foundation\n\n#### Deliverables\n\n- Foundation bundle\n\n#### Validation Plan\n\n- Run foundation validation.\n\n### stage-02: hardening\n\n#### Deliverables\n\n- Hardening bundle\n\n#### Validation Plan\n\n- Run hardening validation.\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/phase-headings.md".to_string()),
            },
        )
        .expect("package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-01"
                && frame.canonical_section == "deliverables"
                && frame.origin_kind == "stage-heading-section"
                && frame.values.contains(&"Foundation bundle".to_string())
        }));
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-02"
                && frame.canonical_section == "required_verification"
                && frame.origin_kind == "stage-heading-section"
                && frame
                    .values
                    .contains(&"Run hardening validation.".to_string())
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-01"
                && document.content.contains("Foundation bundle")
                && document.content.contains("Run foundation validation.")
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-02"
                && document.content.contains("Hardening bundle")
                && document.content.contains("Run hardening validation.")
        }));
    }

    #[test]
    fn import_blueprint_aggregates_stage_details_across_source_blocks_by_stage_alias() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "cross source stage alias markdown".to_string(),
                source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n# Source File\n\ndocs/foundation-notes.md\n\n## Foundation\n\n### Deliverables\n\n- Foundation bundle\n\n### Validation Plan\n\n- Run foundation validation.\n\n# Source File\n\ndocs/hardening-notes.md\n\n## Hardening\n\nDeliverables:\n- Hardening bundle\nValidation Plan:\n- Run hardening validation.\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/cross-source-stage-alias.md".to_string()),
            },
        )
        .expect("package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-01"
                && frame.canonical_section == "deliverables"
                && frame.origin_kind == "stage-alias-heading-section"
                && frame.confidence == "cross-source-exact"
                && frame.values.contains(&"Foundation bundle".to_string())
        }));
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-02"
                && frame.canonical_section == "required_verification"
                && frame.origin_kind == "stage-alias-inline-label"
                && frame.confidence == "cross-source-heuristic"
                && frame
                    .values
                    .contains(&"Run hardening validation.".to_string())
        }));
        assert!(package.semantic_ir.semantic_clusters.iter().any(|cluster| {
            cluster.scope == "stage:stage-01"
                && cluster.canonical_section == "deliverables"
                && cluster
                    .source_locators
                    .iter()
                    .any(|locator| locator.contains("foundation-notes.md"))
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-01"
                && document.content.contains("Foundation bundle")
                && document.content.contains("Run foundation validation.")
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-02"
                && document.content.contains("Hardening bundle")
                && document.content.contains("Run hardening validation.")
        }));
    }

    #[test]
    fn import_blueprint_aggregates_stage_details_from_path_alias_source_blocks() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "path alias stage markdown".to_string(),
                source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n# Source File\n\ndocs/foundation-notes.md\n\n## Deliverables\n\n- Foundation bundle\n\n## Validation Plan\n\n- Run foundation validation.\n\n# Source File\n\ndocs/hardening-validation-plan.md\n\nDeliverables:\n- Hardening bundle\nValidation Plan:\n- Run hardening validation.\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/path-alias-stage-source.md".to_string()),
            },
        )
        .expect("package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-01"
                && frame.canonical_section == "deliverables"
                && frame.origin_kind == "stage-path-alias-heading-section"
                && frame.confidence == "path-alias-exact"
                && frame.values.contains(&"Foundation bundle".to_string())
        }));
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-02"
                && frame.canonical_section == "required_verification"
                && frame.origin_kind == "stage-path-alias-inline-label"
                && frame.confidence == "path-alias-heuristic"
                && frame
                    .values
                    .contains(&"Run hardening validation.".to_string())
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-01"
                && document.content.contains("Foundation bundle")
                && document.content.contains("Run foundation validation.")
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-02"
                && document.content.contains("Hardening bundle")
                && document.content.contains("Run hardening validation.")
        }));
    }

    #[test]
    fn import_blueprint_aggregates_stage_details_from_document_stage_metadata() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "stage metadata markdown".to_string(),
                source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n# Source File\n\ndocs/foundation-plan.md\n\n---\nstage: foundation\n---\n\n## Deliverables\n\n- Foundation bundle\n\nValidation Plan:\n- Run foundation validation.\n\n# Source File\n\ndocs/hardening-plan.md\n\nStage ID: stage-02\n\n## Deliverables\n\n- Hardening bundle\n\nValidation Plan:\n- Run hardening validation.\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/stage-metadata-source.md".to_string()),
            },
        )
        .expect("package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-01"
                && frame.canonical_section == "deliverables"
                && frame.origin_kind == "stage-metadata-heading-section"
                && frame.confidence == "metadata-alias"
                && frame.values.contains(&"Foundation bundle".to_string())
        }));
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-02"
                && frame.canonical_section == "required_verification"
                && frame.origin_kind == "stage-metadata-inline-label"
                && frame.confidence == "metadata-heuristic"
                && frame
                    .values
                    .contains(&"Run hardening validation.".to_string())
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-01"
                && document.content.contains("Foundation bundle")
                && document.content.contains("Run foundation validation.")
        }));
        assert!(package.stage_documents.iter().any(|document| {
            document.stage_id == "stage-02"
                && document.content.contains("Hardening bundle")
                && document.content.contains("Run hardening validation.")
        }));
    }

    #[test]
    fn import_blueprint_aggregates_stage_details_from_generic_metadata_variants() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "generic stage metadata markdown".to_string(),
                source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n# Source File\n\ndocs/foundation-brief.md\n\n- Milestone: foundation\n\n## Deliverables\n\n- Foundation bundle\n\n# Source File\n\ndocs/hardening-brief.md\n\nphase_id = \"stage-02\"\n\nValidation Plan:\n- Run hardening validation.\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/generic-stage-metadata-source.md".to_string()),
            },
        )
        .expect("package should build");

        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-01"
                && frame.canonical_section == "deliverables"
                && frame.origin_kind == "stage-metadata-heading-section"
                && frame.confidence == "metadata-alias"
                && frame.values.contains(&"Foundation bundle".to_string())
        }));
        assert!(package.semantic_ir.semantic_frames.iter().any(|frame| {
            frame.scope == "stage:stage-02"
                && frame.canonical_section == "required_verification"
                && frame.origin_kind == "stage-metadata-inline-label"
                && frame.confidence == "metadata-heuristic"
                && frame
                    .values
                    .contains(&"Run hardening validation.".to_string())
        }));
    }

    #[test]
    fn import_blueprint_reports_stage_semantic_conflicts_from_indirect_sources() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "conflicting stage alias markdown".to_string(),
                source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n\n# Source File\n\ndocs/foundation-notes.md\n\n## Foundation\n\n### Deliverables\n\n- Foundation bundle\n\n# Source File\n\ndocs/foundation-review.md\n\n## Foundation Review\n\n### Deliverables\n\n- Conflicting bundle\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/conflicting-stage-alias.md".to_string()),
            },
        )
        .expect("package should build");

        assert!(package
            .normalization_report
            .semantic_risks
            .iter()
            .any(|item| item.contains("conflicting stage semantic evidence remained in `stage:stage-01` for `deliverables`")
                && item.contains("docs/foundation-notes.md")
                && item.contains("docs/foundation-review.md")));
        assert!(package
            .normalization_report
            .unresolved_ambiguities
            .iter()
            .any(|item| item.contains(
                "stage semantic conflict remained in `stage:stage-01` for `deliverables`"
            ) && item.contains("docs/foundation-notes.md")
                && item.contains("docs/foundation-review.md")));
        assert!(package
            .normalization_report
            .semantic_hints
            .iter()
            .any(|item| item.contains("detected stage-scoped semantic divergence in `stage:stage-01` for `deliverables`")
                && item.contains("path-alias-exact")));
        let conflict = package
            .normalization_report
            .semantic_conflicts
            .iter()
            .find(|conflict| {
                conflict.scope == "stage:stage-01"
                    && conflict.canonical_section == "deliverables"
                    && conflict.conflict_kind == "indirect-source-divergence"
            })
            .expect("semantic conflict detail should exist");
        assert_eq!(conflict.source_groups.len(), 2);
        assert!(conflict
            .source_groups
            .iter()
            .any(|group| group.source_group == "docs/foundation-notes.md"));
        assert!(conflict
            .source_groups
            .iter()
            .any(|group| group.source_group == "docs/foundation-review.md"));
        assert!(conflict
            .confidence_profile
            .iter()
            .any(|value| value == "path-alias-exact"));
        assert_eq!(conflict.severity, "medium");
        assert!(!conflict.blocking);
        assert!(conflict.review_required);
        assert_eq!(
            conflict.recommended_action,
            "add explicit stage metadata or canonical stage-scoped sections to disambiguate conflicting source groups"
        );
        assert_eq!(package.readiness.blocking_semantic_conflict_count, 0);
        assert_eq!(package.readiness.review_required_semantic_conflict_count, 1);
        assert!(package.readiness.gate_holds.is_empty());
        assert!(package
            .readiness
            .recommended_actions
            .iter()
            .any(|item| item
                == "add explicit stage metadata or canonical stage-scoped sections to disambiguate conflicting source groups"));
        assert!(package
            .readiness
            .reason
            .contains("review-required semantic conflicts"));
        assert_eq!(
            package.decision_summary.review_required_kinds,
            vec!["semantic-conflict".to_string()]
        );
        assert_eq!(
            package
                .decision_summary
                .review_required_kind_counts
                .get("semantic-conflict"),
            Some(&1)
        );
        assert!(package.decision_summary.primary_blocker_kind.is_none());
        assert_eq!(
            package.decision_summary.primary_recommended_action.as_deref(),
            Some(
                "add explicit stage metadata or canonical stage-scoped sections to disambiguate conflicting source groups"
            )
        );
        assert!(package.decision_summary.top_blockers.is_empty());
        assert_eq!(package.decision_summary.top_review_items.len(), 1);
        assert_eq!(
            package.decision_summary.top_review_items[0].kind,
            "semantic-conflict"
        );
    }

    #[test]
    fn validate_rejects_semantic_conflicts_without_decision_metadata() {
        let mut package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "conflicting stage alias markdown".to_string(),
                source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n\n# Source File\n\ndocs/foundation-notes.md\n\n## Foundation\n\n### Deliverables\n\n- Foundation bundle\n\n# Source File\n\ndocs/foundation-review.md\n\n## Foundation Review\n\n### Deliverables\n\n- Conflicting bundle\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/conflicting-stage-alias.md".to_string()),
            },
        )
        .expect("package should build");

        let conflict = package
            .normalization_report
            .semantic_conflicts
            .get_mut(0)
            .expect("semantic conflict should exist");
        conflict.severity.clear();
        conflict.recommended_action.clear();
        conflict.review_required = false;
        conflict.blocking = true;

        let error = validate_package(&package).expect_err("missing decision metadata should fail");
        assert!(matches!(
            error,
            CoreError::NormalizationEvidenceMissing { ref field } if field == "semantic_conflicts"
        ));
    }

    #[test]
    fn validate_rejects_readiness_gate_summary_drift() {
        let mut package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "conflicting stage alias markdown".to_string(),
                source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n\n# Source File\n\ndocs/foundation-notes.md\n\n## Foundation\n\n### Deliverables\n\n- Foundation bundle\n\n# Source File\n\ndocs/foundation-review.md\n\n## Foundation Review\n\n### Deliverables\n\n- Conflicting bundle\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/conflicting-stage-alias.md".to_string()),
            },
        )
        .expect("package should build");

        package.readiness.review_required_semantic_conflict_count = 0;

        let error =
            validate_package(&package).expect_err("readiness gate summary drift should fail");
        assert!(matches!(
            error,
            CoreError::ReadinessInconsistent { ref field }
                if field == "review_required_semantic_conflict_count"
        ));
    }

    #[test]
    fn validate_rejects_readiness_patch_gate_summary_drift() {
        let mut package = build_package(
            AuthorMode::UpdateBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "update".to_string(),
                source_text: Some(
                    "# Update Input\n\n## Stage Order\n\n- stage-02: hardening\n\n## Deliverables\n\n- Hardening review bundle\n".to_string(),
                ),
                workspace_source_text: Some(
                    "# Source File\n\nblueprint/authority/00-authority-root.md\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Deliverables\n\n- Foundation bundle\n".to_string(),
                ),
                source_path: Some("workspace:C:/demo + source:updates/update.md".to_string()),
            },
        )
        .expect("package should build");

        package.readiness.high_risk_patch_operation_count = 0;

        let error =
            validate_package(&package).expect_err("readiness patch gate summary drift should fail");
        assert!(matches!(
            error,
            CoreError::ReadinessInconsistent { ref field }
                if field == "high_risk_patch_operation_count"
        ));
    }

    #[test]
    fn validate_rejects_decision_summary_drift() {
        let mut package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "conflicting stage alias markdown".to_string(),
                source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n\n# Source File\n\ndocs/foundation-notes.md\n\n## Foundation\n\n### Deliverables\n\n- Foundation bundle\n\n# Source File\n\ndocs/foundation-review.md\n\n## Foundation Review\n\n### Deliverables\n\n- Conflicting bundle\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/conflicting-stage-alias.md".to_string()),
            },
        )
        .expect("package should build");

        package.decision_summary.recommended_action_count = 0;

        let error =
            validate_package(&package).expect_err("decision summary drift should fail");
        assert!(matches!(
            error,
            CoreError::ReadinessInconsistent { ref field } if field == "decision_summary"
        ));
    }

    #[test]
    fn validate_rejects_agent_brief_drift() {
        let mut package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "conflicting stage alias markdown".to_string(),
                source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n\n# Source File\n\ndocs/foundation-notes.md\n\n## Foundation\n\n### Deliverables\n\n- Foundation bundle\n\n# Source File\n\ndocs/foundation-review.md\n\n## Foundation Review\n\n### Deliverables\n\n- Conflicting bundle\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/conflicting-stage-alias.md".to_string()),
            },
        )
        .expect("package should build");

        package.agent_brief.reason = "tampered agent brief".to_string();

        let error = validate_package(&package).expect_err("agent brief drift should fail");
        assert!(matches!(
            error,
            CoreError::ReadinessInconsistent { ref field } if field == "agent_brief"
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_duplicate_branch_patterns() {
        let mut package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "worktree duplicate branch".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01\n- hardening-role | branch=codex/hardening-role | stages=stage-02\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-duplicate-branch.md".to_string()),
            },
        )
        .expect("package should build");

        package.worktree_protocol.roles[1].branch_pattern =
            package.worktree_protocol.roles[0].branch_pattern.clone();
        package.resolved_contract.worktree = package.worktree_protocol.clone();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_with_invalid_branch_pattern() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "worktree invalid branch pattern".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation role | stages=stage-01\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-invalid-branch-pattern.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn import_mode_uses_module_isolated_worktree_defaults() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "module-isolated defaults".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Worktree Model\n\n- module-isolated-worktree\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-module-default-policy.md".to_string()),
            },
        )
        .expect("package should build");

        assert!(package
            .worktree_protocol
            .parallel_worktree_policy
            .iter()
            .any(|rule| rule.contains("module role")));
        assert!(package
            .worktree_protocol
            .sync_rule
            .iter()
            .any(|rule| rule.contains("module task")));
        assert!(package
            .worktree_protocol
            .merge_back_rule
            .iter()
            .any(|rule| rule.contains("module-scoped")));
        assert!(package
            .worktree_protocol
            .cleanup_rule
            .iter()
            .any(|rule| rule.contains("module-scoped")));
    }

    #[test]
    fn validate_rejects_module_isolated_parallel_policy_with_stage_scope() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "module-isolated policy mismatch".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Worktree Model\n\n- module-isolated-worktree\n\n## Parallel Worktree Policy\n\n- Use one active implementation worktree per stage role when parallel execution is necessary.\n- Shared authority and workflow changes must be synchronized before merge-back.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-module-policy-mismatch.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_module_isolated_sync_rule_with_stage_scope() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "module-isolated sync mismatch".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Worktree Model\n\n- module-isolated-worktree\n\n## Worktree Sync Rule\n\n- Sync each active worktree from the main integration branch before starting a new stage task.\n- Re-run blueprint validation after any shared authority or workflow sync.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-module-sync-mismatch.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_module_isolated_merge_back_rule_with_stage_scope() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "module-isolated merge-back mismatch".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Worktree Model\n\n- module-isolated-worktree\n\n## Worktree Merge Back Rule\n\n- Only merge a worktree back after its stage-scoped changes pass validation and contract recheck.\n- Shared authority and workflow updates must merge before downstream worktrees rebase.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-module-merge-back-mismatch.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_module_isolated_cleanup_rule_with_stage_scope() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "module-isolated cleanup mismatch".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Worktree Model\n\n- module-isolated-worktree\n\n## Worktree Cleanup Rule\n\n- Delete or recycle a worktree after its stage changes are merged and the next-stage handoff is written.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-module-cleanup-mismatch.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_overlapping_branch_hierarchy() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "worktree overlapping branch hierarchy".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation | stages=stage-01\n- hardening-role | branch=codex/foundation/hardening | stages=stage-02\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-overlapping-branch-hierarchy.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_overlapping_exclusive_paths() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "worktree overlapping paths".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | paths=blueprint/stages\n- hardening-role | branch=codex/hardening-role | stages=stage-02 | paths=blueprint/stages/02-hardening.md\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-overlapping-paths.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_role_without_scope() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "worktree role without scope".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-empty-role.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_duplicate_explicit_worktree_module_owners() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "duplicate worktree module owner".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core\n- hardening-role | branch=codex/hardening-role | stages=stage-02 | modules=ara-core\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-duplicate-module-owner.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_module_isolated_worktree_role_without_module_scope() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "module-isolated worktree role without module scope".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Worktree Model\n\n- module-isolated-worktree\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core\n- hardening-role | branch=codex/hardening-role | stages=stage-01\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-missing-module-scope.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_stage_isolated_role_with_multiple_stages() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "stage-isolated multi-stage role".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Worktree Model\n\n- stage-isolated-worktree\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01,stage-02\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-multi-stage-role.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_without_sync_rule() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.worktree_protocol.sync_rule.clear();
        package.resolved_contract.worktree = package.worktree_protocol.clone();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_with_placeholder_sync_rule() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.worktree_protocol.sync_rule = vec!["todo".to_string()];
        package.resolved_contract.worktree = package.worktree_protocol.clone();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_with_non_actionable_sync_rule() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.worktree_protocol.sync_rule =
            vec!["This rule exists but says nothing concrete.".to_string()];
        package.resolved_contract.worktree = package.worktree_protocol.clone();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_without_shared_authority_paths() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.worktree_protocol.shared_authority_paths.clear();

        let error = validate_worktree_protocol_alignment(
            &package.worktree_protocol,
            &package.resolved_contract,
            &package.module_catalog,
        )
        .expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_without_shared_authority_coordination_rule() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "shared authority coordination gap".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Shared Authority Paths\n\n- blueprint/authority\n- blueprint/workflow\n- .codex/auto-dev\n\n## Parallel Worktree Policy\n\n- Use one active implementation worktree per stage role when parallel execution is necessary.\n\n## Worktree Sync Rule\n\n- Sync each active worktree from the main integration branch before starting a new stage task.\n\n## Worktree Merge Back Rule\n\n- Only merge a worktree back after its stage-scoped changes pass validation and contract recheck.\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-shared-authority-gap.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_worktree_protocol_without_shared_authority_cleanup_rule() {
        let error = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "shared authority cleanup gap".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Shared Authority Paths\n\n- blueprint/authority\n- blueprint/workflow\n- .codex/auto-dev\n\n## Parallel Worktree Policy\n\n- Use one active implementation worktree per stage role when parallel execution is necessary.\n- Shared authority and workflow updates must stay serialized across worktrees.\n\n## Worktree Sync Rule\n\n- Sync each active worktree from the main integration branch before starting a new stage task.\n- Re-run blueprint validation after any shared authority or workflow sync.\n\n## Worktree Merge Back Rule\n\n- Only merge a worktree back after its stage-scoped changes pass validation and contract recheck.\n- Shared authority and workflow updates must merge before downstream worktrees rebase.\n\n## Worktree Cleanup Rule\n\n- Delete or recycle a worktree after its stage-scoped changes are merged and the next handoff is written.\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/worktree-shared-authority-cleanup-gap.md".to_string()),
            },
        )
        .expect_err("build should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn update_patch_plan_classifies_strategy_metadata() {
        let package = build_package(
            AuthorMode::UpdateBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "update".to_string(),
                source_text: Some(
                    "# Update Input\n\n## Stage Order\n\n- stage-02: hardening\n\n## Deliverables\n\n- Hardening review bundle\n".to_string(),
                ),
                workspace_source_text: Some(
                    "# Source File\n\nblueprint/authority/00-authority-root.md\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Deliverables\n\n- Foundation bundle\n".to_string(),
                ),
                source_path: Some("workspace:C:/demo + source:updates/update.md".to_string()),
            },
        )
        .expect("package should build");

        let stage_order_patch = package
            .patch_plan
            .operations
            .iter()
            .find(|operation| operation.strategy == "merge-stage-order")
            .expect("stage order patch should exist");
        assert_eq!(
            stage_order_patch.strategy_metadata.get("risk_level"),
            Some(&"medium".to_string())
        );
        assert_eq!(
            stage_order_patch.strategy_metadata.get("review_required"),
            Some(&"false".to_string())
        );
        assert_eq!(
            stage_order_patch.strategy_metadata.get("apply_mode"),
            Some(&"merge".to_string())
        );

        let deliverables_patch = package
            .patch_plan
            .operations
            .iter()
            .find(|operation| operation.target_id == "deliverables")
            .expect("deliverables patch should exist");
        assert!(deliverables_patch
            .strategy_metadata
            .contains_key("risk_level"));
        assert!(deliverables_patch
            .strategy_metadata
            .contains_key("review_required"));
        assert!(deliverables_patch
            .strategy_metadata
            .contains_key("apply_mode"));
    }

    #[test]
    fn update_readiness_projects_patch_risk_summary() {
        let package = build_package(
            AuthorMode::UpdateBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "update".to_string(),
                source_text: Some(
                    "# Update Input\n\n## Stage Order\n\n- stage-02: hardening\n\n## Deliverables\n\n- Hardening review bundle\n".to_string(),
                ),
                workspace_source_text: Some(
                    "# Source File\n\nblueprint/authority/00-authority-root.md\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Deliverables\n\n- Foundation bundle\n".to_string(),
                ),
                source_path: Some("workspace:C:/demo + source:updates/update.md".to_string()),
            },
        )
        .expect("package should build");

        assert!(package.readiness.high_risk_patch_operation_count >= 1);
        assert!(package.readiness.review_required_patch_operation_count >= 1);
        assert!(package
            .readiness
            .gate_holds
            .iter()
            .any(|item| item.starts_with("patch-risk:package:")));
        assert!(package
            .readiness
            .recommended_actions
            .iter()
            .any(|item| item.contains("review patch operation `apply-update-delta`")));
        assert!(package
            .readiness
            .reason
            .contains("high-risk patch operations"));
        assert_eq!(
            package.decision_summary.primary_blocker_kind.as_deref(),
            Some("patch-risk")
        );
        assert_eq!(
            package.decision_summary.primary_blocker_scope.as_deref(),
            Some("package")
        );
        assert_eq!(
            package.decision_summary.primary_blocker_target_id.as_deref(),
            Some("demo")
        );
        assert_eq!(
            package
                .decision_summary
                .blocking_kind_counts
                .get("patch-risk"),
            Some(&package.readiness.high_risk_patch_operation_count)
        );
        assert!(package
            .decision_summary
            .primary_recommended_action
            .as_deref()
            .unwrap_or_default()
            .contains("review patch operation `apply-update-delta`"));
        assert!(!package.decision_summary.top_blockers.is_empty());
        assert_eq!(
            package.decision_summary.top_blockers[0].kind,
            "patch-risk"
        );
    }

    #[test]
    fn update_patch_plan_captures_reversible_patch_metadata() {
        let package = build_package(
            AuthorMode::UpdateBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "update".to_string(),
                source_text: Some(
                    "# Update Input\n\n## Stage Order\n\n- stage-02: hardening\n\n## Deliverables\n\n- Hardening review bundle\n".to_string(),
                ),
                workspace_source_text: Some(
                    "# Source File\n\nblueprint/authority/00-authority-root.md\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Deliverables\n\n- Foundation bundle\n".to_string(),
                ),
                source_path: Some("workspace:C:/demo + source:updates/update.md".to_string()),
            },
        )
        .expect("package should build");

        assert!(package.patch_plan.operations.iter().all(|operation| {
            operation.reverse_strategy.is_some()
                && operation.strategy_metadata.get("is_reversible") == Some(&"true".to_string())
        }));
        assert_eq!(package.patch_execution_report.replay_status, "replayed");
        assert_eq!(
            package.patch_execution_report.reversibility_status,
            "reversible"
        );
        assert_eq!(
            package
                .patch_execution_report
                .reverse_replayed_base_fingerprint,
            package.patch_plan.base_fingerprint
        );
    }

    #[test]
    fn validate_rejects_tampered_module_catalog_definition() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.module_catalog.modules[0].responsibility = "tampered".to_string();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(error, CoreError::ModuleCatalogMismatch { .. }));
    }

    #[test]
    fn validate_rejects_stage_document_metadata_that_disagrees_with_contract() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.stage_documents[0].content = package.stage_documents[0]
            .content
            .replace("foundation", "tampered-stage");
        let updated_manifest_entry = manifest_entry(
            &package.stage_documents[0].path,
            "stage",
            &package.stage_documents[0].stage_id,
            &package.source_provenance,
            &package.stage_documents[0].content,
        );
        let stage_manifest_entry = package
            .manifest
            .files
            .iter_mut()
            .find(|entry| entry.canonical_id == package.stage_documents[0].stage_id)
            .expect("stage manifest entry should exist");
        *stage_manifest_entry = updated_manifest_entry;

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn validate_rejects_duplicate_stage_document_paths() {
        let mut package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n".to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/demo.md".to_string()),
            },
        )
        .expect("package should build");

        package.stage_documents[1].path = package.stage_documents[0].path.clone();

        let error = validate_package(&package).expect_err("duplicate stage paths should fail");
        assert!(matches!(error, CoreError::InvalidStageGraph { .. }));
    }

    #[test]
    fn build_package_disambiguates_stage_document_paths_when_stage_names_collide() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "colliding stage names".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Stage Order\n\n- stage-01: Release Prep\n- stage-02: Release/Prep\n".to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/collision.md".to_string()),
            },
        )
        .expect("package should build");

        assert_eq!(package.stage_documents.len(), 2);
        assert_ne!(
            package.stage_documents[0].path,
            package.stage_documents[1].path
        );
        assert_eq!(
            package.stage_documents[0].path,
            "blueprint/stages/01-release-prep.md"
        );
        assert_eq!(
            package.stage_documents[1].path,
            "blueprint/stages/02-release-prep.md"
        );
    }

    #[test]
    fn manifest_fingerprint_changes_when_content_changes() {
        let package_a = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary-a".to_string(),
                source_text: Some("# Input\n\n## Purpose\n\nAuthority A\n".to_string()),
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        let package_b = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary-b".to_string(),
                source_text: Some("# Input\n\n## Purpose\n\nAuthority B\n".to_string()),
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");

        let authority_a = package_a
            .manifest
            .files
            .iter()
            .find(|entry| entry.canonical_id == "authority-root")
            .expect("authority manifest entry");
        let authority_b = package_b
            .manifest
            .files
            .iter()
            .find(|entry| entry.canonical_id == "authority-root")
            .expect("authority manifest entry");

        assert_ne!(authority_a.fingerprint, authority_b.fingerprint);
        assert_ne!(
            package_a.readiness.fingerprint,
            package_b.readiness.fingerprint
        );
    }

    #[test]
    fn validate_rejects_missing_normalization_evidence() {
        let mut package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n"
                        .to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/demo.md".to_string()),
            },
        )
        .expect("package should build");
        package.normalization_report.source_files.clear();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::NormalizationEvidenceMissing { .. }
        ));
    }

    #[test]
    fn update_mode_merges_new_stage_into_existing_workspace_blueprint() {
        let package = build_package(
            AuthorMode::UpdateBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "update".to_string(),
                source_text: Some(
                    "# Update Input\n\n## Stage Order\n\n- stage-02: hardening\n\n## Deliverables\n\n- Hardening review bundle\n".to_string(),
                ),
                workspace_source_text: Some(
                    "# Source File\n\nblueprint/authority/00-authority-root.md\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Deliverables\n\n- Foundation bundle\n".to_string(),
                ),
                source_path: Some("workspace:C:/demo + source:updates/update.md".to_string()),
            },
        )
        .expect("update package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert_eq!(package.stage_documents.len(), 2);
        assert!(package
            .normalization_report
            .unresolved_ambiguities
            .is_empty());
        assert!(package
            .stage_documents
            .iter()
            .any(|doc| doc.path == "blueprint/stages/02-hardening.md"));
        assert!(package.change_report.patch_operation_count > 0);
        assert!(package
            .change_report
            .patch_operations
            .iter()
            .any(|operation| operation.strategy == "merge-stage-order"));
        assert!(package
            .change_report
            .patch_operations
            .iter()
            .any(|operation| operation.strategy == "union-merge"));
        let stage_order_patch = package
            .patch_plan
            .operations
            .iter()
            .find(|operation| operation.strategy == "merge-stage-order")
            .expect("stage order patch operation should exist");
        assert_eq!(stage_order_patch.value_lines, vec!["stage-02: hardening"]);
        let deliverables_patch = package
            .patch_plan
            .operations
            .iter()
            .find(|operation| {
                operation.strategy == "union-merge" && operation.target_id == "deliverables"
            })
            .expect("deliverables patch operation should exist");
        assert_eq!(
            deliverables_patch.value_lines,
            vec!["Hardening review bundle"]
        );
    }

    #[test]
    fn update_mode_rejects_conflicting_authority_purpose() {
        let error = build_package(
            AuthorMode::UpdateBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "update".to_string(),
                source_text: Some("# Update Input\n\n## Purpose\n\nBuild a different system.\n".to_string()),
                workspace_source_text: Some("# Source File\n\nblueprint/authority/00-authority-root.md\n\n## Purpose\n\nBuild the original system.\n".to_string()),
                source_path: Some("workspace:C:/demo + source:updates/conflicting-update.md".to_string()),
            },
        )
        .expect_err("conflicting authority update should fail");

        assert!(matches!(error, CoreError::AuthorityConflict { .. }));
    }

    #[test]
    fn recompile_mode_strips_backticks_from_generated_stage_ids() {
        let package = build_package(
            AuthorMode::RecompileContract,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "recompile".to_string(),
                source_text: None,
                workspace_source_text: Some(
                    "# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n1. `stage-01` foundation\n2. `stage-02` hardening\n".to_string(),
                ),
                source_path: Some("workspace".to_string()),
            },
        )
        .expect("recompile package should build");

        assert_eq!(package.resolved_contract.stages[0].stage_id, "stage-01");
        assert_eq!(package.resolved_contract.stages[1].stage_id, "stage-02");
    }

    #[test]
    fn recompile_mode_recovers_stage_order_from_workspace_stage_docs_when_workflow_is_missing() {
        let package = build_package(
            AuthorMode::RecompileContract,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "recompile".to_string(),
                source_text: None,
                workspace_source_text: Some(
                    "# Source File\n\nblueprint/stages/01-foundation.md\n\n# Stage Document\n\n## Stage\n\n- `stage_id`: `stage-01`\n- `stage_name`: `foundation`\n\n## Deliverables\n\n- Foundation bundle\n\n# Source File\n\nblueprint/stages/02-hardening.md\n\n# Stage Document\n\n## Stage\n\n- `stage_id`: `stage-02`\n- `stage_name`: `hardening`\n\n## Deliverables\n\n- Hardening bundle\n"
                        .to_string(),
                ),
                source_path: Some("workspace".to_string()),
            },
        )
        .expect("recompile package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert_eq!(package.resolved_contract.stages[0].stage_id, "stage-01");
        assert_eq!(package.resolved_contract.stages[1].stage_id, "stage-02");
        assert!(package
            .normalization_report
            .inferred_sections
            .contains(&"stage_order".to_string()));
    }

    #[test]
    fn emit_prunes_stale_stage_documents() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-core-prune-{unique}"));
        if workspace.exists() {
            fs::remove_dir_all(&workspace).expect("cleanup existing temp workspace");
        }

        let initial_package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n".to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/demo.md".to_string()),
            },
        )
        .expect("initial package should build");
        emit_package(&workspace, &initial_package).expect("initial emit should succeed");

        let stale_stage = workspace.join("blueprint/stages/02-hardening.md");
        assert!(
            stale_stage.exists(),
            "second stage should exist after initial emit"
        );

        let updated_package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "single-stage blueprint".to_string(),
                source_text: Some(
                    "# Input\n\n## Stage Order\n\n- stage-01: foundation\n".to_string(),
                ),
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("updated package should build");
        emit_package(&workspace, &updated_package).expect("updated emit should succeed");

        assert!(
            !stale_stage.exists(),
            "stale stage document should be pruned"
        );

        if workspace.exists() {
            fs::remove_dir_all(&workspace).expect("cleanup temp workspace");
        }
    }

    #[test]
    fn validate_workspace_accepts_missing_patch_plan_via_change_report_fallback() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-core-missing-patch-plan-{unique}"));
        if workspace.exists() {
            fs::remove_dir_all(&workspace).expect("cleanup existing temp workspace");
        }

        let package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        emit_package(&workspace, &package).expect("emit should succeed");

        let patch_plan_path = workspace.join(".codex/auto-dev/patch-plan.json");
        fs::remove_file(&patch_plan_path).expect("remove patch-plan");

        let validated = validate_workspace_package(&workspace).expect("workspace should validate");
        assert_eq!(
            validated.patch_plan.operation_count,
            validated.change_report.patch_operation_count
        );
        assert_eq!(
            validated.patch_plan.operations.len(),
            validated.change_report.patch_operations.len()
        );
        assert!(validated
            .patch_plan
            .operations
            .iter()
            .zip(validated.change_report.patch_operations.iter())
            .all(|(patch_plan_op, change_report_op)| {
                patch_plan_op.scope == change_report_op.scope
                    && patch_plan_op.target_id == change_report_op.target_id
                    && patch_plan_op.strategy == change_report_op.strategy
                    && patch_plan_op.status == change_report_op.status
                    && patch_plan_op.details == change_report_op.details
            }));
        assert!(validated
            .patch_plan
            .operations
            .iter()
            .filter(|operation| operation.scope == "section")
            .any(|operation| !operation.value_lines.is_empty()));

        if workspace.exists() {
            fs::remove_dir_all(&workspace).expect("cleanup temp workspace");
        }
    }

    #[test]
    fn validate_rejects_patch_plan_that_disagrees_with_change_report() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.patch_plan.operations[0].details = "tampered patch operation".to_string();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(error, CoreError::ChangeReportInconsistent { .. }));
    }

    #[test]
    fn validate_workspace_accepts_missing_semantic_ir_via_workspace_fallback() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-core-missing-semantic-ir-{unique}"));
        if workspace.exists() {
            fs::remove_dir_all(&workspace).expect("cleanup existing temp workspace");
        }

        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n".to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/demo.md".to_string()),
            },
        )
        .expect("package should build");
        emit_package(&workspace, &package).expect("emit should succeed");

        let semantic_ir_path = workspace.join(".codex/auto-dev/semantic-ir.json");
        fs::remove_file(&semantic_ir_path).expect("remove semantic-ir");

        let validated = validate_workspace_package(&workspace).expect("workspace should validate");
        assert_eq!(
            validated.semantic_ir.project_name,
            validated.resolved_contract.project_name
        );
        assert_eq!(validated.semantic_ir.stages.len(), 1);

        if workspace.exists() {
            fs::remove_dir_all(&workspace).expect("cleanup temp workspace");
        }
    }

    #[test]
    fn validate_rejects_semantic_ir_that_disagrees_with_rendered_package() {
        let mut package = build_package(
            AuthorMode::NewProject,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "summary".to_string(),
                source_text: None,
                workspace_source_text: None,
                source_path: None,
            },
        )
        .expect("package should build");
        package.semantic_ir.sections[0].values = vec!["tampered semantic value".to_string()];

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(
            error,
            CoreError::ResolvedContractInconsistent { .. }
        ));
    }

    #[test]
    fn import_mode_binds_modules_to_explicit_worktree_roles() {
        let package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n\n## Worktree Model\n\n- stage-isolated-worktree\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core,ara-cli\n- hardening-role | branch=codex/hardening-role | stages=stage-02 | modules=ara-host-api\n".to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/demo-worktree.md".to_string()),
            },
        )
        .expect("package should build");

        let ara_core = package
            .module_catalog
            .modules
            .iter()
            .find(|module| module.module_id == "ara-core")
            .expect("ara-core module should exist");
        assert_eq!(
            ara_core.preferred_worktree_role.as_deref(),
            Some("foundation-role")
        );
        assert_eq!(
            ara_core.allowed_worktree_roles,
            vec!["foundation-role".to_string()]
        );

        let ara_host_api = package
            .module_catalog
            .modules
            .iter()
            .find(|module| module.module_id == "ara-host-api")
            .expect("ara-host-api module should exist");
        assert_eq!(
            ara_host_api.preferred_worktree_role.as_deref(),
            Some("hardening-role")
        );
        assert_eq!(
            ara_host_api.allowed_worktree_roles,
            vec!["hardening-role".to_string()]
        );
    }

    #[test]
    fn validate_rejects_module_catalog_worktree_binding_drift() {
        let mut package = build_package(
            AuthorMode::ImportBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "external blueprint".to_string(),
                source_text: Some(
                    "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01 | modules=ara-core\n".to_string(),
                ),
                workspace_source_text: None,
                source_path: Some("imports/demo-worktree.md".to_string()),
            },
        )
        .expect("package should build");

        let module = package
            .module_catalog
            .modules
            .iter_mut()
            .find(|module| module.module_id == "ara-core")
            .expect("ara-core module should exist");
        module.preferred_worktree_role = Some("tampered-role".to_string());

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(error, CoreError::ModuleCatalogMismatch { .. }));
    }

    #[test]
    fn update_mode_derives_stage_patch_scope_from_worktree_roles() {
        let package = build_package(
            AuthorMode::UpdateBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "worktree scoped update".to_string(),
                source_text: Some(
                    "# Update Input\n\n## Stage Order\n\n- stage-02: hardening\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01\n- hardening-role | branch=codex/hardening-role | stages=stage-02\n\n## Phases\n\n### stage-02: hardening\n#### Deliverables\n- Hardening bundle\n#### Validation Plan\n- Run hardening validation.\n"
                        .to_string(),
                ),
                workspace_source_text: Some(
                    "# Source File\n\nblueprint/authority/00-authority-root.md\n\n## Purpose\n\nBuild a staged automation tool.\n\n# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01\n- hardening-role | branch=codex/hardening-role | stages=stage-02\n"
                        .to_string(),
                ),
                source_path: Some(
                    "workspace:C:/demo + source:updates/worktree-scoped-update.md".to_string(),
                ),
            },
        )
        .expect("package should build");

        let stage_patch = package
            .patch_plan
            .operations
            .iter()
            .find(|operation| operation.target_id == "stage-02:deliverables")
            .expect("stage deliverables patch should exist");
        assert_eq!(
            stage_patch.affected_paths,
            vec!["blueprint/stages/02-hardening.md".to_string()]
        );
        assert_eq!(
            stage_patch.target_worktree_roles,
            vec!["hardening-role".to_string()]
        );
    }

    #[test]
    fn validate_rejects_patch_plan_worktree_scope_drift() {
        let mut package = build_package(
            AuthorMode::UpdateBlueprint,
            &BlueprintAuthorInput {
                project_name: "demo".to_string(),
                source_summary: "worktree scoped update".to_string(),
                source_text: Some(
                    "# Update Input\n\n## Stage Order\n\n- stage-02: hardening\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01\n- hardening-role | branch=codex/hardening-role | stages=stage-02\n\n## Phases\n\n### stage-02: hardening\n#### Deliverables\n- Hardening bundle\n".to_string(),
                ),
                workspace_source_text: Some(
                    "# Source File\n\nblueprint/authority/00-authority-root.md\n\n## Purpose\n\nBuild a staged automation tool.\n\n# Source File\n\nblueprint/workflow/00-workflow-overview.md\n\n## Stage Order\n\n- stage-01: foundation\n\n## Worktree Roles\n\n- foundation-role | branch=codex/foundation-role | stages=stage-01\n- hardening-role | branch=codex/hardening-role | stages=stage-02\n".to_string(),
                ),
                source_path: Some(
                    "workspace:C:/demo + source:updates/worktree-scoped-update.md".to_string(),
                ),
            },
        )
        .expect("package should build");

        let operation = package
            .patch_plan
            .operations
            .iter_mut()
            .find(|operation| operation.target_id == "stage-02:deliverables")
            .expect("stage deliverables patch should exist");
        operation.affected_paths = vec!["blueprint/authority/00-authority-root.md".to_string()];
        operation.target_worktree_roles = vec!["foundation-role".to_string()];
        let change_report_operation = package
            .change_report
            .patch_operations
            .iter_mut()
            .find(|candidate| candidate.target_id == "stage-02:deliverables")
            .expect("change report stage deliverables patch should exist");
        change_report_operation.affected_paths = operation.affected_paths.clone();
        change_report_operation.target_worktree_roles = operation.target_worktree_roles.clone();

        let error = validate_package(&package).expect_err("validation should fail");
        assert!(matches!(error, CoreError::ChangeReportInconsistent { .. }));
    }
}
