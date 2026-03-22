use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::ams2_shared_memory::read_live_session;
use crate::data_store::{persist, RecordedSession, SessionResult, SharedStore};

/// AMS2 session_state value for Race.
const SESSION_RACE: u32 = 5;
/// AMS2 race_state value for actively Racing.
const RACE_RACING: u32 = 2;
/// AMS2 race_state value for Finished.
const RACE_FINISHED: u32 = 3;

/// Starts a background thread that polls AMS2 shared memory once per second.
/// When a race session ends (race_state transitions Racing → Finished), it
/// captures the final standings and persists them to `path`.
pub fn start(store: SharedStore, path: PathBuf) {
    std::thread::spawn(move || {
        let mut prev_race_state: u32 = 0;
        let mut cooldown: u32 = 0;

        loop {
            std::thread::sleep(Duration::from_secs(1));

            if cooldown > 0 {
                cooldown -= 1;
                continue;
            }

            let session = read_live_session();
            if !session.connected {
                prev_race_state = 0;
                continue;
            }

            let race_state = session.race_state;

            // Detect: session is a Race, player just transitioned Racing → Finished.
            if session.session_state == SESSION_RACE
                && prev_race_state == RACE_RACING
                && race_state == RACE_FINISHED
                && session.num_participants > 0
            {
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
                    session_type: session.session_state,
                    results,
                };

                println!(
                    "[recorder] Race at {} recorded — {} participants",
                    recorded.track,
                    recorded.results.len()
                );

                {
                    let mut data = store.write().unwrap();
                    data.sessions.push(recorded);
                }
                persist(&store, &path);
                // 60 s cooldown prevents double-capture at the same session end.
                cooldown = 60;
            }

            prev_race_state = race_state;
        }
    });
}
