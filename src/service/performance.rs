//! The Car Performance and Driver Performance tabs: the roster files as tables, and the edits
//! written back into them.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{
    cfg_custom_ai, cfg_custom_ai_dir, class_file, load_classes, rating_contexts,
    roster_seats_with, season_contexts, App, AppError, AppResult,
};
use crate::roster::custom_ai;

/// Every class ranked, each team's required rating, and the career rating of every human
/// recorded in any of those classes.
#[derive(Serialize, Debug, Clone)]
pub struct CarPerformance {
    /// Career ratings across every class, not per class.
    pub players: Vec<PlayerRating>,
    /// Classes the user is actually racing, for the tab's initial filter selection.
    pub active_classes: Vec<String>,
    /// The range the writer will accept, so the table enforces the same bounds the server does
    /// instead of keeping its own copy that can drift out of step.
    pub scalar_min: f32,
    pub scalar_max: f32,
    pub classes: Vec<CarPerformanceClass>,
}

#[derive(Serialize, Debug, Clone)]
pub struct PlayerRating {
    pub name: String,
    pub rating: f32,
    pub sp_races: u32,
    pub mp_races: u32,
    pub mp_wins: u32,
    pub mp_losses: u32,
}

#[derive(Serialize, Debug, Clone)]
pub struct CarPerformanceClass {
    pub class: String,
    pub year: Option<u16>,
    /// Whether this class has a baseline recorded, so the table knows if there is anything to
    /// reset to. One is taken on the first edit, so a never-edited class has none.
    pub has_baseline: bool,
    pub cars: Vec<CarRow>,
}

#[derive(Serialize, Debug, Clone)]
pub struct CarRow {
    #[serde(flatten)]
    pub car: custom_ai::CarPerformanceRow,
    /// Reputation needed to claim this seat, 0–100. `None` when the team has no seat AMS2 can
    /// actually field, so there is no seat to claim — distinct from a requirement of 0, which
    /// would read as "anyone may drive this".
    pub required_rating: Option<f32>,
    /// True when every one of the team's entries names a livery AMS2 does not own.
    pub phantom: bool,
    /// The car's picture, as a path for `/api/livery-preview/`. `None` when the livery mod
    /// declares none, or when the class has no livery mod at all.
    pub preview: Option<String>,
}

/// Every class in the same chronological order the Car Performance tab uses, each with its
/// named driver entries.
#[derive(Serialize, Debug, Clone)]
pub struct DriverPerformance {
    /// The server's own editable-tag list, so the table's columns and the writer's allowlist
    /// cannot drift apart.
    pub attrs: Vec<&'static str>,
    /// What each attribute contributes to the Rating column, so the table can show its own
    /// formula instead of asking the reader to take the number on trust.
    pub rating_weights: Vec<(&'static str, f32)>,
    /// Per-attribute accepted range, since vehicle_reliability is documented as the one that may
    /// leave 0–1. Each is (field, min, max).
    pub attr_ranges: Vec<(&'static str, f32, f32)>,

    /// Classes the user is actually racing, for the tab's initial filter selection.
    pub active_classes: Vec<String>,
    pub classes: Vec<DriverPerformanceClass>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DriverPerformanceClass {
    pub class: String,
    pub year: Option<u16>,
    /// Cars this class can field — the grid size to race it at. Not the number of rows below: a
    /// season roster names every driver who ever sat in a car, so per-track stand-ins and second
    /// liveries add entries without adding a car. Sent from here because only the server knows
    /// which liveries are actually installed.
    pub cars: usize,
    /// Each entry's picture, keyed by **livery name** rather than by team — every row here binds
    /// to one livery of its own, so unlike the Car Performance table this needs no "first car of
    /// the team" fallback. A row whose livery is absent from the map has no picture, which is
    /// the same thing `phantom` reports for a different reason.
    pub previews: HashMap<String, String>,
    pub drivers: Vec<custom_ai::DriverAttributes>,
}

/// One team's scalars, as the Car Performance tab edits them.
#[derive(Deserialize, Debug, Clone)]
pub struct TeamScalarsEdit {
    pub class: String,
    pub team: String,
    pub power_scalar: f32,
    pub weight_scalar: f32,
    pub drag_scalar: f32,
}

/// One attribute of one driver entry, as the Driver Performance tab edits it.
#[derive(Deserialize, Debug, Clone)]
pub struct DriverAttrEdit {
    pub class: String,
    /// Position among the file's named `<driver>` blocks, from [`DriverPerformance`].
    pub index: usize,
    /// The `<name>` that position is expected to hold — a stale-index guard.
    pub driver: String,
    pub field: String,
    pub value: f32,
}

impl App {
    /// The `*.xml` files in the configured Custom AI Drivers folder whose name AMS2 recognises as
    /// a car class. A file the game never reads cannot shape a session, so it is not offered as
    /// a championship roster.
    pub fn custom_ai_files(&self) -> Vec<String> {
        match self.config().custom_ai_dir {
            Some(dir) => custom_ai::list_files_for_known_classes(std::path::Path::new(&dir)),
            None => vec![],
        }
    }

    /// Built fresh from the XML files on each call, so a scalar edit is reflected immediately —
    /// nothing about this table is cached.
    pub fn car_performance(&self) -> CarPerformance {
        use crate::career::driver_rating;

        let config_path = &self.config_path;
        let career = self.active_path();
        let classes = load_classes(config_path);
        // Only sessions committed to a championship are rated, and they are rated on the
        // career's own tuning rather than whatever Config happens to say now.
        let (params, sessions, active, seasons) = {
            let data = self.data();
            (
                self.career_rating_params(&data),
                driver_rating::assigned_sessions(&data.championships, &data.sessions),
                crate::career::data_store::active_classes(&data.championships),
                season_contexts(config_path, &career, &data),
            )
        };

        let class_rows: Vec<CarPerformanceClass> = classes
            .iter()
            .map(|cd| {
                let required: HashMap<String, f32> =
                    driver_rating::team_requirements_with(&params, &cd.ctx.expected, &cd.skills)
                        .into_iter()
                        .collect();
                // `ctx.seats` is the phantom-filtered list, so a team missing from it has no car
                // AMS2 will field. The ranking table still lists it — it is in the roster, and
                // its scalars are still editable — but it has no seat to demand a rating for.
                let real_teams: HashSet<&str> =
                    cd.ctx.seats.iter().map(|s| s.team.as_str()).collect();
                CarPerformanceClass {
                    class: cd.perf.class.clone(),
                    year: cd.perf.year,
                    has_baseline: cd.has_baseline,
                    cars: cd
                        .perf
                        .cars
                        .iter()
                        .map(|c| {
                            let real = real_teams.contains(c.team.as_str());
                            CarRow {
                                required_rating: if real {
                                    Some(required.get(&c.team).copied().unwrap_or(0.0))
                                } else {
                                    None
                                },
                                phantom: !real,
                                preview: cd.previews.get(&c.team).cloned(),
                                car: c.clone(),
                            }
                        })
                        .collect(),
                }
            })
            .collect();

        // Signed seasons are rated on their own rosters, the table below on the game's files.
        let contexts = rating_contexts(seasons, classes);
        let players: Vec<PlayerRating> = driver_rating::recorded_players_global(&sessions, &contexts)
            .into_iter()
            .map(|name| {
                let r = driver_rating::compute_reputation_global_with(
                    Some(&name),
                    &sessions,
                    &contexts,
                    None,
                    &params,
                );
                PlayerRating {
                    name,
                    rating: r.value,
                    sp_races: r.sp_races,
                    mp_races: r.mp_races,
                    mp_wins: r.mp_wins,
                    mp_losses: r.mp_losses,
                }
            })
            .collect();

        CarPerformance {
            players,
            active_classes: active,
            scalar_min: custom_ai::SCALAR_MIN,
            scalar_max: custom_ai::SCALAR_MAX,
            classes: class_rows,
        }
    }

    /// Writes one team's scalars back into its Custom AI XML file. Answers with the whole
    /// recomputed table: an edit shifts the class baseline, every other car's pace delta, and the
    /// ratings derived from them.
    pub fn set_team_scalars(&self, edit: &TeamScalarsEdit) -> AppResult<CarPerformance> {
        let file = class_file(&self.config_path, &edit.class)?;
        let scalars = custom_ai::Scalars {
            power: edit.power_scalar,
            weight: edit.weight_scalar,
            drag: edit.drag_scalar,
        };
        custom_ai::set_team_scalars(&file, &edit.team, scalars).map_err(AppError::BadRequest)?;
        Ok(self.car_performance())
    }

    /// Records the class's file as it stands now as its baseline, replacing whatever was there.
    ///
    /// For a roster tuned by hand *after* the app first recorded one: the baseline is otherwise
    /// taken once and never refreshed, so it would hold the older version forever and a reset
    /// would throw that tuning away. Deliberately destructive, which is why the client confirms.
    pub fn adopt_class_baseline(&self, class: &str) -> AppResult<CarPerformance> {
        let file = class_file(&self.config_path, class)?;
        custom_ai::set_baseline(&file).map_err(AppError::BadRequest)?;
        Ok(self.car_performance())
    }

    /// Restores the class's file from its baseline. Answers with the whole recomputed table, as
    /// [`Self::adopt_class_baseline`] does: either one moves every scalar in the class.
    pub fn reset_class_to_baseline(&self, class: &str) -> AppResult<CarPerformance> {
        let file = class_file(&self.config_path, class)?;
        custom_ai::reset_from_baseline(&file).map_err(AppError::BadRequest)?;
        Ok(self.car_performance())
    }

    /// The livery mod's own picture of one car, decoded from its `.dds` into a PNG thumbnail.
    ///
    /// `requested` is a path under `Overrides` that one of the performance payloads handed out,
    /// but it arrives from outside and is re-checked rather than trusted: see
    /// [`crate::roster::liveries::preview_file`].
    pub fn livery_preview_png(&self, requested: &str) -> AppResult<Vec<u8>> {
        let file = cfg_custom_ai_dir(&self.config_path)
            .and_then(|dir| crate::roster::liveries::overrides_dir(&dir))
            .and_then(|overrides| crate::roster::liveries::preview_file(&overrides, requested))
            .ok_or_else(|| AppError::NotFound("no such livery preview".into()))?;
        let bytes =
            std::fs::read(&file).map_err(|_| AppError::NotFound("no such livery preview".into()))?;
        crate::roster::livery_image::dds_thumbnail_png(&bytes, crate::roster::livery_image::THUMB_WIDTH)
            .map_err(AppError::Unsupported)
    }

    /// Every named driver entry per class with its AI attributes.
    pub fn driver_performance(&self) -> DriverPerformance {
        let classes = match cfg_custom_ai(&self.config_path) {
            Some((dir, years)) => {
                // Read once for the whole payload: the manifests cover every car model at once,
                // and a livery belongs to a model rather than to a class.
                let installed = crate::roster::liveries::installed_liveries(&dir);
                let names = installed.as_ref().map(|l| l.names());
                custom_ai::class_performance_with(&dir, &years)
                    .into_iter()
                    .map(|c| {
                        let file = format!("{}.xml", c.class);
                        let mut drivers = custom_ai::parse_driver_attributes(&dir.join(&file));
                        custom_ai::mark_phantom_entries(&mut drivers, names.as_ref());
                        let cars =
                            custom_ai::car_count(&roster_seats_with(&dir, &file, names.as_ref()));
                        // Deduped: which model declares a shared name is settled by how much of
                        // the roster it declares, and a livery repeated by a dozen per-track
                        // entries is still one livery's worth of evidence.
                        let roster: Vec<String> = {
                            let mut seen: Vec<String> =
                                drivers.iter().map(|d| d.livery.clone()).collect();
                            seen.sort();
                            seen.dedup();
                            seen
                        };
                        DriverPerformanceClass {
                            previews: installed
                                .as_ref()
                                .map(|l| l.previews_for(&roster))
                                .unwrap_or_default(),
                            drivers,
                            cars,
                            class: c.class,
                            year: c.year,
                        }
                    })
                    .collect()
            }
            None => vec![],
        };
        DriverPerformance {
            attrs: custom_ai::DRIVER_ATTRS.to_vec(),
            rating_weights: custom_ai::RATING_WEIGHTS.to_vec(),
            attr_ranges: custom_ai::attr_ranges(),
            active_classes: crate::career::data_store::active_classes(&self.data().championships),
            classes,
        }
    }

    /// Writes one attribute of one driver entry back to its XML. Nothing else in the table
    /// derives from it, so only the edited row comes back — read back from the file rather than
    /// echoing the request, since the stored value is what the file now says after the writer's
    /// own number formatting.
    pub fn set_driver_attr(
        &self,
        edit: &DriverAttrEdit,
    ) -> AppResult<Option<custom_ai::DriverAttributes>> {
        let file = class_file(&self.config_path, &edit.class)?;
        custom_ai::set_driver_attr(&file, edit.index, &edit.driver, &edit.field, edit.value)
            .map_err(AppError::BadRequest)?;
        Ok(custom_ai::parse_driver_attributes(&file)
            .into_iter()
            .nth(edit.index))
    }

    /// Drops one per-track entry from a roster.
    ///
    /// Answers with the whole table again rather than the removed row: every index below the
    /// deleted one has just shifted, and the table the caller is holding addresses rows by index.
    /// Handing back one row would leave it pointing at the wrong blocks.
    pub fn remove_driver_entry(
        &self,
        class: &str,
        index: usize,
        driver: &str,
    ) -> AppResult<DriverPerformance> {
        let file = class_file(&self.config_path, class)?;
        custom_ai::remove_driver_entry(&file, index, driver).map_err(AppError::BadRequest)?;
        Ok(self.driver_performance())
    }

    /// Clears a class's per-track entries at once.
    ///
    /// Sits beside the per-row delete rather than replacing it: one is for the entry that is in
    /// the way, the other for a roster being made uniform before a season. Both go through the
    /// same `.xml.bak`, so both are undone by Reset to baseline.
    pub fn remove_track_entries(&self, class: &str) -> AppResult<DriverPerformance> {
        let file = class_file(&self.config_path, class)?;
        custom_ai::remove_track_entries(&file).map_err(AppError::BadRequest)?;
        Ok(self.driver_performance())
    }
}
