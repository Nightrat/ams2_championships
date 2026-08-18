use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::data_store::load_data;

/// One career save file found in the saves directory.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SaveInfo {
    /// File stem — what the user sees and passes back to the API.
    pub name: String,
    /// Full path on disk.
    pub file: String,
    pub sessions: usize,
    pub championships: usize,
    pub active: bool,
}

fn is_json(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

/// Build a `SaveInfo` by reading and counting the career file at `path`.
fn info_for(path: &Path, active: &Path) -> Option<SaveInfo> {
    let name = path.file_stem().and_then(|n| n.to_str())?.to_string();
    let data = load_data(path);
    Some(SaveInfo {
        name,
        file: path.display().to_string(),
        sessions: data.sessions.len(),
        championships: data.championships.len(),
        active: path == active,
    })
}

/// List every `*.json` career save directly inside `dir`, sorted by name.
///
/// Subdirectories (notably `track_layouts/`) are skipped. When `active` points at a file
/// outside `dir` — a legacy custom `data_file` path — it is appended so the running career
/// is always selectable.
pub fn list_saves(dir: &Path, active: &Path) -> Vec<SaveInfo> {
    let mut saves: Vec<SaveInfo> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.is_file() && is_json(&path) {
                info_for(&path, active)
            } else {
                None
            }
        })
        .collect();
    saves.sort_by(|a, b| a.name.cmp(&b.name));

    if !saves.iter().any(|s| s.active) {
        if let Some(info) = info_for(active, active) {
            saves.push(info);
        }
    }
    saves
}

/// Validate a user-supplied save name, returning the trimmed name.
///
/// Rejects anything that could escape the saves directory or produce an awkward file name:
/// empty names, path separators, `..`, and characters outside `[A-Za-z0-9 _-]`.
pub fn sanitize_name(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 64 {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '_' || c == '-')
    {
        return None;
    }
    Some(name.to_string())
}

/// Full path of the save called `name` inside `dir`. `name` must already be sanitized.
pub fn save_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

/// The saves folder: `configured` when set and non-blank, otherwise `championships` next to
/// the executable.
pub fn resolve_dir(exe_dir: &Path, configured: Option<&str>) -> PathBuf {
    configured
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| exe_dir.join("championships"))
}

/// The save to open on startup.
///
/// Prefers the remembered `configured` file, but only while it still exists — after the saves
/// folder is changed the old path no longer applies, so the folder itself decides: its
/// `ams2_career.json`, else the first save in it, else a fresh `ams2_career.json`.
pub fn resolve_active(dir: &Path, configured: Option<&str>) -> PathBuf {
    if let Some(file) = configured.map(str::trim).filter(|s| !s.is_empty()) {
        let path = PathBuf::from(file);
        if path.is_file() {
            return path;
        }
    }
    let default = save_path(dir, "ams2_career");
    if default.is_file() {
        return default;
    }
    // `list_saves` invents an entry for a missing active file, so scan for real files instead.
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_json(p))
        .collect();
    found.sort();
    found.into_iter().next().unwrap_or(default)
}

#[cfg(test)]
#[path = "tests/saves.rs"]
mod tests;
