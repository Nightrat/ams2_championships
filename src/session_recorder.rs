use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::ams2_shared_memory::{read_live_session, LiveSessionData};
use crate::data_store::{persist, LapChartEntry, RecordedSession, SessionResult, SharedStore};

#[allow(dead_code)]
mod ams2 {
    /// session_state values
    pub const SESSION_PRACTICE: u32 = 1;
    pub const SESSION_QUALIFY:  u32 = 3;
    pub const SESSION_RACE:     u32 = 5;

    /// race_state values
    pub const RACE_STATE_NOT_STARTED: u32 = 1;
    pub const RACE_STATE_RACING:      u32 = 2;
    pub const RACE_STATE_FINISHED:    u32 = 3;
    pub const RACE_STATE_RETIRED:     u32 = 5;
    pub const RACE_STATE_DNF:         u32 = 6;

    /// game_state values
    pub const GAME_STATE_EXITED:  u32 = 0;
    pub const GAME_STATE_MENUS:   u32 = 1;
    pub const GAME_STATE_TIMEDOUT:u32 = 3;
    pub const GAME_STATE_IN_GAME: u32 = 2;
    pub const GAME_STATE_REPLAY:  u32 = 4;
}

use ams2::{SESSION_PRACTICE, SESSION_QUALIFY, SESSION_RACE};



#[cfg(test)]
#[path = "tests/session_recorder.rs"]
mod tests;

pub(crate) fn capture(store: &SharedStore, path: &PathBuf, session: &LiveSessionData, lap_chart: Vec<LapChartEntry>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let max_laps = session
        .participants
        .iter()
        .map(|p| p.laps_completed)
        .max()
        .unwrap_or(0);

    let results: Vec<SessionResult> = session
        .participants
        .iter()
        .map(|p| SessionResult {
            name: p.name.clone(),
            car_name: p.car_name.clone(),
            car_class: p.car_class.clone(),
            race_position: p.race_position,
            laps_completed: p.laps_completed,
            fastest_lap: p.fastest_lap_time,
            last_lap: p.last_lap_time,
            dnf: max_laps > 0 && p.laps_completed < max_laps,
        })
        .collect();

    let recorded = RecordedSession {
        id: now.to_string(),
        recorded_at: now,
        track: session.track_location.clone(),
        track_variation: session.track_variation.clone(),
        car_name: session.car_name.clone(),
        car_class: session.car_class.clone(),
        session_type: session.session_state,
        results,
        lap_chart,
    };

    let type_name = match session.session_state {
        SESSION_PRACTICE => "Practice",
        SESSION_QUALIFY  => "Qualify",
        SESSION_RACE     => "Race",
        _                => "Session",
    };

    println!(
        "[recorder] {} at {} recorded — {} participants",
        type_name, recorded.track, recorded.results.len()
    );

    {
        let mut data = store.write().unwrap();
        data.sessions.push(recorded);
    }
    persist(store, path);
}

/// Capture the current live session immediately, regardless of auto-record settings.
/// Returns an error string if nothing is worth capturing.
pub fn capture_current(store: &SharedStore, path: &PathBuf) -> Result<(), String> {
    let session = read_live_session();
    if !session.connected {
        return Err("AMS2 is not connected".into());
    }
    if session.num_participants == 0 {
        return Err("No active participants".into());
    }
    if !matches!(session.session_state, SESSION_PRACTICE | SESSION_QUALIFY | SESSION_RACE) {
        return Err("No active session".into());
    }
    capture(store, path, &session, vec![]);
    Ok(())
}

pub(crate) fn should_capture(cached: &LiveSessionData) -> bool {
    if cached.num_participants == 0 {
        return false;
    }
    if cached.session_state == SESSION_RACE {
        let max_laps = cached.participants.iter().map(|p| p.laps_completed).max().unwrap_or(0);
        return max_laps > 0;
    }
    true
}

/// Starts a background thread that polls AMS2 shared memory once per second.
///
/// Observed AMS2 state transitions:
///   Practice:            game=4  session=1  race=1
///   Qualifying:          game=4  session=3  race=1
///   Race lobby (grid):   game=4  session=5  race=1
///   Race lights red:     game=2  session=5  race=1
///   Race lights green:   game=2  session=5  race=2
///   Race end:            game=4  session=5  race=5
///
/// Capture triggers:
///   Race   — when the race is finished the user can only leave session which leads to a disconnect (in SP he can also restart the session, meaning he throws away the current cached result).
///   P / Q  — session_state changes (P→Q, Q→Race lobby)
///   Any    — disconnect while session was active
pub fn start(store: SharedStore, path: PathBuf, record_practice: bool, record_qualify: bool, record_race: bool) {
    std::thread::spawn(move || {
        let mut prev_session_state: u32 = 0;
        // Rolling snapshot — updated whenever in a capturable session with participants,
        // regardless of game_state (P/Q use game=4, not game=2).
        let mut session_cache: Option<LiveSessionData> = None;
        // Lap-by-lap position chart accumulated during a race session.
        let mut lap_chart: Vec<LapChartEntry> = vec![];
        let mut leader_laps: u32 = 0;

        loop {
            std::thread::sleep(Duration::from_secs(1));

            let session = read_live_session();

            let should_record = |state: u32| -> bool {
                match state {
                    SESSION_PRACTICE => record_practice,
                    SESSION_QUALIFY  => record_qualify,
                    SESSION_RACE     => record_race,
                    _                => false,
                }
            };

            if !session.connected {
                if let Some(ref cached) = session_cache {
                    if should_capture(cached) && should_record(cached.session_state) {
                        capture(&store, &path, cached, std::mem::take(&mut lap_chart));
                    }
                }
                prev_session_state = 0;
                session_cache = None;
                lap_chart.clear();
                leader_laps = 0;
                continue;
            }

            let session_state = session.session_state;
            if prev_session_state == 0 {
                prev_session_state = session_state;
            }

            if prev_session_state != session_state {
                if let Some(ref cached) = session_cache {
                    if should_capture(cached) && should_record(cached.session_state) {
                        capture(&store, &path, cached, std::mem::take(&mut lap_chart));
                    }
                }
                session_cache = None;
                lap_chart.clear();
                leader_laps = 0;
                prev_session_state = session_state;
            }

            // ── Accumulate per-lap positions during a race ────────────────────
            if session_state == SESSION_RACE && session.num_participants > 0 {
                let max_laps = session.participants.iter().map(|p| p.laps_completed).max().unwrap_or(0);
                if max_laps > leader_laps {
                    // A new lap was completed — snapshot every participant's position.
                    leader_laps = max_laps;
                    for p in &session.participants {
                        lap_chart.push(LapChartEntry {
                            lap: max_laps,
                            driver: p.name.clone(),
                            position: p.race_position,
                        });
                    }
                }
            }

            // ── Always refresh the rolling cache ─────────────────────────────
            // P/Q run at game_state=4 so we must not gate the cache on game_state.
            if matches!(session_state, SESSION_PRACTICE | SESSION_QUALIFY | SESSION_RACE)
                && session.num_participants > 0
            {
                session_cache = Some(session.clone());
            } else if !matches!(session_state, SESSION_PRACTICE | SESSION_QUALIFY | SESSION_RACE) {
                session_cache = None;
            }
        }
    });
}
