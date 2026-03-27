use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: &str = "ara.v1";
pub const MODULE_CATALOG_SCHEMA_VERSION: &str = "ara.module-catalog.v2";
pub const BLUEPRINT_MANIFEST_SCHEMA_VERSION: &str = "ara.blueprint-manifest.v1";
pub const WORKTREE_PROTOCOL_SCHEMA_VERSION: &str = "ara.worktree-protocol.v1";
pub const SEMANTIC_IR_SCHEMA_VERSION: &str = "ara.semantic-ir.v9";
pub const NORMALIZATION_REPORT_SCHEMA_VERSION: &str = "ara.normalization-report.v4";
pub const DECISION_SUMMARY_SCHEMA_VERSION: &str = "ara.decision-summary.v3";
pub const AGENT_BRIEF_SCHEMA_VERSION: &str = "ara.agent-brief.v1";
pub const CHANGE_REPORT_SCHEMA_VERSION: &str = "ara.change-report.v3";
pub const PATCH_BASE_SCHEMA_VERSION: &str = "ara.patch-base.v1";
pub const PATCH_PLAN_SCHEMA_VERSION: &str = "ara.patch-plan.v4";
pub const PATCH_EXECUTION_REPORT_SCHEMA_VERSION: &str = "ara.patch-execution-report.v3";
pub const MIGRATION_REPORT_SCHEMA_VERSION: &str = "ara.migration-report.v1";
pub const RESOLVED_CONTRACT_SCHEMA_VERSION: &str = "ara.resolved-contract.v2";
pub const READINESS_SCHEMA_VERSION: &str = "ara.readiness.v3";
pub const TASK_PROGRESS_SCHEMA_VERSION: &str = "ara.task-progress.v1";
pub const ERROR_ENVELOPE_SCHEMA_VERSION: &str = "ara.error-envelope.v1";
pub const LOG_EVENT_SCHEMA_VERSION: &str = "ara.log-event.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorMode {
    NewProject,
    ImportBlueprint,
    UpdateBlueprint,
    RecompileContract,
}

impl AuthorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NewProject => "new-project",
            Self::ImportBlueprint => "import-blueprint",
            Self::UpdateBlueprint => "update-blueprint",
            Self::RecompileContract => "recompile-contract",
        }
    }

    pub fn source_type(self) -> &'static str {
        match self {
            Self::NewProject => "product-idea",
            Self::ImportBlueprint => "external-blueprint",
            Self::UpdateBlueprint => "existing-blueprint",
            Self::RecompileContract => "existing-contract",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadinessState {
    Draft,
    Normalized,
    ContractValid,
    CandidateForBlueprintGate,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStageStatus {
    Pending,
    InProgress,
    Completed,
}

impl TaskStageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in-progress",
            Self::Completed => "completed",
        }
    }
}

impl ReadinessState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Normalized => "normalized",
            Self::ContractValid => "contract-valid",
            Self::CandidateForBlueprintGate => "candidate-for-blueprint-gate",
            Self::Stale => "stale",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleLanguage {
    Rust,
    Powershell,
    Sh,
    Bat,
    Python,
    Nodejs,
    Markdown,
    Toml,
    Json,
    Yaml,
}

impl ModuleLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Powershell => "powershell",
            Self::Sh => "sh",
            Self::Bat => "bat",
            Self::Python => "python",
            Self::Nodejs => "nodejs",
            Self::Markdown => "markdown",
            Self::Toml => "toml",
            Self::Json => "json",
            Self::Yaml => "yaml",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    RuntimeFailure,
    PathResolutionFailed,
    AtomicWriteFailed,
    UnsupportedHostPlatform,
    UnsupportedSourceType,
    BlueprintSchemaIncomplete,
    ContractTomlInvalid,
    ResolvedContractInconsistent,
    StageGraphInvalid,
    SchemaVersionMismatch,
    AuthorityRewriteRequired,
    ModuleLanguagePolicyViolation,
    ExternalBlueprintTooAmbiguous,
    RequiredOutputMissing,
    ReadinessCalculationFailed,
    PackageNotReadyForBlueprintGate,
    PackageFingerprintStale,
    ManifestGenerationFailed,
    NormalizationEvidenceMissing,
    ArtifactFingerprintMismatch,
    ChangeReportInvalid,
    WrapperCouldNotLocateCli,
    WrapperInvocationUnsupported,
    HostApiBridgeFailed,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeFailure => "ARA-1000",
            Self::PathResolutionFailed => "ARA-1001",
            Self::AtomicWriteFailed => "ARA-1002",
            Self::UnsupportedHostPlatform => "ARA-1003",
            Self::UnsupportedSourceType => "ARA-2000",
            Self::BlueprintSchemaIncomplete => "ARA-2001",
            Self::ContractTomlInvalid => "ARA-2002",
            Self::ResolvedContractInconsistent => "ARA-2003",
            Self::StageGraphInvalid => "ARA-2004",
            Self::SchemaVersionMismatch => "ARA-2005",
            Self::AuthorityRewriteRequired => "ARA-3000",
            Self::ModuleLanguagePolicyViolation => "ARA-3001",
            Self::ExternalBlueprintTooAmbiguous => "ARA-3002",
            Self::RequiredOutputMissing => "ARA-3003",
            Self::ReadinessCalculationFailed => "ARA-4000",
            Self::PackageNotReadyForBlueprintGate => "ARA-4001",
            Self::PackageFingerprintStale => "ARA-4002",
            Self::ManifestGenerationFailed => "ARA-5000",
            Self::NormalizationEvidenceMissing => "ARA-5001",
            Self::ArtifactFingerprintMismatch => "ARA-5002",
            Self::ChangeReportInvalid => "ARA-5003",
            Self::WrapperCouldNotLocateCli => "ARA-6000",
            Self::WrapperInvocationUnsupported => "ARA-6001",
            Self::HostApiBridgeFailed => "ARA-6002",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleSpec {
    pub module_id: String,
    pub layer: String,
    pub responsibility: String,
    pub recommended_language: ModuleLanguage,
    pub allowed_languages: Vec<ModuleLanguage>,
    pub forbidden_languages: Vec<ModuleLanguage>,
    pub reason: String,
    pub hot_path: bool,
    pub cross_platform_requirement: String,
    pub boundary_type: String,
    pub owned_artifacts: Vec<String>,
    #[serde(default)]
    pub preferred_worktree_role: Option<String>,
    #[serde(default)]
    pub allowed_worktree_roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleCatalog {
    pub schema_version: String,
    pub modules: Vec<ModuleSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeRoleSpec {
    pub role_id: String,
    pub branch_pattern: String,
    #[serde(default)]
    pub stage_ids: Vec<String>,
    #[serde(default)]
    pub module_ids: Vec<String>,
    #[serde(default)]
    pub exclusive_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeProtocol {
    pub schema_version: String,
    pub model: String,
    #[serde(default)]
    pub parallel_worktree_policy: Vec<String>,
    #[serde(default)]
    pub shared_authority_paths: Vec<String>,
    #[serde(default)]
    pub sync_rule: Vec<String>,
    #[serde(default)]
    pub merge_back_rule: Vec<String>,
    #[serde(default)]
    pub cleanup_rule: Vec<String>,
    #[serde(default)]
    pub roles: Vec<WorktreeRoleSpec>,
}

impl Default for WorktreeProtocol {
    fn default() -> Self {
        Self {
            schema_version: String::new(),
            model: String::new(),
            parallel_worktree_policy: Vec::new(),
            shared_authority_paths: Vec::new(),
            sync_rule: Vec::new(),
            merge_back_rule: Vec::new(),
            cleanup_rule: Vec::new(),
            roles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintManifestEntry {
    pub path: String,
    pub doc_role: String,
    pub canonical_id: String,
    pub source_provenance: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintManifest {
    pub schema_version: String,
    pub files: Vec<BlueprintManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizationReport {
    pub schema_version: String,
    pub source_type: String,
    pub source_files: Vec<String>,
    pub preserved_sections: Vec<String>,
    pub inferred_sections: Vec<String>,
    pub dropped_sections: Vec<String>,
    #[serde(default)]
    pub semantic_hints: Vec<String>,
    #[serde(default)]
    pub semantic_risks: Vec<String>,
    #[serde(default)]
    pub semantic_conflicts: Vec<SemanticConflict>,
    pub unresolved_ambiguities: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeOperation {
    pub target_kind: String,
    pub target_id: String,
    pub action: String,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchOperation {
    pub scope: String,
    pub target_id: String,
    pub strategy: String,
    pub status: String,
    pub details: String,
    #[serde(default)]
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub target_worktree_roles: Vec<String>,
    #[serde(default)]
    pub value_lines: Vec<String>,
    #[serde(default)]
    pub stage_name: Option<String>,
    #[serde(default)]
    pub previous_value_lines: Vec<String>,
    #[serde(default)]
    pub previous_stage_name: Option<String>,
    #[serde(default)]
    pub reverse_strategy: Option<String>,
    #[serde(default)]
    pub strategy_metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeReport {
    pub schema_version: String,
    pub mode: String,
    pub operation_count: usize,
    pub conflict_count: usize,
    pub operations: Vec<ChangeOperation>,
    #[serde(default)]
    pub patch_operation_count: usize,
    #[serde(default)]
    pub patch_operations: Vec<PatchOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchPlan {
    pub schema_version: String,
    pub mode: String,
    pub operation_count: usize,
    pub conflict_count: usize,
    #[serde(default)]
    pub base_fingerprint: String,
    #[serde(default)]
    pub result_fingerprint: String,
    pub operations: Vec<PatchOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchBase {
    pub schema_version: String,
    pub mode: String,
    #[serde(default)]
    pub artifact_status: String,
    #[serde(default)]
    pub base_fingerprint: String,
    #[serde(default)]
    pub sections: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub stage_sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchExecutionReport {
    pub schema_version: String,
    pub mode: String,
    #[serde(default)]
    pub base_fingerprint: String,
    #[serde(default)]
    pub expected_result_fingerprint: String,
    #[serde(default)]
    pub replayed_result_fingerprint: String,
    pub replay_status: String,
    pub operation_count: usize,
    pub applied_operation_count: usize,
    pub mismatch_count: usize,
    #[serde(default)]
    pub mismatches: Vec<String>,
    #[serde(default)]
    pub reverse_replayed_base_fingerprint: String,
    #[serde(default)]
    pub reversibility_status: String,
    #[serde(default)]
    pub reverse_mismatch_count: usize,
    #[serde(default)]
    pub reverse_mismatches: Vec<String>,
    #[serde(default)]
    pub scope_validation_status: String,
    #[serde(default)]
    pub scope_mismatch_count: usize,
    #[serde(default)]
    pub scope_mismatches: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticSection {
    pub key: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticFrame {
    pub scope: String,
    pub canonical_section: String,
    pub source_label: String,
    pub source_locator: String,
    pub origin_kind: String,
    pub confidence: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticCluster {
    pub scope: String,
    pub canonical_section: String,
    pub source_labels: Vec<String>,
    pub source_locators: Vec<String>,
    pub origin_kinds: Vec<String>,
    pub confidence_levels: Vec<String>,
    pub merged_values: Vec<String>,
    pub merge_pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticConflictSourceGroup {
    pub source_group: String,
    pub source_labels: Vec<String>,
    pub source_locators: Vec<String>,
    pub confidence_levels: Vec<String>,
    pub merged_values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticConflict {
    pub scope: String,
    pub canonical_section: String,
    pub conflict_kind: String,
    pub source_groups: Vec<SemanticConflictSourceGroup>,
    pub confidence_profile: Vec<String>,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub blocking: bool,
    #[serde(default)]
    pub review_required: bool,
    #[serde(default)]
    pub recommended_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionSummaryEntry {
    pub kind: String,
    pub scope: String,
    pub target_id: String,
    pub severity: String,
    pub blocking: bool,
    pub review_required: bool,
    #[serde(default)]
    pub worktree_roles: Vec<String>,
    pub summary: String,
    pub recommended_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticStage {
    pub stage_id: String,
    pub stage_name: String,
    pub sections: Vec<SemanticSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticIr {
    pub schema_version: String,
    pub mode: String,
    pub project_name: String,
    pub source_type: String,
    pub source_provenance: String,
    #[serde(default)]
    pub derivation: String,
    #[serde(default)]
    pub source_files: Vec<String>,
    #[serde(default)]
    pub source_fingerprint: String,
    #[serde(default)]
    pub projection_fingerprint: String,
    #[serde(default)]
    pub normalized_sections: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub normalized_stage_sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    #[serde(default)]
    pub normalized_section_origins: BTreeMap<String, String>,
    #[serde(default)]
    pub normalized_stage_section_origins: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub semantic_frames: Vec<SemanticFrame>,
    #[serde(default)]
    pub semantic_clusters: Vec<SemanticCluster>,
    pub sections: Vec<SemanticSection>,
    pub stages: Vec<SemanticStage>,
    #[serde(default)]
    pub preserved_sections: Vec<String>,
    #[serde(default)]
    pub inferred_sections: Vec<String>,
    #[serde(default)]
    pub semantic_hints: Vec<String>,
    #[serde(default)]
    pub semantic_risks: Vec<String>,
    #[serde(default)]
    pub semantic_conflicts: Vec<SemanticConflict>,
    pub unresolved_ambiguities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationArtifact {
    pub artifact: String,
    pub path: String,
    pub previous_schema_version: String,
    pub current_schema_version: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationReport {
    pub schema_version: String,
    pub mode: String,
    pub workspace: String,
    pub migrated_artifacts: usize,
    pub unchanged_artifacts: usize,
    pub artifacts: Vec<MigrationArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractPaths {
    pub blueprint_root: String,
    pub authority_root: String,
    pub workflow_root: String,
    pub stages_root: String,
    pub modules_root: String,
    pub contract_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractStage {
    pub stage_id: String,
    pub stage_name: String,
    pub default_next_goal: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedContract {
    pub schema_version: String,
    pub project_name: String,
    pub workflow_mode: String,
    pub paths: ContractPaths,
    pub stages: Vec<ContractStage>,
    #[serde(default)]
    pub worktree: WorktreeProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessReport {
    pub schema_version: String,
    pub state: String,
    pub reason: String,
    pub fingerprint: String,
    #[serde(default)]
    pub blocking_semantic_conflict_count: usize,
    #[serde(default)]
    pub review_required_semantic_conflict_count: usize,
    #[serde(default)]
    pub high_risk_patch_operation_count: usize,
    #[serde(default)]
    pub review_required_patch_operation_count: usize,
    #[serde(default)]
    pub gate_holds: Vec<String>,
    #[serde(default)]
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionSummary {
    pub schema_version: String,
    pub mode: String,
    pub readiness_state: String,
    pub reason: String,
    pub blocking: bool,
    pub review_required: bool,
    #[serde(default)]
    pub blocking_semantic_conflict_count: usize,
    #[serde(default)]
    pub review_required_semantic_conflict_count: usize,
    #[serde(default)]
    pub high_risk_patch_operation_count: usize,
    #[serde(default)]
    pub review_required_patch_operation_count: usize,
    #[serde(default)]
    pub gate_hold_count: usize,
    #[serde(default)]
    pub recommended_action_count: usize,
    #[serde(default)]
    pub blocking_kinds: Vec<String>,
    #[serde(default)]
    pub review_required_kinds: Vec<String>,
    #[serde(default)]
    pub blocking_kind_counts: BTreeMap<String, usize>,
    #[serde(default)]
    pub review_required_kind_counts: BTreeMap<String, usize>,
    #[serde(default)]
    pub primary_blocker_kind: Option<String>,
    #[serde(default)]
    pub primary_blocker_scope: Option<String>,
    #[serde(default)]
    pub primary_blocker_target_id: Option<String>,
    #[serde(default)]
    pub primary_blocker_summary: Option<String>,
    #[serde(default)]
    pub primary_recommended_action: Option<String>,
    #[serde(default)]
    pub top_blockers: Vec<DecisionSummaryEntry>,
    #[serde(default)]
    pub top_review_items: Vec<DecisionSummaryEntry>,
    #[serde(default)]
    pub scoped_worktree_roles: Vec<String>,
    #[serde(default)]
    pub entries: Vec<DecisionSummaryEntry>,
    #[serde(default)]
    pub gate_holds: Vec<String>,
    #[serde(default)]
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentBrief {
    pub schema_version: String,
    pub project_name: String,
    pub mode: String,
    pub readiness_state: String,
    pub reason: String,
    #[serde(default)]
    pub current_stage_id: Option<String>,
    #[serde(default)]
    pub current_stage_name: Option<String>,
    pub total_stages: usize,
    pub completed_stages: usize,
    pub overall_progress_percent: u8,
    pub blocking: bool,
    pub review_required: bool,
    #[serde(default)]
    pub primary_blocker_kind: Option<String>,
    #[serde(default)]
    pub primary_blocker_scope: Option<String>,
    #[serde(default)]
    pub primary_blocker_target_id: Option<String>,
    #[serde(default)]
    pub primary_blocker_summary: Option<String>,
    #[serde(default)]
    pub primary_recommended_action: Option<String>,
    #[serde(default)]
    pub top_blockers: Vec<DecisionSummaryEntry>,
    #[serde(default)]
    pub top_review_items: Vec<DecisionSummaryEntry>,
    #[serde(default)]
    pub scoped_worktree_roles: Vec<String>,
    #[serde(default)]
    pub gate_holds: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProgressStage {
    pub stage_id: String,
    pub stage_name: String,
    pub status: String,
    pub stage_progress_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProgressReport {
    pub schema_version: String,
    pub total_stages: usize,
    pub completed_stages: usize,
    pub current_stage_id: Option<String>,
    pub overall_progress_percent: u8,
    pub stages: Vec<TaskProgressStage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliOutput {
    pub status: String,
    pub mode: String,
    pub project_name: String,
    pub readiness: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub schema_version: String,
    pub error_code: String,
    pub message: String,
    pub command: String,
    pub mode: Option<String>,
    pub correlation_id: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredLogEvent {
    pub schema_version: String,
    pub event_name: String,
    pub level: String,
    pub correlation_id: String,
    pub workspace: String,
    pub stage: Option<String>,
    pub loop_id: Option<String>,
    pub command: String,
    pub decision_code: Option<String>,
    pub error_code: Option<String>,
    pub report_path: Option<String>,
    pub fingerprint: Option<String>,
    pub message: String,
    pub timestamp_utc: String,
}
