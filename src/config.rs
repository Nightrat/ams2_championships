use serde::{Deserialize, Serialize};
use std::path::Path;

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
    /// Path to the *active* career save file. Set by the career switcher, not by hand.
    /// When unset (or pointing at a file that no longer exists) the server picks a save
    /// from `saves_dir` on startup.
    #[serde(default)]
    pub data_file: Option<String>,
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
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: default_port(),
            host: default_host(),
            saves_dir: None,
            data_file: None,
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
            starting_rating: default_starting_rating(),
            rating_strictness: RatingParams::default().strictness,
            eligibility_gates: Gates::default(),
            rating_half_life: default_rating_half_life(),
            count_retirements: RatingParams::default().count_retirements,
            retirement_min_laps_down: default_retirement_laps(),
            retirement_distance_pct: default_retirement_distance_pct(),
        }
    }
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;

/// Load config from `path`. If the file does not exist, write defaults and return them.
/// On parse error, print a warning and return defaults.
pub fn load_or_create(path: &Path) -> Config {
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(cfg) => {
                    // Rewrite the file so any new fields added since last run are persisted.
                    if let Ok(updated) = serde_json::to_string_pretty(&cfg) {
                        let _ = std::fs::write(path, updated);
                    }
                    return cfg;
                }
                Err(e) => eprintln!("Warning: could not parse config file ({e}), using defaults"),
            },
            Err(e) => eprintln!("Warning: could not read config file ({e}), using defaults"),
        }
        return Config::default();
    }

    // File does not exist — write defaults.
    let cfg = Config::default();
    match serde_json::to_string_pretty(&cfg) {
        Ok(text) => {
            if let Err(e) = std::fs::write(path, &text) {
                eprintln!("Warning: could not write default config ({e})");
            } else {
                println!("Config:         {} (created with defaults)", path.display());
            }
        }
        Err(e) => eprintln!("Warning: could not serialize default config ({e})"),
    }
    cfg
}
