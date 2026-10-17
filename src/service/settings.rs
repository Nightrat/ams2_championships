//! Settings: config.json, the rating tuning a career adopts from it, the spotter, and the track
//! layouts the radar draws.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{bad, internal, App, AppResult};
use crate::config::Config;
use crate::career::driver_rating::{Gates, RatingParams};

/// One row of the Config tab's season-year list.
#[derive(Serialize, Debug, Clone)]
pub struct ClassYearRow {
    pub class: String,
    /// The year in force: the override where there is one, else the shipped table's.
    pub year: Option<u16>,
    /// What the shipped table says on its own — what clearing the box falls back to, and `None`
    /// for a class it has never heard of (every mod, and Formula Edge).
    pub builtin: Option<u16>,
    /// Whether `year` came from config rather than from the table.
    pub overridden: bool,
}

/// The config, plus what the Config tab needs to explain it.
#[derive(Serialize)]
pub struct ConfigView {
    #[serde(flatten)]
    pub cfg: Config,
    /// Whether the active career is actually being judged on the rating settings shown here.
    /// False once they have been edited without being adopted — the career keeps the tuning it
    /// was created with, so the tab must not imply otherwise.
    pub career_rating_matches: bool,
    /// The tuning the career really runs on, so the notice can say what it would change.
    pub career_rating: Option<RatingParams>,
    /// Every class a season year can be set for, with the year in force and the one the shipped
    /// table would give. `cfg.class_years` alone could not fill the form: it holds only the
    /// classes somebody has answered for.
    pub classes: Vec<ClassYearRow>,
    /// The range a year is held to, so the boxes enforce what the server enforces rather than
    /// keeping a second copy of the bounds that can drift — the same reason [`super::CarPerformance`]
    /// ships `scalar_min`/`scalar_max`.
    pub year_min: u16,
    pub year_max: u16,
}

/// The Config tab's form. `active_career` is deliberately absent — the active career is owned
/// by the saves operations — and so are the spotter fields, which [`App::patch_spotter`] owns.
#[derive(Deserialize, Debug, Clone)]
pub struct ConfigPatch {
    pub port: u16,
    pub host: String,
    // Optional so a form that predates it carries the stored choice through.
    #[serde(default)]
    pub open_browser: Option<bool>,
    pub poll_ms: u64,
    pub record_practice: bool,
    pub record_qualify: bool,
    pub record_race: bool,
    pub show_track_map: bool,
    pub track_map_max_points: u32,
    #[serde(default)]
    pub saves_dir: Option<String>,
    #[serde(default)]
    pub custom_ai_dir: Option<String>,
    #[serde(default = "yes")]
    pub enforce_team_eligibility: bool,
    #[serde(default)]
    pub hide_locked_teams: bool,
    // Optional so a form that omits one carries the stored value through rather than resetting
    // it behind the user — the same reason the spotter fields are not in this body at all. The
    // consequence here would be worse: an omitted salary would zero the whole economy.
    #[serde(default)]
    pub contract_top_salary: Option<i64>,
    #[serde(default)]
    pub contract_floor_salary: Option<i64>,
    #[serde(default)]
    pub contract_buy_in_per_point: Option<i64>,
    pub contract_pay_driver_margin: Option<f32>,
    #[serde(default)]
    pub champion_prize: Option<i64>,
    #[serde(default)]
    pub last_place_prize: Option<i64>,
    #[serde(default)]
    pub starting_balance: Option<i64>,
    // Rating tuning. Each falls back to the default rather than to zero, so a form that predates
    // these fields — or one that fails to send them — leaves the rating alone instead of
    // silently resetting every driver to a rating of 0.
    #[serde(default = "default_start")]
    pub starting_rating: f32,
    #[serde(default)]
    pub rating_strictness: f32,
    #[serde(default)]
    pub eligibility_gates: Gates,
    #[serde(default = "default_half_life")]
    pub rating_half_life: f32,
    #[serde(default = "yes")]
    pub count_retirements: bool,
    #[serde(default = "default_retire_laps")]
    pub retirement_min_laps_down: u32,
    #[serde(default = "default_retire_distance")]
    pub retirement_distance_pct: f32,
    #[serde(default = "default_margin")]
    pub offer_margin: f32,
    /// Season year per class. Optional for the same reason the money is: a form that predates
    /// the field, or one rendered before the Custom AI folder was set, must carry the stored
    /// overrides through rather than wipe them. An empty map is a deliberate "no overrides" and
    /// is honoured as one.
    #[serde(default)]
    pub class_years: Option<BTreeMap<String, u16>>,
}

impl ConfigPatch {
    /// The form a front end holding a whole [`Config`] would send: every field as it stands,
    /// with the class-year overrides given separately because the form edits them as text.
    ///
    /// The fields the form does not own — the active career, the spotter — are not part of a
    /// patch at all, so [`App::patch_config`] carries the stored ones through either way.
    pub fn from_config(cfg: &Config, class_years: BTreeMap<String, u16>) -> Self {
        Self {
            port: cfg.port,
            host: cfg.host.clone(),
            open_browser: Some(cfg.open_browser),
            poll_ms: cfg.poll_ms,
            record_practice: cfg.record_practice,
            record_qualify: cfg.record_qualify,
            record_race: cfg.record_race,
            show_track_map: cfg.show_track_map,
            track_map_max_points: cfg.track_map_max_points,
            saves_dir: cfg.saves_dir.clone().filter(|s| !s.trim().is_empty()),
            custom_ai_dir: cfg.custom_ai_dir.clone().filter(|s| !s.trim().is_empty()),
            enforce_team_eligibility: cfg.enforce_team_eligibility,
            hide_locked_teams: cfg.hide_locked_teams,
            contract_top_salary: Some(cfg.contract_top_salary),
            contract_floor_salary: Some(cfg.contract_floor_salary),
            contract_buy_in_per_point: Some(cfg.contract_buy_in_per_point),
            contract_pay_driver_margin: Some(cfg.contract_pay_driver_margin),
            champion_prize: Some(cfg.champion_prize),
            last_place_prize: Some(cfg.last_place_prize),
            starting_balance: Some(cfg.starting_balance),
            starting_rating: cfg.starting_rating,
            rating_strictness: cfg.rating_strictness,
            eligibility_gates: cfg.eligibility_gates,
            rating_half_life: cfg.rating_half_life,
            count_retirements: cfg.count_retirements,
            retirement_min_laps_down: cfg.retirement_min_laps_down,
            retirement_distance_pct: cfg.retirement_distance_pct,
            offer_margin: cfg.offer_margin,
            class_years: Some(class_years),
        }
    }
}

fn yes() -> bool {
    true
}
fn default_start() -> f32 {
    RatingParams::default().starting_rating
}
fn default_half_life() -> f32 {
    RatingParams::default().recency_half_life
}
fn default_retire_laps() -> u32 {
    RatingParams::default().retirement_min_laps_down
}
fn default_retire_distance() -> f32 {
    RatingParams::default().retirement_distance * 100.0
}
fn default_margin() -> f32 {
    RatingParams::default().offer_margin
}

/// What saving the Config tab stored, and what it cannot apply until a restart.
#[derive(Serialize)]
pub struct ConfigPatched {
    pub config: Config,
    pub restart_required: Vec<&'static str>,
    /// Rebuilt rather than echoed back: saving may have changed the Custom AI folder, and with it
    /// which classes there are to answer for at all.
    pub classes: Vec<ClassYearRow>,
}

/// The spotter's settings as they stand in the running app.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SpotterStatus {
    pub enabled: bool,
    /// The driver the spotter follows; `None` follows the player.
    pub player: Option<String>,
    /// The system voice to speak in; `None` is the system default.
    pub voice: Option<String>,
}

/// A change to the spotter. A field left `None` is left alone; `Some(None)` clears it.
#[derive(Debug, Clone, Default)]
pub struct SpotterPatch {
    pub enabled: Option<bool>,
    pub player: Option<Option<String>>,
    pub voice: Option<Option<String>>,
}

impl App {
    pub fn config_view(&self) -> ConfigView {
        let cfg = self.config();
        let career_rating = self.data().rating_params;
        ConfigView {
            career_rating_matches: career_rating.is_none_or(|p| p == cfg.rating_params()),
            career_rating,
            classes: class_year_rows(&cfg),
            year_min: crate::roster::season_years::YEAR_MIN,
            year_max: crate::roster::season_years::YEAR_MAX,
            cfg,
        }
    }

    /// Saves the Config tab's form.
    ///
    /// The saves folder is only read at startup — the running app keeps its current folder and
    /// active save until restarted. The folder is created now so a typo surfaces here rather
    /// than at the next launch.
    pub fn patch_config(&self, req: ConfigPatch) -> AppResult<ConfigPatched> {
        let old_cfg = self.config();

        let mut restart_required: Vec<&'static str> = vec![];
        if req.port != old_cfg.port {
            restart_required.push("port");
        }
        if req.host != old_cfg.host {
            restart_required.push("host");
        }

        let blank = |s: &Option<String>| s.as_deref().map(str::trim).unwrap_or("").is_empty();
        let saves_dir_changed =
            !(blank(&req.saves_dir) && blank(&old_cfg.saves_dir)) && req.saves_dir != old_cfg.saves_dir;
        if saves_dir_changed {
            if let Some(dir) = req
                .saves_dir
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                std::fs::create_dir_all(dir)
                    .map_err(|e| bad(format!("cannot use that saves folder: {e}")))?;
            }
            restart_required.push("saves_dir");
        }

        let mut new_cfg = Config {
            port: req.port,
            host: req.host,
            open_browser: req.open_browser.unwrap_or(old_cfg.open_browser),
            saves_dir: req.saves_dir,
            // A remembered career in the old folder means nothing in the new one — drop it so the
            // next startup picks a career from the folder itself.
            active_career: if saves_dir_changed {
                None
            } else {
                old_cfg.active_career
            },
            // Folded into `active_career` on read and never written back.
            legacy_data_file: None,
            poll_ms: req.poll_ms,
            record_practice: req.record_practice,
            record_qualify: req.record_qualify,
            record_race: req.record_race,
            show_track_map: req.show_track_map,
            track_map_max_points: req.track_map_max_points,
            spotter_enabled: old_cfg.spotter_enabled,
            spotter_voice: old_cfg.spotter_voice,
            spotter_name: old_cfg.spotter_name,
            custom_ai_dir: req.custom_ai_dir,
            enforce_team_eligibility: req.enforce_team_eligibility,
            hide_locked_teams: req.hide_locked_teams,
            // Taken raw here and clamped by `normalize_economy` below, which reads the bounds off
            // `Config::offer_params` / `prize_params`. Clamping each field on its own could not
            // enforce the relationship *between* a pair: a floor above its top would be stored
            // and shown by the Config tab while the economy quietly ran on something else.
            contract_top_salary: req.contract_top_salary.unwrap_or(old_cfg.contract_top_salary),
            contract_floor_salary: req
                .contract_floor_salary
                .unwrap_or(old_cfg.contract_floor_salary),
            contract_buy_in_per_point: req
                .contract_buy_in_per_point
                .unwrap_or(old_cfg.contract_buy_in_per_point),
            contract_pay_driver_margin: req
                .contract_pay_driver_margin
                .unwrap_or(old_cfg.contract_pay_driver_margin),
            champion_prize: req.champion_prize.unwrap_or(old_cfg.champion_prize),
            last_place_prize: req.last_place_prize.unwrap_or(old_cfg.last_place_prize),
            // Only the default for the *next* career — existing saves carry their own figure, so
            // moving this never changes a balance that has already been founded.
            starting_balance: req
                .starting_balance
                .unwrap_or(old_cfg.starting_balance)
                .clamp(0, 1_000_000_000),
            starting_rating: req.starting_rating.clamp(0.0, 100.0),
            rating_strictness: req.rating_strictness.clamp(-50.0, 50.0),
            eligibility_gates: req.eligibility_gates,
            rating_half_life: req.rating_half_life.max(0.0),
            count_retirements: req.count_retirements,
            retirement_min_laps_down: req.retirement_min_laps_down.min(50),
            retirement_distance_pct: req.retirement_distance_pct.clamp(0.0, 100.0),
            offer_margin: req.offer_margin.clamp(0.0, 100.0),
            // Taken raw and cleaned by `normalize_class_years` below, the same way the economy
            // is: the clamp and the "same as the built-in table" rule live on `Config`, so this
            // cannot state a second version of either.
            class_years: req.class_years.unwrap_or(old_cfg.class_years),
        };
        // Store what the economy will actually run on, so the form cannot show one thing while
        // the grid uses another.
        new_cfg.normalize_economy();
        new_cfg.normalize_class_years();

        crate::config::save(&self.config_path, &new_cfg).map_err(internal)?;

        Ok(ConfigPatched {
            classes: class_year_rows(&new_cfg),
            config: new_cfg,
            restart_required,
        })
    }

    /// Copies the current Config rating settings into this career.
    ///
    /// The deliberate exception to the stamp being permanent, and the same shape as
    /// `custom_ai::set_baseline` is to the one-time `ensure_baseline`: retuning difficulty
    /// mid-career has to be possible, but it must be an act rather than a side effect of editing
    /// a form. Every rating figure and every team bar in the career moves with it, including the
    /// ones past seasons were judged on, which is why the client confirms first.
    pub fn adopt_rating(&self) -> AppResult<RatingParams> {
        let adopted = self.config().rating_params();
        self.store.write().unwrap().rating_params = Some(adopted);
        self.persist()?;
        Ok(adopted)
    }

    pub fn spotter_status(&self) -> SpotterStatus {
        let cfg = self.spotter_focus.lock().unwrap();
        SpotterStatus {
            enabled: cfg.enabled,
            player: cfg.name.clone(),
            voice: cfg.voice.clone(),
        }
    }

    /// Changes the running spotter and records the change in config.json.
    pub fn patch_spotter(&self, patch: SpotterPatch) -> SpotterStatus {
        let status = {
            let mut cfg = self.spotter_focus.lock().unwrap();
            if let Some(b) = patch.enabled {
                cfg.enabled = b;
            }
            if let Some(player) = patch.player {
                cfg.name = player;
            }
            if let Some(voice) = patch.voice {
                cfg.voice = voice;
            }
            SpotterStatus {
                enabled: cfg.enabled,
                player: cfg.name.clone(),
                voice: cfg.voice.clone(),
            }
        };
        let mut file_cfg = self.config();
        file_cfg.spotter_enabled = status.enabled;
        file_cfg.spotter_voice = status.voice.clone();
        file_cfg.spotter_name = status.player.clone();
        // Guarded and atomic: never writes defaults over a config that could not be read. A
        // failure leaves the running spotter changed, which is what the user asked for.
        let _ = crate::config::save(&self.config_path, &file_cfg);
        status
    }

    /// The saved radar layout for a track, as the JSON array it was saved as.
    pub fn track_layout(&self, track: &str) -> Option<Vec<u8>> {
        let file = self
            .layouts_dir
            .join(format!("{}.json", crate::web::http::track_slug(track)));
        file.exists().then(|| std::fs::read(&file).unwrap_or_default())
    }

    /// Saves a track's radar layout. Refused with too few points to be a lap.
    pub fn save_track_layout(&self, track: &str, json: &[u8]) -> AppResult<()> {
        let count = serde_json::from_slice::<serde_json::Value>(json)
            .ok()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0);
        if count < 300 {
            return Err(bad("too few points"));
        }
        let file = self
            .layouts_dir
            .join(format!("{}.json", crate::web::http::track_slug(track)));
        std::fs::write(&file, json).map_err(|e| internal(e.to_string()))
    }
}

/// Every class the Config tab offers a season year for: the rosters AMS2 would actually read,
/// plus any class an override names.
///
/// The second half matters — an override whose roster has since been removed from the folder
/// would otherwise be unreachable from the UI while still being stored, and the only way to
/// clear it would be to edit `config.json` by hand.
///
/// Ordered the way the Car Performance tab orders classes (year, then name) so the list reads as
/// the timeline it is, and so a corrected year visibly moves its class into place on save.
pub fn class_year_rows(cfg: &Config) -> Vec<ClassYearRow> {
    use crate::roster::{custom_ai, season_years};
    let years = cfg.class_years();
    let mut names: std::collections::BTreeSet<String> = years.keys().cloned().collect();
    if let Some(dir) = cfg
        .custom_ai_dir
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        for file in custom_ai::list_files_for_known_classes(std::path::Path::new(dir)) {
            names.insert(custom_ai::class_of_file(&file).to_string());
        }
    }
    let mut rows: Vec<ClassYearRow> = names
        .into_iter()
        .map(|class| {
            let builtin = season_years::season_year(&class);
            let overridden = years.contains_key(&class);
            ClassYearRow {
                year: years.get(&class).copied().or(builtin),
                builtin,
                overridden,
                class,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        a.year
            .unwrap_or(u16::MAX)
            .cmp(&b.year.unwrap_or(u16::MAX))
            .then_with(|| a.class.cmp(&b.class))
    });
    rows
}
