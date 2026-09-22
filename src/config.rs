use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use crate::contracts::{OfferParams, PrizeParams};
use crate::driver_rating::{Gates, RatingParams};

fn default_port() -> u16 {
    8080
}
fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_poll_ms() -> u64 {
    200
}
fn default_true() -> bool {
    true
}
fn default_show_track_map() -> bool {
    false
}
fn default_track_map_max_points() -> u32 {
    5000
}
fn default_starting_rating() -> f32 {
    RatingParams::default().starting_rating
}
fn default_rating_half_life() -> f32 {
    RatingParams::default().recency_half_life
}
fn default_retirement_laps() -> u32 {
    RatingParams::default().retirement_min_laps_down
}
fn default_retirement_distance_pct() -> f32 {
    RatingParams::default().retirement_distance * 100.0
}
fn default_offer_margin() -> f32 {
    RatingParams::default().offer_margin
}
fn default_top_salary() -> i64 {
    OfferParams::default().top_salary
}
fn default_floor_salary() -> i64 {
    OfferParams::default().floor_salary
}
fn default_buy_in() -> i64 {
    OfferParams::default().buy_in_per_point
}
fn default_pay_driver_margin() -> f32 {
    OfferParams::default().pay_driver_margin
}
fn default_champion_prize() -> i64 {
    PrizeParams::default().champion_prize
}
fn default_floor_prize() -> i64 {
    PrizeParams::default().floor_prize
}
/// Enough that a brand-new career can buy into **at least two** of the seats that ask for
/// sponsorship — a choice of way in, rather than one take-it-or-leave-it.
///
/// Measured, not guessed. Across the eight rosters in `docs/custom_ai_files_with_perf_scalars`
/// the second-cheapest pay-driver seat for a driver on the starting rating runs to 4,949,999 at
/// worst (F-Retro_Gen3); this covers it with a little room.
/// `test_a_new_career_can_buy_into_two_pay_seats_on_every_shipped_grid` re-measures it and fails
/// if retuning the economy moves the costs out from under it.
///
/// It applies to **every** shipped class now. Two used to be exceptions — F-Vintage_Gen2 offered
/// one pay seat and F-Classic_Gen3 none — because their back rows were reachable on merit and so
/// were free rather than for sale. [`OfferParams::pay_driver_margin`] closed that: a team at the
/// back sells to anyone it does not actively want.
///
/// Falling short of every seat is not a dead end — `contracts::offers_for_with` drops the
/// cheapest to whatever the career holds. This figure is what stops that last resort being the
/// *normal* way a career begins.
fn default_starting_balance() -> i64 {
    5_000_000
}

/// Upper bound on every configurable money figure. Not a rule about what a career should be
/// worth — just far enough out that a slipped digit cannot overflow the arithmetic downstream.
const MONEY_MAX: i64 = 1_000_000_000;

#[derive(Serialize, Deserialize)]
pub struct Config {
    /// HTTP/WebSocket port.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Bind address. Use "0.0.0.0" to allow LAN access.
    #[serde(default = "default_host")]
    pub host: String,
    /// Folder holding the career save files (`*.json`) and their track layouts.
    /// Defaults to `championships` next to the executable.
    #[serde(default)]
    pub saves_dir: Option<String>,
    /// Name of the *active* career — the folder name inside `saves_dir`, not a path. Set by the
    /// career switcher, not by hand. When unset, or naming a career that is no longer there, the
    /// server picks one from `saves_dir` on startup.
    ///
    /// A name rather than a path because that is how every other part of the app addresses a
    /// save: the routes take names, `sanitize_name` gates names, and `saves::existing_save_path`
    /// resolves one. A second setting holding a full path could contradict `saves_dir`, and used
    /// to.
    #[serde(default)]
    pub active_career: Option<String>,
    /// Pre-folders spelling of [`Self::active_career`]: the active save's full **path**. Read
    /// once so a config written by an older build keeps the career it was on — [`read_config`]
    /// folds it into `active_career` and it is dropped on the next write.
    #[serde(default, rename = "data_file", skip_serializing)]
    pub legacy_data_file: Option<String>,
    /// Shared memory poll interval in milliseconds (live view refresh rate).
    #[serde(default = "default_poll_ms")]
    pub poll_ms: u64,
    /// Automatically record practice sessions.
    #[serde(default = "default_true")]
    pub record_practice: bool,
    /// Automatically record qualifying sessions.
    #[serde(default = "default_true")]
    pub record_qualify: bool,
    /// Automatically record race sessions.
    #[serde(default = "default_true")]
    pub record_race: bool,
    /// Show the track radar canvas in the live timing view.
    #[serde(default = "default_show_track_map")]
    pub show_track_map: bool,
    /// Maximum number of unique grid cells stored for the track radar before saving stops accumulating.
    #[serde(default = "default_track_map_max_points")]
    pub track_map_max_points: u32,
    /// Whether the server-side voice spotter is enabled.
    #[serde(default)]
    pub spotter_enabled: bool,
    /// TTS voice name for the spotter (None = system default).
    #[serde(default)]
    pub spotter_voice: Option<String>,
    /// Spotter focus player name in multiplayer (None = viewed player).
    #[serde(default)]
    pub spotter_name: Option<String>,
    /// Folder containing AMS2 Custom AI Driver XML files (e.g. .../UserData/CustomAIDrivers).
    /// Used to look up team/livery names for drivers in a championship.
    #[serde(default)]
    pub custom_ai_dir: Option<String>,
    /// Refuse to set a championship's player team when the driver rating has not earned that
    /// seat. Turning this off keeps the ratings visible but stops them blocking anything.
    #[serde(default = "default_true")]
    pub enforce_team_eligibility: bool,
    /// List locked teams in the championship's team picker, disabled and showing what they ask
    /// for, instead of hiding them. Seeing the ladder is better motivation than an empty list,
    /// so this defaults off; it changes nothing about what may be claimed.
    #[serde(default)]
    pub hide_locked_teams: bool,
    /// Per-season pay for the quickest car on the grid, in credits.
    #[serde(default = "default_top_salary")]
    pub contract_top_salary: i64,
    /// Per-season pay for the slowest car on the grid, in credits.
    #[serde(default = "default_floor_salary")]
    pub contract_floor_salary: i64,
    /// Credits demanded per rating point short of a locked team's bar. Zero switches pay-driver
    /// seats off, so a team out of reach simply makes no offer.
    #[serde(default = "default_buy_in")]
    pub contract_buy_in_per_point: i64,
    /// Rating points clear of a back-marker's bar before it pays the driver instead of selling
    /// them the seat. Zero means clearing the bar is enough anywhere on the grid, which leaves
    /// the slowest team free to whoever can beat its incumbent.
    #[serde(default = "default_pay_driver_margin")]
    pub contract_pay_driver_margin: f32,
    /// Credits for winning a championship.
    #[serde(default = "default_champion_prize")]
    pub champion_prize: i64,
    /// Credits for finishing last among the drivers who scored.
    #[serde(default = "default_floor_prize")]
    pub last_place_prize: i64,
    /// Credits a newly created career starts with. Recorded on the save at creation, so changing
    /// this never moves the balance of a career that already exists.
    #[serde(default = "default_starting_balance")]
    pub starting_balance: i64,

    // ── Driver rating tuning ─────────────────────────────────────────────────
    // Defaults reproduce the behaviour these numbers were hard-coded to. See
    // `driver_rating::RatingParams`, which is what they are read into.
    /// Rating a driver with no recorded results starts at, 0–100.
    #[serde(default = "default_starting_rating")]
    pub starting_rating: f32,
    /// Rating points added to every team's requirement; negative opens the grid up.
    #[serde(default)]
    pub rating_strictness: f32,
    /// Which bars a team's requirement is built from.
    #[serde(default)]
    pub eligibility_gates: Gates,
    /// Results this far back count half. Zero weighs a whole career equally.
    #[serde(default = "default_rating_half_life")]
    pub rating_half_life: f32,
    /// Whether a retirement costs a point of finish rate.
    #[serde(default = "default_true")]
    pub count_retirements: bool,
    /// Laps behind the leader before a car counts as retired rather than lapped.
    #[serde(default = "default_retirement_laps")]
    pub retirement_min_laps_down: u32,
    /// Percentage of the leader's distance a car must fall short of to count as retired.
    #[serde(default = "default_retirement_distance_pct")]
    pub retirement_distance_pct: f32,
    /// How far below a team's requirement a driver may sit and still be offered the seat.
    #[serde(default = "default_offer_margin")]
    pub offer_margin: f32,

    /// Season year per car class, overriding the table shipped in [`crate::season_years`].
    ///
    /// Keyed on the class name — the file stem AMS2 matches, the same key the built-in table
    /// uses. Only classes somebody has actually answered for are stored: [`Config::class_years`]
    /// drops anything that merely restates the built-in year, so the shipped table stays the
    /// source of truth for every class nobody has had an opinion about and keeps applying if it
    /// is ever corrected.
    #[serde(default)]
    pub class_years: BTreeMap<String, u16>,
}

impl Config {
    /// The rating tuning, clamped to sane ranges.
    ///
    /// Hand-edited config files are the norm here, so the bounds are enforced on the way out
    /// rather than trusted on the way in: a negative half-life or a strictness of 400 would
    /// otherwise produce a rating no UI could explain.
    pub fn rating_params(&self) -> RatingParams {
        RatingParams {
            starting_rating: self.starting_rating.clamp(0.0, 100.0),
            strictness: self.rating_strictness.clamp(-50.0, 50.0),
            gates: self.eligibility_gates,
            recency_half_life: self.rating_half_life.max(0.0),
            count_retirements: self.count_retirements,
            retirement_min_laps_down: self.retirement_min_laps_down,
            // The config speaks in percent because that is how the hint reads; the rating math
            // wants the fraction. One conversion, in one place.
            retirement_distance: self.retirement_distance_pct.clamp(0.0, 100.0) / 100.0,
            // Negative would make a team refuse a driver who has cleared its bar; past 100 every
            // bar on the grid is within reach of any rating, which is a setting rather than a
            // mistake — it simply removes the locked tier.
            offer_margin: self.offer_margin.clamp(0.0, 100.0),
        }
    }

    /// The contract economy, clamped. Only the money is configurable; deal length and objective
    /// slack stay on [`OfferParams::default`] until there is a reason to expose them.
    ///
    /// The floor is held below the top rather than swapped when a hand-edited config inverts
    /// them: an inverted pair would pay the back of the grid more than the front, which no UI
    /// could explain, and silently reordering someone's numbers is worse than ignoring one.
    pub fn offer_params(&self) -> OfferParams {
        let top = self.contract_top_salary.clamp(1, MONEY_MAX);
        OfferParams {
            top_salary: top,
            floor_salary: self.contract_floor_salary.clamp(1, top),
            buy_in_per_point: self.contract_buy_in_per_point.clamp(0, MONEY_MAX),
            pay_driver_margin: if self.contract_pay_driver_margin.is_finite() {
                self.contract_pay_driver_margin.clamp(0.0, 100.0)
            } else {
                default_pay_driver_margin()
            },
            ..OfferParams::default()
        }
    }

    /// The prize money table, clamped the same way.
    pub fn prize_params(&self) -> PrizeParams {
        let top = self.champion_prize.clamp(1, MONEY_MAX);
        PrizeParams {
            champion_prize: top,
            floor_prize: self.last_place_prize.clamp(1, top),
        }
    }

    /// The season-year overrides, cleaned: names trimmed, years held inside
    /// [`crate::season_years::YEAR_MIN`]..=[`crate::season_years::YEAR_MAX`], and any entry that
    /// merely repeats the built-in year dropped.
    ///
    /// Dropping the agreeing ones is what keeps the shipped table meaningful. Without it, saving
    /// the Config tab once would freeze every class the folder happens to hold at whatever this
    /// build thinks today — a later correction to `SEASON_YEARS` would then never reach anyone
    /// who had pressed Save, and `config.json` would carry a line per class saying nothing.
    pub fn class_years(&self) -> BTreeMap<String, u16> {
        self.class_years
            .iter()
            .filter_map(|(class, year)| {
                let class = class.trim();
                if class.is_empty() {
                    return None;
                }
                let year = (*year).clamp(crate::season_years::YEAR_MIN, crate::season_years::YEAR_MAX);
                if crate::season_years::season_year(class) == Some(year) {
                    return None;
                }
                Some((class.to_string(), year))
            })
            .collect()
    }

    /// Writes the cleaned overrides back over the raw field, for the same reason
    /// [`Self::normalize_economy`] exists: what is stored has to be what is used, or the Config
    /// tab shows one year while every table sorts by another.
    pub fn normalize_class_years(&mut self) {
        self.class_years = self.class_years();
    }

    /// Writes the clamped economy back over the raw fields, so what is stored is what is used.
    ///
    /// Clamping only on the way out is not enough for a *pair*: `PATCH /api/config` would
    /// happily store a floor above its top, the Config tab would show it, and the economy would
    /// quietly run on something else. Rather than repeat the rules, this reads them back off the
    /// accessors above — they stay the single definition, and the two cannot drift apart.
    pub fn normalize_economy(&mut self) {
        let offers = self.offer_params();
        let prizes = self.prize_params();
        self.contract_top_salary = offers.top_salary;
        self.contract_floor_salary = offers.floor_salary;
        self.contract_buy_in_per_point = offers.buy_in_per_point;
        self.contract_pay_driver_margin = offers.pay_driver_margin;
        self.champion_prize = prizes.champion_prize;
        self.last_place_prize = prizes.floor_prize;
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: default_port(),
            host: default_host(),
            saves_dir: None,
            active_career: None,
            legacy_data_file: None,
            poll_ms: default_poll_ms(),
            record_practice: default_true(),
            record_qualify: default_true(),
            record_race: default_true(),
            show_track_map: default_show_track_map(),
            track_map_max_points: default_track_map_max_points(),
            spotter_enabled: false,
            spotter_voice: None,
            spotter_name: None,
            custom_ai_dir: None,
            enforce_team_eligibility: default_true(),
            hide_locked_teams: false,
            contract_top_salary: default_top_salary(),
            contract_floor_salary: default_floor_salary(),
            contract_buy_in_per_point: default_buy_in(),
            contract_pay_driver_margin: default_pay_driver_margin(),
            champion_prize: default_champion_prize(),
            last_place_prize: default_floor_prize(),
            starting_balance: default_starting_balance(),
            starting_rating: default_starting_rating(),
            rating_strictness: RatingParams::default().strictness,
            eligibility_gates: Gates::default(),
            rating_half_life: default_rating_half_life(),
            count_retirements: RatingParams::default().count_retirements,
            retirement_min_laps_down: default_retirement_laps(),
            retirement_distance_pct: default_retirement_distance_pct(),
            offer_margin: default_offer_margin(),
            class_years: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;

/// Load config from `path`. If the file does not exist, write defaults and return them.
/// On parse error, print a warning and return defaults.
/// Reads the config, writing defaults only when the file does not exist yet.
///
/// **A read never writes.** It used to rewrite the file on every call, to pick up fields added
/// since the last run — but this is called several times per HTTP request, from more than one
/// thread, and `fs::write` truncates before it writes. A reader landing in that window saw an
/// empty file, reported `EOF while parsing a value at line 1 column 0`, and fell back to
/// defaults; the next call would then persist those defaults over the user's real settings.
/// The upgrade rewrite now happens once, at startup, in [`load_and_upgrade`].
pub fn load_or_create(path: &Path) -> Config {
    if path.exists() {
        match read_config(path) {
            Ok(cfg) => return cfg,
            // Defaults let the server start so the problem can be seen and fixed. They are safe
            // to return only because [`save`] refuses to write over a config it could not read.
            Err(e) => eprintln!(
                "ERROR: config file {} could not be read ({e}) — running on defaults. \
                 It will NOT be written to, so your settings are intact. Fix or delete it, \
                 then restart.",
                path.display()
            ),
        }
        return Config::default();
    }

    let cfg = Config::default();
    match save(path, &cfg) {
        Ok(()) => println!("Config:         {} (created with defaults)", path.display()),
        Err(e) => eprintln!("Warning: could not write default config ({e})"),
    }
    cfg
}

/// [`load_or_create`] plus the one-time rewrite that persists fields added since the last run.
///
/// Startup only. Doing this on every read is what made the file momentarily empty for other
/// threads — see [`load_or_create`].
pub fn load_and_upgrade(path: &Path) -> Config {
    let cfg = load_or_create(path);
    if path.exists() {
        // A failure here is not worth reporting twice: either the file is unreadable, which
        // `load_or_create` has already said, or `save` has refused and said so itself.
        let _ = save(path, &cfg);
    }
    cfg
}

fn read_config(path: &Path) -> Result<Config, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    // Notepad and PowerShell both write a BOM by default, exactly as for career saves.
    let mut cfg: Config = serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text))
        .map_err(|e| e.to_string())?;
    // Fold the pre-folders `data_file` path into `active_career`. Done here rather than in
    // `load_and_upgrade` so that every reader sees the same answer, whether or not the one-time
    // rewrite at startup has happened yet. A path that named a save outside `saves_dir` resolves
    // to a name that is not in the folder, and startup falls through to picking one — which is
    // the support for an active save outside the saves folder ending, deliberately.
    if cfg.active_career.is_none() {
        cfg.active_career = cfg
            .legacy_data_file
            .as_deref()
            .map(Path::new)
            .and_then(crate::saves::save_name_of);
    }
    Ok(cfg)
}

/// Writes the config, unless that would overwrite a file that exists but could not be read.
///
/// The same rule as career saves: a config that will not parse was never loaded, so whatever is
/// in memory did not come from it — everything the caller is about to write is defaults, and
/// writing them would destroy the user's real settings.
///
/// The write goes to a sibling temp file and is then renamed over the target, so a concurrent
/// reader sees either the old file or the new one and never a half-written one.
pub fn save(path: &Path, cfg: &Config) -> Result<(), String> {
    if path.exists() && read_config(path).is_err() {
        let msg = format!(
            "refusing to overwrite {}: it exists but could not be read. \
             Fix or delete it — your settings have not been changed.",
            path.display()
        );
        eprintln!("ERROR: {msg}");
        return Err(msg);
    }
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}
