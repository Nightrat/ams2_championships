use crate::ams2_shared_memory::{read_live_session, LiveSessionData};

#[derive(PartialEq, Clone, Copy)]
enum GapCategory { Close, Medium, Clear }

// Simplified flag state for transition detection
#[derive(PartialEq, Clone, Copy)]
enum FlagState { None, Yellow, SafetyCar, Red }

#[derive(PartialEq, Clone, Copy)]
enum FuelWarning { Ok, Low, Critical }

#[derive(PartialEq, Clone, Copy)]
enum TyreWarning { Ok, Worn, Critical }

pub struct SpotterState {
    prev_position:    u32,   // last announced position
    pending_position: u32,   // real-time position; may differ while debouncing
    pos_cooldown:     u32,   // frames until pending_position is announced
    prev_laps:        u32,
    prev_gap_ahead:   GapCategory,
    prev_gap_behind:  GapCategory,
    prev_flag:        FlagState,
    prev_fuel:        FuelWarning,
    prev_tyre:        [TyreWarning; 4],
    prev_best_lap:    f32,
    start_fuel:       f32,
    track_key:        String,
}

fn fmt_lap_tts(secs: f32) -> String {
    let mins = secs as u32 / 60;
    let s = secs - mins as f32 * 60.0;
    if mins > 0 { format!("{mins} {s:.1}") } else { format!("{s:.1}") }
}

impl SpotterState {
    pub fn new() -> Self {
        Self {
            prev_position:   0,
            pending_position: 0,
            pos_cooldown:    0,
            prev_laps:       0,
            prev_gap_ahead:  GapCategory::Clear,
            prev_gap_behind: GapCategory::Clear,
            prev_flag:       FlagState::None,
            prev_fuel:       FuelWarning::Ok,
            prev_tyre:       [TyreWarning::Ok; 4],
            prev_best_lap:   0.0,
            start_fuel:      0.0,
            track_key:       String::new(),
        }
    }

    fn reset_session(&mut self) {
        self.prev_position    = 0;
        self.pending_position = 0;
        self.pos_cooldown     = 0;
        self.prev_laps        = 0;
        self.prev_gap_ahead  = GapCategory::Clear;
        self.prev_gap_behind = GapCategory::Clear;
        self.prev_flag       = FlagState::None;
        self.prev_fuel       = FuelWarning::Ok;
        self.prev_tyre       = [TyreWarning::Ok; 4];
        self.prev_best_lap   = 0.0;
        self.start_fuel      = 0.0;
    }

    /// `focus`: if `Some(name)`, track that participant by name; otherwise track the viewed player.
    pub fn update(&mut self, data: &LiveSessionData, focus: &Option<String>) -> Vec<String> {
        let mut events = Vec::new();

        if !data.connected {
            self.reset_session();
            return events;
        }

        // Reset on track, session, or focus change
        let focus_key = focus.as_deref().unwrap_or("");
        let key = format!("{}|{}|{}|{}", data.track_location, data.track_variation, data.session_state, focus_key);
        if key != self.track_key {
            self.track_key = key;
            self.reset_session();
            return events;
        }

        let player = match focus {
            Some(name) => data.participants.iter().find(|p| p.name == *name),
            None       => data.participants.iter().find(|p| p.is_player),
        };
        let Some(player) = player else {
            return events;
        };

        let is_race = data.session_state == 5;

        // ── Position change (debounced) ───────────────────────────────────────
        // Debounce for ~2 s so a rapid stack of overtakes collapses into one
        // announcement for the final settled position.
        const POS_DEBOUNCE: u32 = 10; // frames at 200 ms poll ≈ 2 s
        let real_pos = player.race_position;
        if real_pos > 0 {
            if real_pos != self.pending_position {
                self.pending_position = real_pos;
                if self.prev_position > 0 {
                    // Position moved — (re)start the settle timer.
                    self.pos_cooldown = POS_DEBOUNCE;
                } else {
                    // First reading of the session: initialise silently.
                    self.prev_position = real_pos;
                }
            }
            if self.pos_cooldown > 0 {
                self.pos_cooldown -= 1;
                if self.pos_cooldown == 0 && self.pending_position != self.prev_position {
                    events.push(format!("Position {}", self.pending_position));
                    self.prev_position = self.pending_position;
                }
            }
        }

        // ── Lap completion (race only) ─────────────────────────────────────────
        if is_race && player.laps_completed > self.prev_laps && self.prev_laps > 0 {
            events.push(format!("Lap {}", player.current_lap));
        }
        if is_race {
            self.prev_laps = player.laps_completed;
        }

        // ── Fastest / personal-best lap ──────────────────────────────────────
        let best = player.fastest_lap_time;
        if best > 0.0 && best != self.prev_best_lap {
            if self.prev_best_lap > 0.0 {
                let overall_best = data.participants.iter()
                    .filter(|p| p.fastest_lap_time > 0.0)
                    .map(|p| p.fastest_lap_time)
                    .fold(f32::MAX, f32::min);
                if best <= overall_best {
                    events.push(format!("Fastest lap, {}", fmt_lap_tts(best)));
                } else {
                    events.push(format!("Personal best, {}", fmt_lap_tts(best)));
                }
            }
            self.prev_best_lap = best;
        }

        // ── Gap ahead advisory (race only) ────────────────────────────────────
        if is_race && player.interval_gap_secs >= 0.0 {
            let cat = if player.interval_gap_secs < 1.5 {
                GapCategory::Close
            } else if player.interval_gap_secs < 5.0 {
                GapCategory::Medium
            } else {
                GapCategory::Clear
            };

            if cat != self.prev_gap_ahead {
                match cat {
                    GapCategory::Close => {
                        let name = data.participants.iter()
                            .find(|p| p.race_position == player.race_position - 1)
                            .map(|p| p.name.as_str())
                            .unwrap_or("car ahead");
                        events.push(format!("{:.1} seconds to {name}", player.interval_gap_secs));
                    }
                    GapCategory::Clear if self.prev_gap_ahead == GapCategory::Close => {
                        events.push("Clear ahead".to_string());
                    }
                    _ => {}
                }
                self.prev_gap_ahead = cat;
            }
        }

        // ── Gap behind advisory (race only) ───────────────────────────────────
        // The car directly behind has interval_gap_secs equal to its gap to us.
        if is_race && player.race_position > 1 {
            let behind = data.participants.iter()
                .find(|p| p.race_position == player.race_position + 1);

            if let Some(behind) = behind {
                let behind_gap = behind.interval_gap_secs;
                let cat = if behind_gap < 1.5 {
                    GapCategory::Close
                } else if behind_gap < 2.0 {
                    GapCategory::Medium
                } else {
                    GapCategory::Clear
                };

                if cat != self.prev_gap_behind {
                    match cat {
                        GapCategory::Close => {
                            events.push(format!("{:.1} seconds to {} behind", behind_gap, behind.name));
                        }
                        GapCategory::Clear if self.prev_gap_behind != GapCategory::Clear => {
                            events.push("Clear behind".to_string());
                        }
                        _ => {}
                    }
                    self.prev_gap_behind = cat;
                }
            }
        }

        // ── Flag status ───────────────────────────────────────────────────────
        let flag = match data.race_flag_reason {
            6 => FlagState::SafetyCar,
            7 => FlagState::SafetyCar, // returning — keep SC state until green
            _ => match data.race_flag_colour {
                5 => FlagState::Red,
                6 | 7 => FlagState::Yellow,
                _ => FlagState::None,
            },
        };

        if flag != self.prev_flag {
            match flag {
                FlagState::Yellow    => events.push("Yellow flag".to_string()),
                FlagState::SafetyCar => {
                    if data.race_flag_reason == 7 {
                        events.push("Safety Car returning".to_string());
                    } else {
                        events.push("Safety Car deployed".to_string());
                    }
                }
                FlagState::Red  => events.push("Red flag".to_string()),
                FlagState::None => {
                    match self.prev_flag {
                        FlagState::None => {}
                        _ => events.push("Green flag".to_string()),
                    }
                }
            }
            self.prev_flag = flag;
        }

        // ── Fuel warning (race only) ───────────────────────────────────────────
        if is_race {
            let lvl = data.player_telemetry.fuel_level;
            // Capture starting fuel on the first frame it's available
            if self.start_fuel <= 0.0 && lvl > 0.0 {
                self.start_fuel = lvl;
            }

            let used = self.start_fuel - lvl;
            let laps_done = player.laps_completed;

            // Estimate laps remaining from average consumption; fall back to capacity %
            let fuel_laps: Option<u32> = if laps_done > 0 && used > 0.0 {
                let per_lap = used / laps_done as f32;
                Some((lvl / per_lap) as u32)
            } else {
                None
            };

            let fuel = match fuel_laps {
                Some(l) if l <= 2 => FuelWarning::Critical,
                Some(l) if l <= 5 => FuelWarning::Low,
                None if data.player_telemetry.fuel_capacity > 0.0 => {
                    let pct = lvl / data.player_telemetry.fuel_capacity;
                    if pct < 0.05 { FuelWarning::Critical }
                    else if pct < 0.15 { FuelWarning::Low }
                    else { FuelWarning::Ok }
                }
                _ => FuelWarning::Ok,
            };

            if fuel != self.prev_fuel {
                let laps_str = fuel_laps.map(|l| format!("{l} laps remaining"))
                    .unwrap_or_else(|| format!("{lvl:.0} litres remaining"));
                match fuel {
                    FuelWarning::Low      => events.push(format!("Low fuel, {laps_str}")),
                    FuelWarning::Critical => events.push(format!("Fuel critical, {laps_str}")),
                    FuelWarning::Ok       => {}
                }
                self.prev_fuel = fuel;
            }
        }

        // ── Tyre wear warning (race only) ─────────────────────────────────────
        if is_race {
            const TYRE_NAMES: [&str; 4] = ["front left", "front right", "rear left", "rear right"];
            for i in 0..4 {
                let wear = data.player_telemetry.tyre_wear[i];
                if wear <= 0.0 { continue; }
                let tw = if wear >= 0.9 {
                    TyreWarning::Critical
                } else if wear >= 0.7 {
                    TyreWarning::Worn
                } else {
                    TyreWarning::Ok
                };
                if tw != self.prev_tyre[i] {
                    match tw {
                        TyreWarning::Worn     => events.push(format!("{} tyre worn", TYRE_NAMES[i])),
                        TyreWarning::Critical => events.push(format!("{} tyre critical", TYRE_NAMES[i])),
                        TyreWarning::Ok       => {}
                    }
                    self.prev_tyre[i] = tw;
                }
            }
        }

        events
    }
}

// ── TTS spotter thread ────────────────────────────────────────────────────────
//
// Uses a persistent PowerShell process with System.Speech.Synthesis (classic
// SAPI / .NET Framework — always available on Windows, no language packs needed).

#[cfg(windows)]
fn spawn_tts(voice: Option<&str>) -> Option<std::io::BufWriter<std::process::ChildStdin>> {
    use std::process::{Command, Stdio};
    let voice_cmd = match voice {
        Some(v) => format!("$v.SelectVoice('{}');", v.replace('\'', "''")),
        None    => String::new(),
    };
    let script = format!(
        "Add-Type -AssemblyName System.Speech;\
        $v=New-Object System.Speech.Synthesis.SpeechSynthesizer;\
        {voice_cmd}\
        while(($l=[Console]::ReadLine()) -ne $null){{\
          $v.SpeakAsync($l)|Out-Null\
        }}"
    );
    let child = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    Some(std::io::BufWriter::new(child.stdin?))
}

/// Returns the names of all installed SAPI voices.
#[cfg(windows)]
pub fn list_voices() -> Vec<String> {
    use std::process::Command;
    let script = "Add-Type -AssemblyName System.Speech;\
        (New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() |\
        ForEach-Object { $_.VoiceInfo.Name }";
    let Ok(out) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
    else { return vec![]; };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(not(windows))]
pub fn list_voices() -> Vec<String> { vec![] }

#[derive(Clone, Default)]
pub struct SpotterConfig {
    pub enabled: bool,
    pub name:    Option<String>,
    pub voice:   Option<String>,
}

/// Shared spotter configuration (enabled flag, focused player name, TTS voice).
pub type Focus = std::sync::Arc<std::sync::Mutex<SpotterConfig>>;

/// Spawn a background thread that speaks spotter events through system audio.
/// Returns immediately; the thread runs for the lifetime of the process.
#[cfg(windows)]
pub fn start(poll_ms: u64, focus: Focus) {
    std::thread::spawn(move || {
        use std::io::Write;
        let mut tts: Option<std::io::BufWriter<std::process::ChildStdin>> = None;
        let mut active_voice: Option<Option<String>> = None; // None = not yet started
        let mut state = SpotterState::new();
        loop {
            let cfg = focus.lock().unwrap().clone();
            if cfg.enabled {
                // (Re)start subprocess if voice changed or process died
                if tts.is_none() || active_voice.as_ref() != Some(&cfg.voice) {
                    active_voice = Some(cfg.voice.clone());
                    tts = spawn_tts(cfg.voice.as_deref());
                    if tts.is_none() {
                        eprintln!("Spotter: failed to start TTS process");
                    }
                }
                if let Some(ref mut w) = tts {
                    let data = read_live_session();
                    let mut failed = false;
                    for event in state.update(&data, &cfg.name) {
                        if writeln!(w, "{event}").is_err() || w.flush().is_err() {
                            failed = true;
                            break;
                        }
                    }
                    if failed { tts = None; }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(poll_ms));
        }
    });
}

#[cfg(not(windows))]
pub fn start(_poll_ms: u64, _focus: Focus) {}
