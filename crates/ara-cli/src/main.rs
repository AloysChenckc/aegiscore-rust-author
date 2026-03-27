use ara_core::{
    apply_workspace_patch_plan, build_package, default_contract_root, emit_package,
    migrate_workspace_package, read_workspace_blueprint_bundle_from_defaults, validate_package,
    validate_workspace_package, BlueprintAuthorInput, CoreError,
};
use ara_runtime::{append_log_event, correlation_id, now_utc_rfc3339, read_utf8};
use ara_schemas::{
    AuthorMode, ErrorCode, ErrorEnvelope, LogLevel, StructuredLogEvent,
    ERROR_ENVELOPE_SCHEMA_VERSION, LOG_EVENT_SCHEMA_VERSION,
};
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(name = "ara-cli", version, about = "AegisCore Rust Author CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    New(CommonArgs),
    Import(CommonArgs),
    Update(CommonArgs),
    Recompile(CommonArgs),
    Validate(ValidateArgs),
    ValidateWorkspace(WorkspaceValidateArgs),
    ApplyPatchPlan(WorkspacePatchApplyArgs),
    MigrateWorkspace(WorkspaceMigrateArgs),
    Emit(EmitArgs),
}

#[derive(Debug, Clone, Args)]
struct AuthorInputArgs {
    #[arg(long)]
    project_name: String,
    #[arg(long, default_value = "No source summary provided.")]
    source_summary: String,
    #[arg(long)]
    source_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
struct LoggingArgs {
    #[arg(long)]
    log_path: Option<PathBuf>,
    #[arg(long)]
    correlation_id: Option<String>,
}

#[derive(Debug, Clone, Args)]
struct WorkspaceArgs {
    #[arg(long)]
    workspace: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
struct CommonArgs {
    #[command(flatten)]
    input: AuthorInputArgs,
    #[command(flatten)]
    workspace: WorkspaceArgs,
    #[command(flatten)]
    logging: LoggingArgs,
}

#[derive(Debug, Clone, Args)]
struct ValidateArgs {
    #[arg(long, default_value = "new-project")]
    mode: String,
    #[command(flatten)]
    input: AuthorInputArgs,
    #[command(flatten)]
    workspace: WorkspaceArgs,
    #[command(flatten)]
    logging: LoggingArgs,
}

#[derive(Debug, Clone, Args)]
struct WorkspaceValidateArgs {
    #[arg(long)]
    workspace: PathBuf,
    #[command(flatten)]
    logging: LoggingArgs,
}

#[derive(Debug, Clone, Args)]
struct WorkspacePatchApplyArgs {
    #[arg(long)]
    workspace: PathBuf,
    #[command(flatten)]
    logging: LoggingArgs,
}

#[derive(Debug, Clone, Args)]
struct WorkspaceMigrateArgs {
    #[arg(long)]
    workspace: PathBuf,
    #[command(flatten)]
    logging: LoggingArgs,
}

#[derive(Debug, Clone, Args)]
struct EmitArgs {
    #[arg(long)]
    workspace: PathBuf,
    #[arg(long, default_value = "new-project")]
    mode: String,
    #[command(flatten)]
    input: AuthorInputArgs,
    #[command(flatten)]
    logging: LoggingArgs,
}

#[derive(Debug, Clone)]
struct ExecutionContext {
    command: String,
    mode: Option<AuthorMode>,
    correlation_id: String,
    workspace: Option<String>,
    log_path: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(output) => {
            println!("{output}");
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<String, String> {
    match cli.command {
        Command::New(args) => execute_summary("new", AuthorMode::NewProject, args),
        Command::Import(args) => execute_summary("import", AuthorMode::ImportBlueprint, args),
        Command::Update(args) => execute_summary("update", AuthorMode::UpdateBlueprint, args),
        Command::Recompile(args) => {
            execute_summary("recompile", AuthorMode::RecompileContract, args)
        }
        Command::Validate(args) => execute_validate(args),
        Command::ValidateWorkspace(args) => execute_validate_workspace(args),
        Command::ApplyPatchPlan(args) => execute_apply_patch_plan(args),
        Command::MigrateWorkspace(args) => execute_migrate_workspace(args),
        Command::Emit(args) => execute_emit(args),
    }
}

fn execute_summary(command: &str, mode: AuthorMode, args: CommonArgs) -> Result<String, String> {
    let context = context_from_common(command, mode, &args.workspace, &args.logging);
    log_event(
        &context,
        LogLevel::Info,
        "command_started",
        Some("command-started"),
        None,
        None,
        "authoring command started",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    let input = load_author_input(mode, &args.input, args.workspace.workspace.as_deref())
        .map_err(|error| serialize_core_error(error, &context))?;
    let package =
        build_package(mode, &input).map_err(|error| serialize_core_error(error, &context))?;
    let output =
        serde_json::to_string_pretty(&package_to_output(&package, &context.correlation_id))
            .map_err(|error| {
                serialize_error(
                    ErrorCode::ManifestGenerationFailed,
                    error.to_string(),
                    &context,
                    vec![],
                )
            })?;

    log_event(
        &context,
        LogLevel::Info,
        "command_succeeded",
        Some("package-built"),
        None,
        Some(package.readiness.fingerprint.clone()),
        "authoring command completed",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    Ok(output)
}

fn execute_validate(args: ValidateArgs) -> Result<String, String> {
    let mode = parse_mode(&args.mode).map_err(|error| {
        serialize_error(
            ErrorCode::UnsupportedSourceType,
            error,
            &context_from_logging(
                "validate",
                None,
                &args.logging,
                args.workspace.workspace.as_deref(),
            ),
            Vec::new(),
        )
    })?;
    let context = context_from_logging(
        "validate",
        Some(mode),
        &args.logging,
        args.workspace.workspace.as_deref(),
    );
    log_event(
        &context,
        LogLevel::Info,
        "command_started",
        Some("command-started"),
        None,
        None,
        "validation started",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    let input = load_author_input(mode, &args.input, args.workspace.workspace.as_deref())
        .map_err(|error| serialize_core_error(error, &context))?;
    let package =
        build_package(mode, &input).map_err(|error| serialize_core_error(error, &context))?;
    validate_package(&package).map_err(|error| serialize_core_error(error, &context))?;

    let output = serde_json::json!({
        "status": "ok",
        "mode": "validate",
        "validated_mode": mode.as_str(),
        "correlation_id": context.correlation_id,
    });
    log_event(
        &context,
        LogLevel::Info,
        "command_succeeded",
        Some("package-validated"),
        None,
        Some(package.readiness.fingerprint.clone()),
        "validation completed",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    serde_json::to_string_pretty(&output).map_err(|error| {
        serialize_error(
            ErrorCode::ManifestGenerationFailed,
            error.to_string(),
            &context_from_logging(
                "validate",
                Some(mode),
                &args.logging,
                args.workspace.workspace.as_deref(),
            ),
            Vec::new(),
        )
    })
}

fn execute_validate_workspace(args: WorkspaceValidateArgs) -> Result<String, String> {
    let context = context_from_logging(
        "validate-workspace",
        None,
        &args.logging,
        Some(&args.workspace),
    );
    log_event(
        &context,
        LogLevel::Info,
        "command_started",
        Some("command-started"),
        None,
        None,
        "workspace validation started",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    let package = validate_workspace_package(&args.workspace)
        .map_err(|error| serialize_core_error(error, &context))?;
    let output = serde_json::json!({
        "status": "ok",
        "mode": "validate-workspace",
        "validated_mode": package.mode.as_str(),
        "project_name": package.project_name,
        "correlation_id": context.correlation_id,
    });

    log_event(
        &context,
        LogLevel::Info,
        "command_succeeded",
        Some("workspace-package-validated"),
        Some(".codex/auto-dev/blueprint-manifest.json"),
        Some(package.readiness.fingerprint.clone()),
        "workspace validation completed",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    serde_json::to_string_pretty(&output).map_err(|error| {
        serialize_error(
            ErrorCode::ManifestGenerationFailed,
            error.to_string(),
            &context,
            Vec::new(),
        )
    })
}

fn execute_apply_patch_plan(args: WorkspacePatchApplyArgs) -> Result<String, String> {
    let context = context_from_logging(
        "apply-patch-plan",
        None,
        &args.logging,
        Some(&args.workspace),
    );
    log_event(
        &context,
        LogLevel::Info,
        "command_started",
        Some("command-started"),
        None,
        None,
        "standalone patch apply started",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    let report = apply_workspace_patch_plan(&args.workspace)
        .map_err(|error| serialize_core_error(error, &context))?;
    log_event(
        &context,
        LogLevel::Info,
        "command_succeeded",
        Some("workspace-patch-applied"),
        Some(".codex/auto-dev/patch-execution-report.json"),
        Some(report.replayed_result_fingerprint.clone()),
        "standalone patch apply completed",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    serde_json::to_string_pretty(&report).map_err(|error| {
        serialize_error(
            ErrorCode::ManifestGenerationFailed,
            error.to_string(),
            &context,
            Vec::new(),
        )
    })
}

fn execute_migrate_workspace(args: WorkspaceMigrateArgs) -> Result<String, String> {
    let context = context_from_logging(
        "migrate-workspace",
        None,
        &args.logging,
        Some(&args.workspace),
    );
    log_event(
        &context,
        LogLevel::Info,
        "command_started",
        Some("command-started"),
        None,
        None,
        "workspace migration started",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    let report = migrate_workspace_package(&args.workspace)
        .map_err(|error| serialize_core_error(error, &context))?;
    log_event(
        &context,
        LogLevel::Info,
        "command_succeeded",
        Some("workspace-package-migrated"),
        Some(".codex/auto-dev/migration-report.json"),
        None,
        "workspace migration completed",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    serde_json::to_string_pretty(&report).map_err(|error| {
        serialize_error(
            ErrorCode::ManifestGenerationFailed,
            error.to_string(),
            &context,
            Vec::new(),
        )
    })
}

fn execute_emit(args: EmitArgs) -> Result<String, String> {
    let mode = parse_mode(&args.mode).map_err(|error| {
        serialize_error(
            ErrorCode::UnsupportedSourceType,
            error,
            &context_from_logging("emit", None, &args.logging, Some(&args.workspace)),
            Vec::new(),
        )
    })?;
    let context = context_from_logging("emit", Some(mode), &args.logging, Some(&args.workspace));
    log_event(
        &context,
        LogLevel::Info,
        "command_started",
        Some("command-started"),
        None,
        None,
        "emit started",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    let input = load_author_input(mode, &args.input, Some(&args.workspace))
        .map_err(|error| serialize_core_error(error, &context))?;
    let package =
        build_package(mode, &input).map_err(|error| serialize_core_error(error, &context))?;
    let output = emit_package(&args.workspace, &package)
        .map_err(|error| serialize_core_error(error, &context))?;
    log_event(
        &context,
        LogLevel::Info,
        "command_succeeded",
        Some("package-emitted"),
        Some(".codex/auto-dev/author-report.md"),
        Some(package.readiness.fingerprint.clone()),
        "emit completed",
    )
    .map_err(|error| {
        serialize_error(
            runtime_error_code(&error),
            error.to_string(),
            &context,
            Vec::new(),
        )
    })?;

    serde_json::to_string_pretty(&output).map_err(|error| {
        serialize_error(
            ErrorCode::ManifestGenerationFailed,
            error.to_string(),
            &context,
            Vec::new(),
        )
    })
}

fn load_author_input(
    mode: AuthorMode,
    args: &AuthorInputArgs,
    workspace: Option<&Path>,
) -> Result<BlueprintAuthorInput, CoreError> {
    if matches!(
        mode,
        AuthorMode::UpdateBlueprint | AuthorMode::RecompileContract
    ) && workspace.is_none()
        && args.source_file.is_none()
    {
        return Err(CoreError::MissingWorkspaceBlueprint {
            workspace: "<none>".to_string(),
        });
    }

    let source_file_text = args
        .source_file
        .as_ref()
        .map(|path| read_utf8(path))
        .transpose()?;
    let workspace_text = if matches!(
        mode,
        AuthorMode::UpdateBlueprint | AuthorMode::RecompileContract
    ) {
        workspace
            .map(read_workspace_blueprint_bundle_from_defaults)
            .transpose()
            .map_err(|_| CoreError::MissingWorkspaceBlueprint {
                workspace: workspace
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "<none>".to_string()),
            })?
    } else {
        None
    };
    let source_text = match mode {
        AuthorMode::UpdateBlueprint => source_file_text,
        AuthorMode::RecompileContract => None,
        _ => match (workspace_text.clone(), source_file_text) {
            (Some(existing), Some(delta)) => {
                Some(format!("{existing}\n# Update Input\n\n{delta}\n"))
            }
            (Some(existing), None) => Some(existing),
            (None, Some(delta)) => Some(delta),
            (None, None) => None,
        },
    };
    let source_path = match (&args.source_file, workspace) {
        (Some(path), Some(workspace_root)) => Some(format!(
            "workspace:{} + source:{}",
            workspace_root.display(),
            path.display()
        )),
        (Some(path), None) => Some(path.display().to_string()),
        (None, Some(workspace_root))
            if matches!(
                mode,
                AuthorMode::UpdateBlueprint | AuthorMode::RecompileContract
            ) =>
        {
            Some(format!("workspace:{}", workspace_root.display()))
        }
        _ => None,
    };

    Ok(BlueprintAuthorInput {
        project_name: args.project_name.clone(),
        source_summary: args.source_summary.clone(),
        source_text,
        workspace_source_text: workspace_text,
        source_path,
    })
}

fn context_from_common(
    command: &str,
    mode: AuthorMode,
    workspace: &WorkspaceArgs,
    logging: &LoggingArgs,
) -> ExecutionContext {
    context_from_logging(command, Some(mode), logging, workspace.workspace.as_deref())
}

fn context_from_logging(
    command: &str,
    mode: Option<AuthorMode>,
    logging: &LoggingArgs,
    workspace: Option<&Path>,
) -> ExecutionContext {
    let workspace_string = workspace.map(|path| path.display().to_string());
    let correlation = logging.correlation_id.clone().unwrap_or_else(|| {
        correlation_id(&format!(
            "{}:{}:{}",
            command,
            mode.map(|item| item.as_str()).unwrap_or("none"),
            workspace_string
                .clone()
                .unwrap_or_else(|| "no-workspace".to_string())
        ))
    });
    let log_path = logging.log_path.clone().or_else(|| {
        workspace.and_then(|path| {
            default_contract_root()
                .ok()
                .map(|contract_root| path.join(contract_root).join("ara-events.jsonl"))
        })
    });

    ExecutionContext {
        command: command.to_string(),
        mode,
        correlation_id: correlation,
        workspace: workspace_string,
        log_path,
    }
}

fn package_to_output(
    package: &ara_core::BlueprintPackage,
    correlation_id: &str,
) -> ara_schemas::CliOutput {
    ara_schemas::CliOutput {
        status: "ok".to_string(),
        mode: package.mode.as_str().to_string(),
        project_name: package.project_name.clone(),
        readiness: package.readiness.state.clone(),
        correlation_id: correlation_id.to_string(),
    }
}

fn parse_mode(mode: &str) -> Result<AuthorMode, String> {
    match mode {
        "new-project" => Ok(AuthorMode::NewProject),
        "import-blueprint" => Ok(AuthorMode::ImportBlueprint),
        "update-blueprint" => Ok(AuthorMode::UpdateBlueprint),
        "recompile-contract" => Ok(AuthorMode::RecompileContract),
        other => Err(format!("unsupported mode: {other}")),
    }
}

fn log_event(
    context: &ExecutionContext,
    level: LogLevel,
    event_name: &str,
    decision_code: Option<&str>,
    report_path: Option<&str>,
    fingerprint: Option<String>,
    message: &str,
) -> Result<(), ara_runtime::RuntimeError> {
    if let Some(log_path) = &context.log_path {
        let event = StructuredLogEvent {
            schema_version: LOG_EVENT_SCHEMA_VERSION.to_string(),
            event_name: event_name.to_string(),
            level: level.as_str().to_string(),
            correlation_id: context.correlation_id.clone(),
            workspace: context.workspace.clone().unwrap_or_default(),
            stage: Some("authoring".to_string()),
            loop_id: None,
            command: context.command.clone(),
            decision_code: decision_code.map(str::to_string),
            error_code: None,
            report_path: report_path.map(str::to_string),
            fingerprint,
            message: message.to_string(),
            timestamp_utc: now_utc_rfc3339()?,
        };
        append_log_event(log_path, &event)?;
    }
    Ok(())
}

fn serialize_core_error(error: CoreError, context: &ExecutionContext) -> String {
    serialize_error(
        error.error_code(),
        error.to_string(),
        context,
        error.details(),
    )
}

fn serialize_error(
    code: ErrorCode,
    message: String,
    context: &ExecutionContext,
    details: Vec<String>,
) -> String {
    if let Some(log_path) = &context.log_path {
        let _ = append_log_event(
            log_path,
            &StructuredLogEvent {
                schema_version: LOG_EVENT_SCHEMA_VERSION.to_string(),
                event_name: "command_failed".to_string(),
                level: LogLevel::Error.as_str().to_string(),
                correlation_id: context.correlation_id.clone(),
                workspace: context.workspace.clone().unwrap_or_default(),
                stage: Some("authoring".to_string()),
                loop_id: None,
                command: context.command.clone(),
                decision_code: None,
                error_code: Some(code.as_str().to_string()),
                report_path: None,
                fingerprint: None,
                message: message.clone(),
                timestamp_utc: now_utc_rfc3339()
                    .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string()),
            },
        );
    }

    let envelope = ErrorEnvelope {
        schema_version: ERROR_ENVELOPE_SCHEMA_VERSION.to_string(),
        error_code: code.as_str().to_string(),
        message,
        command: context.command.clone(),
        mode: context.mode.map(|item| item.as_str().to_string()),
        correlation_id: context.correlation_id.clone(),
        details,
    };
    serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| {
        "{\"error_code\":\"ARA-1000\",\"message\":\"failed to serialize error envelope\"}"
            .to_string()
    })
}

fn runtime_error_code(error: &ara_runtime::RuntimeError) -> ErrorCode {
    match error {
        ara_runtime::RuntimeError::MissingParent(_) => ErrorCode::PathResolutionFailed,
        ara_runtime::RuntimeError::MissingInputPath(_) => ErrorCode::UnsupportedSourceType,
        ara_runtime::RuntimeError::Io(_) => ErrorCode::RuntimeFailure,
        ara_runtime::RuntimeError::Json(_) => ErrorCode::ManifestGenerationFailed,
        ara_runtime::RuntimeError::TimeFormat(_) => ErrorCode::RuntimeFailure,
    }
}
