//! The roster a season was signed on, kept beside the career.
//!
//! Everything singleplayer is measured against a Custom AI file: the rating scores each result
//! against the roster's pace scalars, a team's bar comes from its incumbents' skills, and the
//! live grid's team names, the seat checks and the standings' team names all read it. The file
//! itself lives in the game's install, where the Car and Driver Performance tabs edit it and the
//! user may replace it outright — so a season read from there is re-judged every time it changes,
//! long after it was raced and paid for.
//!
//! So **signing a contract copies the roster into the career**, as `rosters/<champ id>.xml`, and
//! from then on that season reads its roster from the copy and nowhere else. The game still loads
//! its own file, which is why [`matches_game`] exists: the live tab compares the two and offers
//! [`restore`], copying the season's roster back to where AMS2 reads it.
//!
//! **Legacy flat saves get no copy.** A flat `<name>.json` has no folder to keep one in, and those
//! saves are never migrated (see [`crate::saves`]), so they go on reading the game's file exactly
//! as before — the same rule lap charts follow.

use std::fs;
use std::path::{Path, PathBuf};

use crate::data_store::Championship;

/// Folder holding one career's season rosters, inside that career's own folder.
const ROSTERS_DIR: &str = "rosters";

/// Where this career keeps its season rosters, or `None` for a legacy flat save.
pub fn rosters_dir(career_file: &Path) -> Option<PathBuf> {
    crate::saves::career_dir(career_file).map(|dir| dir.join(ROSTERS_DIR))
}

/// The file one season's roster is kept in, whether or not it exists yet.
///
/// `None` for a flat save, and for a championship id that cannot be a filename — ids are written
/// as unix timestamps, but a hand-edited career can hold anything and this is joined onto a path.
pub fn copy_path(career_file: &Path, champ_id: &str) -> Option<PathBuf> {
    let name = crate::saves::sanitize_name(champ_id)?;
    rosters_dir(career_file).map(|dir| dir.join(format!("{name}.xml")))
}

/// The season's stored roster, if it has one.
pub fn stored(career_file: &Path, champ_id: &str) -> Option<PathBuf> {
    copy_path(career_file, champ_id).filter(|p| p.is_file())
}

/// The file a championship's roster is read from: its stored copy when it has one, otherwise the
/// game's file in `ai_dir`. `None` when there is neither a copy nor a folder and a file to read.
///
/// **Every read of a season's roster goes through here**, so a signed season cannot be judged on
/// one file by the rating and another by the seat check.
pub fn roster_file(
    career_file: &Path,
    ai_dir: Option<&Path>,
    champ: &Championship,
) -> Option<PathBuf> {
    let file = champ.custom_ai_file.as_deref()?;
    stored(career_file, &champ.id).or_else(|| ai_dir.map(|d| d.join(file)))
}

/// Keeps a copy of `game_file` as the season's roster, replacing any it had.
///
/// Fails for a flat save rather than doing nothing: the caller decides whether a season that
/// cannot be fixed may still be signed.
pub fn store(career_file: &Path, champ_id: &str, game_file: &Path) -> Result<(), String> {
    let path = copy_path(career_file, champ_id)
        .ok_or_else(|| "this save has nowhere to keep a season's roster".to_string())?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    fs::copy(game_file, &path)
        .map(|_| ())
        .map_err(|e| format!("cannot copy {} into the career: {e}", game_file.display()))
}

/// Forgets a season's roster. One that is not there is already the wanted outcome.
pub fn remove(career_file: &Path, champ_id: &str) {
    if let Some(path) = copy_path(career_file, champ_id) {
        let _ = fs::remove_file(path);
    }
}

/// Whether the file AMS2 will load for this season is the season's own roster.
///
/// `None` when there is nothing to compare — no copy, no folder, no file assigned. A game file
/// that cannot be read (deleted, say) is `Some(false)`: AMS2 is certainly not loading the
/// season's roster then. Compared byte for byte, because any difference at all is a grid the
/// season was not signed on, and the remedy — copying it back — is the same either way.
pub fn matches_game(career_file: &Path, ai_dir: &Path, champ: &Championship) -> Option<bool> {
    let file = champ.custom_ai_file.as_deref()?;
    let copy = fs::read(stored(career_file, &champ.id)?).ok()?;
    Some(fs::read(ai_dir.join(file)).is_ok_and(|game| game == copy))
}

/// Copies the season's roster over the game's file, so AMS2 races the grid the season is judged
/// on. The game reads a roster when a session loads, so the session has to be restarted after.
///
/// The game's file gets its baseline first, like every other write the app makes to a roster
/// (see [`crate::custom_ai::ensure_baseline`]) — Reset to baseline must still reach the roster as
/// it was before the app ever touched it.
pub fn restore(career_file: &Path, ai_dir: &Path, champ: &Championship) -> Result<(), String> {
    let file = champ
        .custom_ai_file
        .as_deref()
        .ok_or_else(|| "this season has no Custom AI Drivers file".to_string())?;
    let copy = stored(career_file, &champ.id)
        .ok_or_else(|| "this season has no stored roster".to_string())?;
    let target = ai_dir.join(file);
    if target.is_file() {
        crate::custom_ai::ensure_baseline(&target)?;
    }
    fs::copy(&copy, &target)
        .map(|_| ())
        .map_err(|e| format!("cannot write {}: {e}", target.display()))
}

#[cfg(test)]
#[path = "tests/season_rosters.rs"]
mod tests;
