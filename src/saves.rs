use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::data_store::{try_load_data, CareerData, CareerMode};

/// The career file inside a save folder. Fixed, so a save is renamed by renaming one directory
/// rather than a directory and the file inside it.
pub const CAREER_FILE: &str = "career.json";

/// One career save found in the saves directory.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SaveInfo {
    /// What the user sees and passes back to the API — the folder name of a folder save, the
    /// file stem of a legacy flat one.
    pub name: String,
    /// Full path of the career file on disk.
    pub file: String,
    pub sessions: usize,
    pub championships: usize,
    pub active: bool,
    /// Which kind of career it is. `Unset` for a save written before careers had a mode; the
    /// UI asks once and `PATCH /api/career/mode` settles it.
    pub mode: CareerMode,
    /// Why the save could not be read, when it could not be — or what else about it the user
    /// needs to know, such as a legacy file being shadowed by a folder of the same name. A
    /// broken save still appears in the list: showing it as an empty career would be a lie, and
    /// hiding it would leave the user wondering where their season went.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn is_json(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

/// Build a `SaveInfo` by reading and counting the career file at `path`, under the given name.
///
/// The name is passed rather than derived because the two layouts take it from different places:
/// a folder save is named by its directory, a legacy flat save by its file stem.
fn info_for(path: &Path, name: String, active: &Path) -> SaveInfo {
    let (data, error) = match try_load_data(path) {
        Ok(data) => (data, None),
        Err(e) => (CareerData::default(), Some(e)),
    };
    SaveInfo {
        name,
        file: path.display().to_string(),
        sessions: data.sessions.len(),
        championships: data.championships.len(),
        active: path == active,
        mode: data.mode,
        error,
    }
}

/// The name of the save whose career file is `path`: the **folder** for a folder save, the file
/// stem for a legacy flat one.
///
/// Every folder save's file is called `career.json`, so a name taken from the stem would call all
/// of them "career".
pub fn save_name_of(path: &Path) -> Option<String> {
    if path.file_name().and_then(|n| n.to_str()) == Some(CAREER_FILE) {
        return path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .map(str::to_string);
    }
    path.file_stem().and_then(|n| n.to_str()).map(str::to_string)
}

/// `SaveInfo` for a career file identified only by its path — an active save `list_saves` did not
/// find, whether because it sits outside the saves directory or because it has not been written
/// to disk yet.
fn info_for_path(path: &Path, active: &Path) -> Option<SaveInfo> {
    Some(info_for(path, save_name_of(path)?, active))
}

/// Every career save inside `dir`, sorted by name.
///
/// Two layouts are recognised. A **folder save** is a directory holding a [`CAREER_FILE`], and is
/// what every new career is created as. A
/// **legacy flat save** is a `*.json` file directly inside `dir`, which is how every save written
/// before folders existed is stored; those keep working exactly as they did, and are never
/// migrated behind the user's back.
///
/// A directory without a [`CAREER_FILE`] is not a save at all, which is what keeps the shared
/// `track_layouts/` out of the list without naming it.
///
/// An `active` save the scan did not find is appended, so the running career is always
/// selectable. In practice that is a brand-new career whose file has not been written yet;
/// `config.active_career` is a name resolved against `dir`, so the active save can no longer sit
/// outside it.
pub fn list_saves(dir: &Path, active: &Path) -> Vec<SaveInfo> {
    let mut folders: Vec<(String, PathBuf)> = Vec::new();
    let mut flats: Vec<(String, PathBuf)> = Vec::new();

    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            let career = path.join(CAREER_FILE);
            if career.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    folders.push((name.to_string(), career));
                }
            }
        } else if path.is_file() && is_json(&path) {
            if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                flats.push((name.to_string(), path));
            }
        }
    }

    // A folder and a flat file can only share a name if someone made it so by hand — every route
    // that creates a save refuses a name either layout already holds. The folder wins, because it
    // is the layout a career grows into, and the entry says the other file is being ignored
    // rather than letting it vanish silently.
    let mut saves: Vec<SaveInfo> = folders
        .into_iter()
        .map(|(name, career)| {
            let shadowed = flats.iter().any(|(flat, _)| *flat == name);
            let mut info = info_for(&career, name, active);
            if shadowed && info.error.is_none() {
                info.error = Some(format!(
                    "a legacy {}.json beside this folder is being ignored",
                    info.name
                ));
            }
            info
        })
        .collect();
    let taken: Vec<String> = saves.iter().map(|s| s.name.clone()).collect();
    saves.extend(
        flats
            .into_iter()
            .filter(|(name, _)| !taken.contains(name))
            .map(|(name, path)| info_for(&path, name, active)),
    );
    saves.sort_by(|a, b| a.name.cmp(&b.name));

    // The active save is always selectable, even when the scan above did not find it — which is
    // a brand-new career, whose file is not written until something is saved into it.
    if !saves.iter().any(|s| s.active) {
        if let Some(info) = info_for_path(active, active) {
            saves.push(info);
        }
    }
    saves
}

/// Validate a user-supplied save name, returning the trimmed name.
///
/// Rejects anything that could escape the saves directory or produce an awkward folder name:
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

/// Where a **new** save called `name` goes: `<dir>/<name>/career.json`. `name` must already be
/// sanitized. Use [`existing_save_path`] to find a save that is already on disk, since it may be
/// in either layout.
pub fn save_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name).join(CAREER_FILE)
}

/// Where a legacy flat save called `name` lives: `<dir>/<name>.json`.
pub fn legacy_save_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

/// The career file of the save called `name`, in whichever layout it is stored, or `None` if no
/// save by that name exists. The folder layout wins when both are present — the same precedence
/// [`list_saves`] applies.
pub fn existing_save_path(dir: &Path, name: &str) -> Option<PathBuf> {
    let folder = save_path(dir, name);
    if folder.is_file() {
        return Some(folder);
    }
    let flat = legacy_save_path(dir, name);
    if flat.is_file() {
        return Some(flat);
    }
    None
}

/// True when a save called `name` exists in *either* layout. Creating, renaming or duplicating
/// onto a taken name must check both, or a new folder save would shadow a legacy file.
pub fn name_taken(dir: &Path, name: &str) -> bool {
    existing_save_path(dir, name).is_some()
}

/// The folder holding a save, given the path of its career file — `None` for a legacy flat save,
/// which has no folder of its own.
pub fn career_dir(career_file: &Path) -> Option<PathBuf> {
    if career_file.file_name().and_then(|n| n.to_str()) != Some(CAREER_FILE) {
        return None;
    }
    career_file.parent().map(Path::to_path_buf)
}


/// Create the folder a new save's career file sits in. No-op for a legacy flat path.
pub fn prepare_save_dir(career_file: &Path) -> Result<(), String> {
    let Some(dir) = career_dir(career_file) else {
        return Ok(());
    };
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))
}

fn copy_dir_all(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    for entry in fs::read_dir(from)
        .map_err(|e| format!("cannot read {}: {e}", from.display()))?
        .flatten()
    {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir_all(&src, &dst)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| format!("cannot copy {}: {e}", src.display()))?;
        }
    }
    Ok(())
}

/// Copy the save called `name` to `new_name`, returning the new career file's path.
///
/// The copy is always made in the folder layout, seasons and all — a duplicate is a new save, so
/// it is written the way new saves are written. Duplicating a legacy flat save therefore produces
/// a folder save and leaves the original flat file untouched.
pub fn duplicate_save(dir: &Path, name: &str, new_name: &str) -> Result<PathBuf, String> {
    let Some(from) = existing_save_path(dir, name) else {
        return Err("save not found".to_string());
    };
    if name_taken(dir, new_name) {
        return Err("a save with that name already exists".to_string());
    }
    let to = save_path(dir, new_name);
    match career_dir(&from) {
        // Folder save: copy the whole folder so seasons come with it.
        Some(src_dir) => copy_dir_all(&src_dir, &dir.join(new_name))?,
        None => {
            prepare_save_dir(&to)?;
            fs::copy(&from, &to).map_err(|e| format!("cannot copy {}: {e}", from.display()))?;
        }
    }
    Ok(to)
}

/// Rename the save called `name` to `new_name`, returning the new career file's path.
///
/// A folder save is renamed by moving its **folder**, which carries its seasons with it; a legacy
/// flat save is renamed in place and stays flat. Neither is migrated to the other layout — moving
/// a career the user did not ask to move is how careers get lost.
pub fn rename_save(dir: &Path, name: &str, new_name: &str) -> Result<PathBuf, String> {
    let Some(from) = existing_save_path(dir, name) else {
        return Err("save not found".to_string());
    };
    if name_taken(dir, new_name) {
        return Err("a save with that name already exists".to_string());
    }
    match career_dir(&from) {
        Some(src_dir) => {
            let dest_dir = dir.join(new_name);
            fs::rename(&src_dir, &dest_dir)
                .map_err(|e| format!("cannot rename {}: {e}", src_dir.display()))?;
            Ok(dest_dir.join(CAREER_FILE))
        }
        None => {
            let to = legacy_save_path(dir, new_name);
            fs::rename(&from, &to).map_err(|e| format!("cannot rename {}: {e}", from.display()))?;
            Ok(to)
        }
    }
}

/// Delete the save called `name`, folder and all.
///
/// A folder save is a recursive delete, which a single file never was, so it is guarded: the
/// folder must be a **direct child** of the saves directory and must actually hold a
/// [`CAREER_FILE`]. `name` is already sanitized by the time it gets here — these checks exist so
/// that a future caller which forgets cannot turn this into a recursive delete of an arbitrary
/// path.
pub fn delete_save(dir: &Path, name: &str) -> Result<(), String> {
    let folder = dir.join(name);
    if folder.join(CAREER_FILE).is_file() {
        if folder.parent() != Some(dir) {
            return Err("refusing to delete a folder outside the saves directory".to_string());
        }
        return fs::remove_dir_all(&folder)
            .map_err(|e| format!("cannot delete {}: {e}", folder.display()));
    }
    let flat = legacy_save_path(dir, name);
    if flat.is_file() {
        return fs::remove_file(&flat).map_err(|e| format!("cannot delete {}: {e}", flat.display()));
    }
    Err("save not found".to_string())
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

/// The save to open on startup, or `None` when the folder holds no careers at all.
///
/// Prefers the remembered career by `name`, but only while it is still there — after the saves
/// folder is changed the remembered name may name nothing, so the folder itself decides: its
/// `ams2_career` save in either layout, else the first save in it.
///
/// **An empty folder yields `None` rather than inventing a career.** Naming one for the user is
/// a decision that is theirs: a career is created with a name and a [`CareerMode`] they choose,
/// and a mode is permanent. An invented save arrives `Unset`, which is the state reserved for
/// careers written before modes existed — so the switcher greets a first-time user by asking
/// them to settle a question about a career they never asked for.
pub fn resolve_active(dir: &Path, name: Option<&str>) -> Option<PathBuf> {
    if let Some(remembered) = name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|n| existing_save_path(dir, n))
    {
        return Some(remembered);
    }
    if let Some(found) = existing_save_path(dir, "ams2_career") {
        return Some(found);
    }
    // `list_saves` invents an entry for a missing active file, so scan for real saves instead.
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            let career = path.join(CAREER_FILE);
            if career.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    found.push((name.to_string(), career));
                }
            }
        } else if path.is_file() && is_json(&path) {
            if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                found.push((name.to_string(), path));
            }
        }
    }
    found.sort();
    found.into_iter().next().map(|(_, p)| p)
}

#[cfg(test)]
#[path = "tests/saves.rs"]
mod tests;
