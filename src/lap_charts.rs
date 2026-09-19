//! Lap charts, kept beside a career rather than inside it.
//!
//! A lap chart is one row per driver per completed lap, so it is by far the biggest thing a
//! session carries: measured across the reference career it is **44% of every byte** in
//! `career.json`, and it grows with laps × drivers while everything else grows with drivers
//! alone. It is also the only part of a session that nothing aggregate reads — standings, the
//! driver rating, track stats and contracts are all built from results — and it is looked at one
//! session at a time, when someone opens that session's chart.
//!
//! So it lives in `laps/<session id>.json` inside the career's folder, and the store never holds
//! it at all. That keeps the file every mutation rewrites small: `persist` serialises the whole
//! career on each change, which on a saves folder inside Google Drive means re-uploading it.
//!
//! **Legacy flat saves keep their charts inline.** A flat save is a bare `<name>.json` with no
//! folder to put anything beside, and those saves are never migrated — see [`crate::saves`]. So
//! [`externalize`] simply does nothing for them and the charts stay where they are, which is
//! also why every reader here falls back to the copy held in the session.

use std::fs;
use std::path::{Path, PathBuf};

use crate::data_store::{LapChartEntry, RecordedSession};

/// Folder holding one career's lap charts, inside that career's own folder.
const LAPS_DIR: &str = "laps";

/// Where this career's lap charts live, or `None` for a legacy flat save.
pub fn laps_dir(career_file: &Path) -> Option<PathBuf> {
    crate::saves::career_dir(career_file).map(|dir| dir.join(LAPS_DIR))
}

/// The file one session's chart is stored in.
///
/// `None` for a flat save, and also for a session id that cannot be a filename. Ids are written
/// as unix timestamps, but a hand-edited career can hold anything, and this value is joined onto
/// a path — so it goes through the same gate every user-supplied save name does.
pub fn chart_path(career_file: &Path, session_id: &str) -> Option<PathBuf> {
    let name = crate::saves::sanitize_name(session_id)?;
    laps_dir(career_file).map(|dir| dir.join(format!("{name}.json")))
}

/// One session's chart, or empty when there is none to read.
///
/// Empty is the honest answer for every way this can fail — no file, a flat save, a chart that
/// was never recorded because the session was practice. The chart is a display detail, so a
/// missing one draws nothing rather than failing a request.
pub fn read(career_file: &Path, session_id: &str) -> Vec<LapChartEntry> {
    let Some(path) = chart_path(career_file, session_id) else {
        return vec![];
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return vec![];
    };
    serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default()
}

/// Writes one session's chart beside the career.
///
/// Charts are written once, when the session is recorded, and never edited afterwards — so
/// unlike the career file there is nothing here to guard against overwriting.
pub fn write(career_file: &Path, session_id: &str, chart: &[LapChartEntry]) -> Result<(), String> {
    let Some(path) = chart_path(career_file, session_id) else {
        return Err("this save has nowhere to keep lap charts".to_string());
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string(chart).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Deletes one session's chart. A chart that is not there is already the wanted outcome.
pub fn remove(career_file: &Path, session_id: &str) {
    if let Some(path) = chart_path(career_file, session_id) {
        let _ = fs::remove_file(path);
    }
}

/// Moves any chart still held inline out to its own file, and reports how many moved.
///
/// This is both the upgrade for a career written before charts were split out and the step
/// `capture` takes for each new session, which is why it works on a slice rather than a whole
/// career. A session whose chart cannot be written keeps it inline: the career file is about to
/// be saved either way, so the chart stays with it and nothing is lost.
pub fn externalize(career_file: &Path, sessions: &mut [RecordedSession]) -> usize {
    let mut moved = 0;
    for session in sessions.iter_mut() {
        if session.lap_chart.is_empty() {
            continue;
        }
        match write(career_file, &session.id, &session.lap_chart) {
            Ok(()) => {
                session.lap_chart = Vec::new();
                moved += 1;
            }
            // Only worth saying for a save that was supposed to be able to do this. A flat save
            // refuses every time by design, and saying so once per session would be noise.
            Err(e) if laps_dir(career_file).is_some() => {
                eprintln!(
                    "Keeping the lap chart for session {} inline: {e}",
                    session.id
                );
            }
            Err(_) => {}
        }
    }
    moved
}

#[cfg(test)]
#[path = "tests/lap_charts.rs"]
mod tests;
