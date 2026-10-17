//! Career saves: listing them, creating, switching, duplicating, renaming and deleting.
//!
//! Exactly one save is active. Switching swaps the in-memory store's *contents* so the recorder
//! thread — which shares the same `Arc` — follows along without a restart.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{bad, conflict, internal, not_found, App, AppResult};
use crate::career::data_store::{CareerData, CareerMode};
use crate::career::saves::{self, SaveInfo};

/// Every save with its counts, and which one is active.
#[derive(Serialize, Debug)]
pub struct SavesView {
    pub active: String,
    /// Folder in use by the running app — may lag config.json until a restart.
    pub dir: String,
    pub saves: Vec<SaveInfo>,
}

impl App {
    pub fn saves(&self) -> SavesView {
        self.saves_view(&self.active_path())
    }

    fn saves_view(&self, active: &Path) -> SavesView {
        let saves = saves::list_saves(&self.saves_dir, active);
        let active_name = saves
            .iter()
            .find(|s| s.active)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        SavesView {
            active: active_name,
            dir: self.saves_dir.display().to_string(),
            saves,
        }
    }

    /// Creates a new empty career and switches to it.
    ///
    /// A new career must say which kind it is: every rule turns on it, and `Unset` exists only
    /// to describe saves written before the question was asked.
    pub fn create_save(&self, name: &str, mode: CareerMode) -> AppResult<SavesView> {
        let name = saves::sanitize_name(name).ok_or_else(|| bad("invalid save name"))?;
        if saves::name_taken(&self.saves_dir, &name) {
            return Err(conflict("a save with that name already exists"));
        }
        if mode == CareerMode::Unset {
            return Err(bad("choose singleplayer or multiplayer for the new career"));
        }
        // The balance and the rating tuning are recorded on the save now and never re-read from
        // config, so a career keeps what it was founded with however the settings move
        // afterwards. `App::adopt_rating` is the deliberate way to change the tuning later.
        let founding = self.config();
        let fresh = CareerData {
            mode,
            starting_balance: founding.starting_balance.max(0),
            rating_params: Some(founding.rating_params()),
            ..CareerData::default()
        };
        let text = serde_json::to_string_pretty(&fresh).unwrap_or_default();
        // A new career is created in the folder layout.
        let target = saves::save_path(&self.saves_dir, &name);
        saves::prepare_save_dir(&target).map_err(internal)?;
        std::fs::write(&target, text).map_err(|e| internal(e.to_string()))?;
        self.switch_to(target)
    }

    /// Switches to an existing career, wherever it lives — for a save written before folders,
    /// that is a flat `<name>.json`.
    pub fn activate_save(&self, name: &str) -> AppResult<SavesView> {
        let name = saves::sanitize_name(name).ok_or_else(|| bad("invalid save name"))?;
        let target = saves::existing_save_path(&self.saves_dir, &name)
            .ok_or_else(|| not_found("save not found"))?;
        self.switch_to(target)
    }

    fn switch_to(&self, target: PathBuf) -> AppResult<SavesView> {
        // Read the incoming career *before* touching anything. Switching to a save that cannot
        // be parsed would put an empty career in front of the user under that save's name; the
        // file itself is protected by `persist`, but the switch is still the wrong answer.
        let incoming = crate::career::data_store::try_load_data(&target).map_err(|e| {
            conflict(format!(
                "{} could not be read ({e}). It has not been changed, and the current \
                 career is still active.",
                target.display()
            ))
        })?;

        // Flush the outgoing career before repointing, then swap the store's *contents* — never
        // the Arc itself, which the recorder thread also holds.
        //
        // There may be no outgoing career: the saves folder was empty at startup, and this is the
        // first one being created. Flushing then would fail and take the creation down with it,
        // which is the one operation a careerless app must still be able to perform.
        let outgoing = self.active_path();
        if !outgoing.as_os_str().is_empty() {
            self.persist_to(&outgoing)?;
        }
        {
            let mut active = self.data_path.write().unwrap();
            *self.store.write().unwrap() = incoming;
            *active = target.clone();
        }
        // The incoming career is being loaded, so it gets the same upgrade startup gives the
        // one the app opened with — this and startup are the only two places a career is read in.
        self.seal_career(&target).map_err(internal)?;
        self.store_active_save(&target).map_err(internal)?;
        Ok(self.saves_view(&target))
    }

    /// Copies a career under a new name. The active save stays active.
    pub fn duplicate_save(&self, name: &str, new_name: &str) -> AppResult<SavesView> {
        let name = saves::sanitize_name(name).ok_or_else(|| bad("invalid save name"))?;
        let new_name = saves::sanitize_name(new_name).ok_or_else(|| bad("invalid new save name"))?;
        if saves::existing_save_path(&self.saves_dir, &name).is_none() {
            return Err(not_found("save not found"));
        }
        if saves::name_taken(&self.saves_dir, &new_name) {
            return Err(conflict("a save with that name already exists"));
        }
        saves::duplicate_save(&self.saves_dir, &name, &new_name).map_err(internal)?;
        Ok(self.saves())
    }

    /// Renames a save. A folder save moves its whole folder, so its seasons travel with it.
    pub fn rename_save(&self, name: &str, new_name: &str) -> AppResult<SavesView> {
        let (Some(name), Some(new_name)) = (saves::sanitize_name(name), saves::sanitize_name(new_name))
        else {
            return Err(bad("invalid save name"));
        };
        let from = saves::existing_save_path(&self.saves_dir, &name)
            .ok_or_else(|| not_found("save not found"))?;
        if saves::name_taken(&self.saves_dir, &new_name) {
            return Err(conflict("a save with that name already exists"));
        }
        // Flush first: an active save with unwritten changes would otherwise be renamed out from
        // under the next persist. Refused when the file could not be read, so it must not be
        // renamed either — moving it would leave the unreadable career under a name the switcher
        // offers as this one.
        let was_active = self.active_path() == from;
        if was_active {
            self.persist_to(&from)?;
        }
        let to = saves::rename_save(&self.saves_dir, &name, &new_name).map_err(internal)?;
        if was_active {
            *self.data_path.write().unwrap() = to.clone();
            self.store_active_save(&to).map_err(internal)?;
        }
        Ok(self.saves())
    }

    /// Deletes a save — never the active one. A folder save takes its seasons with it; see
    /// [`saves::delete_save`] for the guards on what is a recursive delete.
    pub fn delete_save(&self, name: &str) -> AppResult<SavesView> {
        let name = saves::sanitize_name(name).ok_or_else(|| bad("invalid save name"))?;
        let target = saves::existing_save_path(&self.saves_dir, &name)
            .ok_or_else(|| not_found("save not found"))?;
        if self.active_path() == target {
            return Err(bad(
                "cannot delete the active save — switch to another one first",
            ));
        }
        saves::delete_save(&self.saves_dir, &name).map_err(internal)?;
        Ok(self.saves())
    }

    /// Records the active career in config.json so it survives a restart.
    fn store_active_save(&self, file: &Path) -> Result<(), String> {
        let mut cfg = self.config();
        // The name, not the path — `saves_dir` already says which folder it is in.
        cfg.active_career = saves::save_name_of(file);
        crate::config::save(&self.config_path, &cfg)
    }
}
