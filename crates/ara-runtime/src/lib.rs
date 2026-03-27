use ara_schemas::StructuredLogEvent;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("parent path is missing for write target: {0}")]
    MissingParent(String),
    #[error("input path does not exist: {0}")]
    MissingInputPath(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("time formatting error: {0}")]
    TimeFormat(#[from] time::error::Format),
}

pub fn normalize_repo_relative_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

pub fn fingerprint_text(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn correlation_id(seed: &str) -> String {
    format!("ara-{}", fingerprint_text(seed))
}

pub fn now_utc_rfc3339() -> Result<String, RuntimeError> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

pub fn to_pretty_json<T: Serialize>(value: &T) -> Result<String, RuntimeError> {
    Ok(serde_json::to_string_pretty(value)? + "\n")
}

pub fn read_utf8(path: &Path) -> Result<String, RuntimeError> {
    if !path.exists() {
        return Err(RuntimeError::MissingInputPath(path.display().to_string()));
    }
    let contents = fs::read_to_string(path)?;
    Ok(contents
        .strip_prefix('\u{feff}')
        .unwrap_or(&contents)
        .to_string())
}

pub fn read_workspace_blueprint_bundle_with_roots(
    workspace_root: &Path,
    blueprint_roots: &[String],
) -> Result<String, RuntimeError> {
    let blueprint_roots = blueprint_roots
        .iter()
        .map(|root| workspace_root.join(root))
        .collect::<Vec<_>>();

    let mut files = Vec::new();
    for root in blueprint_roots {
        if root.exists() {
            for entry in fs::read_dir(root)? {
                let path = entry?.path();
                if path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
                {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    if files.is_empty() {
        return Err(RuntimeError::MissingInputPath(
            workspace_root.display().to_string(),
        ));
    }

    let mut bundle = String::new();
    for path in files {
        let relative = path
            .strip_prefix(workspace_root)
            .unwrap_or(path.as_path())
            .display()
            .to_string();
        bundle.push_str("# Source File\n\n");
        bundle.push_str(&normalize_repo_relative_path(&relative));
        bundle.push_str("\n\n");
        bundle.push_str(&fs::read_to_string(&path)?);
        bundle.push_str("\n\n");
    }

    Ok(bundle)
}

pub fn read_workspace_blueprint_bundle(workspace_root: &Path) -> Result<String, RuntimeError> {
    read_workspace_blueprint_bundle_with_roots(
        workspace_root,
        &[
            "blueprint/authority".to_string(),
            "blueprint/workflow".to_string(),
            "blueprint/stages".to_string(),
        ],
    )
}

pub fn write_utf8_atomic(path: &Path, contents: &str) -> Result<(), RuntimeError> {
    let parent = path
        .parent()
        .ok_or_else(|| RuntimeError::MissingParent(path.display().to_string()))?;
    fs::create_dir_all(parent)?;

    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, contents)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temp_path, path)?;
    Ok(())
}

pub fn append_log_event(path: &Path, event: &StructuredLogEvent) -> Result<(), RuntimeError> {
    let parent = path
        .parent()
        .ok_or_else(|| RuntimeError::MissingParent(path.display().to_string()))?;
    fs::create_dir_all(parent)?;

    let payload = serde_json::to_string(event)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{payload}")?;
    Ok(())
}

pub fn prune_directory_files(
    workspace_root: &Path,
    relative_root: &str,
    keep_relative_paths: &[String],
) -> Result<(), RuntimeError> {
    let target_root = workspace_root.join(relative_root);
    if !target_root.exists() {
        return Ok(());
    }

    let keep = keep_relative_paths
        .iter()
        .map(|path| normalize_repo_relative_path(path))
        .collect::<BTreeSet<_>>();

    for entry in fs::read_dir(&target_root)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            continue;
        }

        let relative = path
            .strip_prefix(workspace_root)
            .unwrap_or(path.as_path())
            .display()
            .to_string();
        let normalized = normalize_repo_relative_path(&relative);
        if !keep.contains(&normalized) {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn prune_directory_files_removes_only_unlisted_markdown_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-runtime-prune-{unique}"));
        let stages_dir = workspace.join("blueprint/stages");
        fs::create_dir_all(&stages_dir).expect("create temp stages dir");

        let keep_file = stages_dir.join("01-foundation.md");
        let stale_file = stages_dir.join("02-hardening.md");
        let sidecar_file = stages_dir.join("notes.txt");
        fs::write(&keep_file, "keep").expect("write keep file");
        fs::write(&stale_file, "stale").expect("write stale file");
        fs::write(&sidecar_file, "notes").expect("write sidecar file");

        prune_directory_files(
            &workspace,
            "blueprint/stages",
            &[String::from("blueprint/stages/01-foundation.md")],
        )
        .expect("prune should succeed");

        assert!(
            keep_file.exists(),
            "listed markdown file should be preserved"
        );
        assert!(
            !stale_file.exists(),
            "unlisted markdown file should be removed"
        );
        assert!(
            sidecar_file.exists(),
            "non-markdown sidecar file should be preserved"
        );

        if workspace.exists() {
            fs::remove_dir_all(workspace).expect("cleanup temp workspace");
        }
    }

    #[test]
    fn read_utf8_strips_utf8_bom() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("ara-runtime-bom-{unique}"));
        fs::create_dir_all(&workspace).expect("create temp workspace");
        let file_path = workspace.join("bom.json");
        fs::write(&file_path, "\u{feff}{\"ok\":true}").expect("write bom file");

        let contents = read_utf8(&file_path).expect("read utf8 should succeed");

        assert_eq!(contents, "{\"ok\":true}");

        if workspace.exists() {
            fs::remove_dir_all(workspace).expect("cleanup temp workspace");
        }
    }
}
