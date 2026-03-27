use ara_core::{
    apply_workspace_patch_plan, build_package, emit_package, load_workspace_package,
    migrate_workspace_package, read_workspace_blueprint_bundle_from_defaults, validate_package,
    validate_workspace_package, BlueprintAuthorInput, BlueprintPackage, CoreError,
};
use ara_schemas::{AuthorMode, CliOutput, MigrationReport, PatchExecutionReport};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorRequest {
    pub project_name: String,
    pub source_summary: String,
    pub source_text: Option<String>,
    pub workspace_source_text: Option<String>,
    pub source_path: Option<String>,
}

impl AuthorRequest {
    pub fn new(project_name: impl Into<String>, source_summary: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            source_summary: source_summary.into(),
            source_text: None,
            workspace_source_text: None,
            source_path: None,
        }
    }

    pub fn with_source_text(mut self, source_text: impl Into<String>) -> Self {
        self.source_text = Some(source_text.into());
        self
    }

    pub fn with_workspace_source_text(mut self, workspace_source_text: impl Into<String>) -> Self {
        self.workspace_source_text = Some(workspace_source_text.into());
        self
    }

    pub fn with_source_path(mut self, source_path: impl Into<String>) -> Self {
        self.source_path = Some(source_path.into());
        self
    }

    pub fn with_workspace_bundle(
        mut self,
        workspace_root: &Path,
        source_path: Option<impl Into<String>>,
    ) -> Result<Self, CoreError> {
        self.workspace_source_text = Some(read_workspace_blueprint_bundle_from_defaults(
            workspace_root,
        )?);
        self.source_path = Some(match source_path {
            Some(value) => value.into(),
            None => format!("workspace:{}", workspace_root.display()),
        });
        Ok(self)
    }

    fn into_core_input(self) -> BlueprintAuthorInput {
        BlueprintAuthorInput {
            project_name: self.project_name,
            source_summary: self.source_summary,
            source_text: self.source_text,
            workspace_source_text: self.workspace_source_text,
            source_path: self.source_path,
        }
    }
}

pub fn build_blueprint_package(
    mode: AuthorMode,
    project_name: impl Into<String>,
    source_summary: impl Into<String>,
) -> Result<BlueprintPackage, CoreError> {
    build_blueprint_package_from_request(mode, AuthorRequest::new(project_name, source_summary))
}

pub fn build_blueprint_package_from_request(
    mode: AuthorMode,
    request: AuthorRequest,
) -> Result<BlueprintPackage, CoreError> {
    build_package(mode, &request.into_core_input())
}

pub fn validate_blueprint_request(
    mode: AuthorMode,
    request: AuthorRequest,
) -> Result<BlueprintPackage, CoreError> {
    let package = build_blueprint_package_from_request(mode, request)?;
    validate_package(&package)?;
    Ok(package)
}

pub fn emit_blueprint_package(
    workspace_root: &Path,
    mode: AuthorMode,
    project_name: impl Into<String>,
    source_summary: impl Into<String>,
) -> Result<CliOutput, CoreError> {
    emit_blueprint_package_from_request(
        workspace_root,
        mode,
        AuthorRequest::new(project_name, source_summary),
    )
}

pub fn emit_blueprint_package_from_request(
    workspace_root: &Path,
    mode: AuthorMode,
    request: AuthorRequest,
) -> Result<CliOutput, CoreError> {
    let package = validate_blueprint_request(mode, request)?;
    emit_package(workspace_root, &package)
}

pub fn load_workspace_blueprint_package(
    workspace_root: &Path,
) -> Result<BlueprintPackage, CoreError> {
    load_workspace_package(workspace_root)
}

pub fn validate_workspace_blueprint_package(
    workspace_root: &Path,
) -> Result<BlueprintPackage, CoreError> {
    validate_workspace_package(workspace_root)
}

pub fn migrate_workspace_blueprint_package(
    workspace_root: &Path,
) -> Result<MigrationReport, CoreError> {
    migrate_workspace_package(workspace_root)
}

pub fn apply_workspace_blueprint_patch_plan(
    workspace_root: &Path,
) -> Result<PatchExecutionReport, CoreError> {
    apply_workspace_patch_plan(workspace_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn request_can_build_import_package_with_source_text() {
        let request = AuthorRequest::new("demo", "external blueprint").with_source_text(
            "# External Blueprint\n\n## Purpose\n\nBuild a staged automation tool.\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n",
        );

        let package = build_blueprint_package_from_request(AuthorMode::ImportBlueprint, request)
            .expect("package should build");

        assert_eq!(package.resolved_contract.stages.len(), 2);
        assert_eq!(package.stage_documents.len(), 2);
    }

    #[test]
    fn request_can_load_workspace_bundle_and_recompile() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-host-api-{unique}"));
        fs::create_dir_all(workspace.join("blueprint/stages")).expect("create temp workspace");
        fs::write(
            workspace.join("blueprint/stages/01-foundation.md"),
            "# Stage Document\n\n## Stage\n\n- `stage_id`: `stage-01`\n- `stage_name`: `foundation`\n",
        )
        .expect("write stage document");

        let request = AuthorRequest::new("demo", "workspace recompile")
            .with_workspace_bundle(&workspace, None::<String>)
            .expect("workspace bundle should load");
        let package = validate_blueprint_request(AuthorMode::RecompileContract, request)
            .expect("recompile request should validate");

        assert_eq!(package.resolved_contract.stages.len(), 1);
        assert_eq!(package.resolved_contract.stages[0].stage_id, "stage-01");

        if workspace.exists() {
            fs::remove_dir_all(workspace).expect("cleanup temp workspace");
        }
    }

    #[test]
    fn emit_from_request_writes_outputs() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-host-api-emit-{unique}"));
        let request = AuthorRequest::new("demo", "generated package");

        let output =
            emit_blueprint_package_from_request(&workspace, AuthorMode::NewProject, request)
                .expect("emit should succeed");

        assert_eq!(output.status, "ok");
        assert!(workspace.join(".codex/auto-dev/readiness.json").exists());
        assert!(workspace.join(".codex/auto-dev/decision-summary.json").exists());
        assert!(workspace.join(".codex/auto-dev/agent-brief.json").exists());

        if workspace.exists() {
            fs::remove_dir_all(workspace).expect("cleanup temp workspace");
        }
    }

    #[test]
    fn workspace_package_can_be_loaded_and_validated_after_emit() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-host-api-workspace-{unique}"));
        let request = AuthorRequest::new("demo", "generated package");

        emit_blueprint_package_from_request(&workspace, AuthorMode::NewProject, request)
            .expect("emit should succeed");

        let loaded =
            load_workspace_blueprint_package(&workspace).expect("workspace package should load");
        let validated = validate_workspace_blueprint_package(&workspace)
            .expect("workspace package should validate");

        assert_eq!(loaded.project_name, "demo");
        assert_eq!(validated.readiness.state, "candidate-for-blueprint-gate");

        if workspace.exists() {
            fs::remove_dir_all(workspace).expect("cleanup temp workspace");
        }
    }

    #[test]
    fn workspace_package_can_be_migrated_after_schema_drift() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-host-api-migrate-{unique}"));
        let request = AuthorRequest::new("demo", "generated package");

        emit_blueprint_package_from_request(&workspace, AuthorMode::NewProject, request)
            .expect("emit should succeed");

        let resolved_contract_path = workspace.join(".codex/auto-dev/resolved-contract.json");
        let mut resolved_contract: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&resolved_contract_path).expect("read resolved contract"),
        )
        .expect("parse resolved contract");
        resolved_contract["schema_version"] =
            serde_json::Value::String("ara.resolved-contract.v0".to_string());
        fs::write(
            &resolved_contract_path,
            serde_json::to_string_pretty(&resolved_contract).expect("serialize resolved contract")
                + "\n",
        )
        .expect("write drifted resolved contract");

        let migration_report = migrate_workspace_blueprint_package(&workspace)
            .expect("workspace migration should succeed");
        assert!(migration_report
            .artifacts
            .iter()
            .any(|artifact| artifact.artifact == "resolved-contract"
                && artifact.action == "migrated"));

        let validated = validate_workspace_blueprint_package(&workspace)
            .expect("workspace package should validate");
        assert_eq!(
            validated.resolved_contract.schema_version,
            "ara.resolved-contract.v2"
        );

        if workspace.exists() {
            fs::remove_dir_all(workspace).expect("cleanup temp workspace");
        }
    }

    #[test]
    fn workspace_patch_plan_can_be_applied_after_update_emit() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-host-api-apply-patch-{unique}"));
        let initial_request = AuthorRequest::new("demo", "initial blueprint")
            .with_source_text("# Input\n\n## Stage Order\n\n- stage-01: foundation\n");
        emit_blueprint_package_from_request(&workspace, AuthorMode::NewProject, initial_request)
            .expect("initial emit should succeed");

        let update_source = workspace.join("update.md");
        fs::write(
            &update_source,
            "# Update Input\n\n## Stage Order\n\n- stage-01: foundation\n- stage-02: hardening\n",
        )
        .expect("write update input");
        let update_request = AuthorRequest::new("demo", "updated blueprint")
            .with_source_text(
                fs::read_to_string(&update_source).expect("read update input for request"),
            )
            .with_workspace_bundle(&workspace, Some(update_source.display().to_string()))
            .expect("workspace bundle should load");
        emit_blueprint_package_from_request(
            &workspace,
            AuthorMode::UpdateBlueprint,
            update_request,
        )
        .expect("update emit should succeed");

        let report = apply_workspace_blueprint_patch_plan(&workspace)
            .expect("standalone patch apply should succeed");
        assert_eq!(report.replay_status, "replayed", "{report:?}");
        assert_eq!(report.applied_operation_count, report.operation_count);

        if workspace.exists() {
            fs::remove_dir_all(workspace).expect("cleanup temp workspace");
        }
    }
}
