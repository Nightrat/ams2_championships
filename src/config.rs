use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_port()          -> u16    { 8080 }
fn default_host()          -> String { "127.0.0.1".into() }
fn default_poll_ms()       -> u64    { 200 }
fn default_true()          -> bool   { true }
fn default_show_track_map()    -> bool { true }
fn default_track_map_max_points() -> u32 { 5000 }

#[derive(Serialize, Deserialize)]
pub struct Config {
    /// HTTP/WebSocket port.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Bind address. Use "0.0.0.0" to allow LAN access.
    #[serde(default = "default_host")]
    pub host: String,
    /// Path to the career JSON data file. Defaults to championships/ams2_career.json
    /// next to the executable.
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
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: default_port(),
            host: default_host(),
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
