//! Folder listings for the in-app file chooser (ADR 0020).

use crate::{StorageRoot, args};
use impulcifer_types::ipc::{self, ErrorCode};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const ENTRY_LIMIT: usize = 2000;

#[derive(Serialize)]
struct Entry {
    name: String,
    path: PathBuf,
    dir: bool,
    size: Option<u64>,
    modified_ms: Option<u64>,
}

pub(crate) fn list(request: &Value, roots: &[StorageRoot]) -> Result<Value, Value> {
    if !request.is_null() && !request.is_object() {
        return Err(args::invalid("Request must be an object."));
    }
    let text = |key: &str| match &request[key] {
        Value::Null => Ok(None),
        Value::String(value) => Ok(Some(value.as_str())),
        _ => Err(args::invalid(format!("{key} must be a string."))),
    };
    let kind = text("kind")?.unwrap_or("dir");
    let extensions: &[&str] = match kind {
        "dir" => &[],
        "audio" => &["wav", "mlp", "thd", "truehd"],
        "text" => &["csv", "txt"],
        "wav" => &["wav"],
        "measurement" => &["mdat", "liveproject"],
        _ => return Err(args::invalid("Unsupported file kind.")),
    };
    let path = text("path")?
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| roots.first().map(|root| PathBuf::from(&root.path)))
        .or_else(std::env::home_dir)
        .unwrap_or_default();
    // Strip only trailing separators, retaining the requested spelling, including
    // symlinks and dot components. A filesystem root retains its separator.
    let raw = path.to_string_lossy();
    let trimmed = raw.trim_end_matches(std::path::is_separator);
    let path = if path.parent().is_some() {
        PathBuf::from(trimmed)
    } else {
        path
    };
    let missing = || {
        ipc::error(
            ErrorCode::FileNotFound,
            "Folder does not exist.",
            json!({"path":path}),
            false,
        )
    };
    if !path.is_absolute() {
        return Err(missing());
    }
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => {}
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(read_error(&path, error));
        }
        _ => return Err(missing()),
    }
    let directory = std::fs::read_dir(&path).map_err(|error| read_error(&path, error))?;
    let mut entries = Vec::new();
    for entry in directory {
        let entry = entry.map_err(|error| read_error(&path, error))?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let full_path = entry.path();
        let Ok(metadata) = std::fs::metadata(&full_path) else {
            continue;
        };
        let dir = metadata.is_dir();
        if !dir
            && (kind != "dir"
                && !full_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        extensions
                            .iter()
                            .any(|allowed| ext.eq_ignore_ascii_case(allowed))
                    }))
        {
            continue;
        }
        entries.push(Entry {
            name,
            path: full_path,
            dir,
            size: (!dir).then_some(metadata.len()),
            modified_ms: metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| u64::try_from(duration.as_millis()).ok()),
        });
    }
    entries.sort_by_cached_key(|entry| (!entry.dir, entry.name.to_lowercase(), entry.name.clone()));
    let truncated = entries.len() > ENTRY_LIMIT;
    entries.truncate(ENTRY_LIMIT);
    let parent = if roots.iter().any(|root| Path::new(&root.path) == path) {
        None
    } else {
        path.parent()
    };
    Ok(json!({
        "path":path, "parent":parent, "entries":entries, "truncated":truncated,
        "roots":roots.iter().map(|root| json!({
            "path":root.path, "label":root.label, "removable":root.removable,
        })).collect::<Vec<_>>(),
    }))
}

fn read_error(path: &Path, error: std::io::Error) -> Value {
    let denied = error.kind() == std::io::ErrorKind::PermissionDenied;
    let details = if denied {
        json!({"kind":"access_denied","path":path})
    } else {
        json!({"path":path})
    };
    ipc::error(
        ErrorCode::InvalidRequest,
        "The folder cannot be read.",
        details,
        true,
    )
}
