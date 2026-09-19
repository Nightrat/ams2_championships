use super::*;
use crate::ams2_shared_memory::{LiveSessionData, ParticipantData, PlayerTelemetry};
use crate::data_store::{CareerData, SharedStore};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

// ── helpers ───────────────────────────────────────────────────────────────────

fn empty_telemetry() -> PlayerTelemetry {
    PlayerTelemetry {
        tyre_temp_left: [0.0; 4],
        tyre_temp_center: [0.0; 4],
        tyre_temp_right: [0.0; 4],
        tyre_wear: [0.0; 4],
        tyre_flags: [0; 4],
        terrain: [0; 4],
        tyre_y: [0.0; 4],
        tyre_rps: [0.0; 4],
        tyre_temp: [0.0; 4],
        tyre_height_above_ground: [0.0; 4],
        tyre_tread_temp: [0.0; 4],
        tyre_layer_temp: [0.0; 4],
        tyre_carcass_temp: [0.0; 4],
        tyre_rim_temp: [0.0; 4],
        tyre_internal_air_temp: [0.0; 4],
        tyre_slip_speed: [0.0; 4],
        tyre_grip: [0.0; 4],
        tyre_lateral_stiffness: [0.0; 4],
        suspension_velocity: [0.0; 4],
        wheel_local_position_y: [0.0; 4],
        tyre_pressure: [0.0; 4],
        brake_temp: [0.0; 4],
        suspension_travel: [0.0; 4],
        ride_height: [0.0; 4],
        throttle: 0.0,
        brake_input: 0.0,
        steering: 0.0,
        speed: 0.0,
        rpm: 0.0,
        gear: 0,
        tyre_compound: [String::new(), String::new(), String::new(), String::new()],
        fuel_level: 0.0,
        fuel_capacity: 0.0,
        crash_state: 0,
        aero_damage: 0.0,
        engine_damage: 0.0,
        brake_damage: [0.0; 4],
        suspension_damage: [0.0; 4],
        last_collision_index: -1,
        last_collision_magnitude: 0.0,
    }
}

fn make_participant(name: &str, pos: u32, laps: u32, fl: f32, car: &str) -> ParticipantData {
    ParticipantData {
        name: name.into(),
        car_name: car.into(),
        car_class: String::new(),
        is_active: true,
        is_player: false,
        race_position: pos,
        laps_completed: laps,
        current_lap: laps + 1,
        current_lap_distance: 0.0,
        cur_s1: -1.0,
        cur_s2: -1.0,
        cur_s3: -1.0,
        best_s1: -1.0,
        best_s2: -1.0,
        best_s3: -1.0,
        fastest_lap_time: fl,
        last_lap_time: 0.0,
        world_pos_x: 0.0,
        world_pos_z: 0.0,
        interval_gap_secs: 0.0,
        interval_gap_laps: 0,
        in_pits: false,
    }
}

fn make_session(session_state: u32, participants: Vec<ParticipantData>) -> LiveSessionData {
    let n = participants.len() as i32;
    LiveSessionData {
        connected: true,
        game_state: 2,
        session_state,
        race_state: 2,
        num_participants: n,
        track_location: "Spa".into(),
        track_variation: "GP".into(),
        track_length: 7000.0,
        laps_in_event: 0,
        car_name: "Ferrari".into(),
        car_class: "GT3".into(),
        participants,
        player_telemetry: empty_telemetry(),
        race_flag_colour: 0,
        race_flag_reason: 0,
        pit_mode: 0,
    }
}

// ── lap chart accumulation ────────────────────────────────────────────────────

/// Drives the accumulator through a race where the leader reaches `laps`.
fn run_laps(chart: &mut Vec<LapChartEntry>, leader: &mut u32, laps: u32) {
    for lap in 1..=laps {
        let s = make_session(
            5,
            vec![
                make_participant("A", 1, lap, 90.0, "Ferrari"),
                make_participant("B", 2, lap.saturating_sub(1), 91.0, "Ferrari"),
            ],
        );
        accumulate_lap_chart(chart, leader, &s);
    }
}

#[test]
fn test_lap_chart_records_every_lap() {
    let (mut chart, mut leader) = (vec![], 0);
    run_laps(&mut chart, &mut leader, 5);
    let laps: std::collections::BTreeSet<u32> = chart.iter().map(|e| e.lap).collect();
    assert_eq!(laps.into_iter().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
    assert_eq!(chart.len(), 10, "two drivers snapshotted per lap");
}

#[test]
fn test_lap_chart_ignores_repeated_polls_within_a_lap() {
    let (mut chart, mut leader) = (vec![], 0);
    let s = make_session(5, vec![make_participant("A", 1, 3, 90.0, "Ferrari")]);
    accumulate_lap_chart(&mut chart, &mut leader, &s);
    accumulate_lap_chart(&mut chart, &mut leader, &s);
    accumulate_lap_chart(&mut chart, &mut leader, &s);
    assert_eq!(chart.len(), 1);
}

#[test]
fn test_lap_chart_restarts_when_the_race_is_restarted() {
    // Run to lap 5, then restart and run to lap 3. AMS2 keeps session_state at RACE across a
    // restart, so the falling lap count is the only evidence the earlier run was abandoned.
    let (mut chart, mut leader) = (vec![], 0);
    run_laps(&mut chart, &mut leader, 5);
    run_laps(&mut chart, &mut leader, 3);
    let laps: std::collections::BTreeSet<u32> = chart.iter().map(|e| e.lap).collect();
    assert_eq!(
        laps.into_iter().collect::<Vec<_>>(),
        vec![1, 2, 3],
        "the abandoned run must be discarded, not merged"
    );
    assert_eq!(leader, 3);
}

#[test]
fn test_lap_chart_restart_does_not_leave_a_single_final_lap() {
    // The failure this guards: a stale high-water mark of 38 means a fresh 39-lap race records
    // only lap 39 — one column per driver, which is what a broken chart looked like.
    let (mut chart, mut leader) = (vec![], 38);
    run_laps(&mut chart, &mut leader, 39);
    let laps: std::collections::BTreeSet<u32> = chart.iter().map(|e| e.lap).collect();
    assert_eq!(laps.len(), 39, "got {:?}", laps);
}

#[test]
fn test_lap_chart_ignores_empty_grids() {
    let (mut chart, mut leader) = (vec![], 0);
    accumulate_lap_chart(&mut chart, &mut leader, &make_session(5, vec![]));
    assert!(chart.is_empty());
    assert_eq!(leader, 0);
}

fn make_store() -> (SharedStore, PathBuf) {
    let store = Arc::new(RwLock::new(CareerData::default()));
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ams2_rec_test_{ns}.json"));
    (store, path)
}

// ── capture() ─────────────────────────────────────────────────────────────────

#[test]
fn test_capture_stores_session_with_correct_fields() {
    let (store, path) = make_store();
    let session = make_session(
        5,
        vec![
            make_participant("Alice", 1, 10, 90.0, "Ferrari"),
            make_participant("Bob", 2, 10, 91.0, "McLaren"),
        ],
    );
    capture(&store, &path, &session, vec![], None);
    let data = store.read().unwrap();
    assert_eq!(data.sessions.len(), 1);
    let s = &data.sessions[0];
    assert_eq!(s.track, "Spa");
    assert_eq!(s.track_variation, "GP");
    assert_eq!(s.session_type, 5);
    assert_eq!(s.car_name, "Ferrari");
    assert_eq!(s.car_class, "GT3");
    assert_eq!(s.results.len(), 2);
    assert_eq!(s.results[0].name, "Alice");
    assert_eq!(s.results[1].name, "Bob");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_capture_dnf_driver_with_fewer_laps() {
    let (store, path) = make_store();
    // Alice completed 10 (max); Bob only 8 → Bob is DNF
    let session = make_session(
        5,
        vec![
            make_participant("Alice", 1, 10, 90.0, ""),
            make_participant("Bob", 2, 8, 91.0, ""),
        ],
    );
    capture(&store, &path, &session, vec![], None);
    let data = store.read().unwrap();
    let results = &data.sessions[0].results;
    assert!(!results[0].dnf, "Alice finished");
    assert!(results[1].dnf, "Bob is DNF");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_capture_all_same_laps_no_dnf() {
    let (store, path) = make_store();
    let session = make_session(
        5,
        vec![
            make_participant("Alice", 1, 5, 90.0, ""),
            make_participant("Bob", 2, 5, 91.0, ""),
        ],
    );
    capture(&store, &path, &session, vec![], None);
    let data = store.read().unwrap();
    for r in &data.sessions[0].results {
        assert!(!r.dnf, "{} should not be DNF", r.name);
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_capture_zero_laps_nobody_is_dnf() {
    // max_laps == 0 → guard prevents DNF marking
    let (store, path) = make_store();
    let session = make_session(5, vec![make_participant("Alice", 1, 0, 0.0, "")]);
    capture(&store, &path, &session, vec![], None);
    let data = store.read().unwrap();
    assert!(!data.sessions[0].results[0].dnf);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_capture_maps_session_type_practice() {
    let (store, path) = make_store();
    let session = make_session(1, vec![make_participant("Alice", 1, 3, 90.0, "")]);
    capture(&store, &path, &session, vec![], None);
    assert_eq!(store.read().unwrap().sessions[0].session_type, 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_capture_maps_session_type_qualify() {
    let (store, path) = make_store();
    let session = make_session(3, vec![make_participant("Alice", 1, 3, 90.0, "")]);
    capture(&store, &path, &session, vec![], None);
    assert_eq!(store.read().unwrap().sessions[0].session_type, 3);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_capture_is_player_from_telemetry_flag() {
    let (store, path) = make_store();
    let mut alice = make_participant("Alice", 1, 10, 90.0, "Ferrari");
    alice.is_player = true;
    let session = make_session(
        5,
        vec![alice, make_participant("Bob", 2, 10, 91.0, "McLaren")],
    );
    capture(&store, &path, &session, vec![], None);
    let data = store.read().unwrap();
    assert!(data.sessions[0].results[0].is_player, "Alice");
    assert!(!data.sessions[0].results[1].is_player, "Bob");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_capture_is_player_falls_back_to_player_name_override() {
    // The terminal snapshot (e.g. taken during the post-race results screen) may have lost
    // AMS2's is_player flag for everyone — the tracked player_name from an earlier poll
    // should still mark the right result.
    let (store, path) = make_store();
    let session = make_session(
        5,
        vec![
            make_participant("Alice", 1, 10, 90.0, "Ferrari"),
            make_participant("Bob", 2, 10, 91.0, "McLaren"),
        ],
    );
    capture(&store, &path, &session, vec![], Some("Bob"));
    let data = store.read().unwrap();
    assert!(!data.sessions[0].results[0].is_player, "Alice");
    assert!(data.sessions[0].results[1].is_player, "Bob");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_capture_persists_to_file() {
    let (store, path) = make_store();
    let session = make_session(5, vec![make_participant("Alice", 1, 10, 90.0, "")]);
    capture(&store, &path, &session, vec![], None);
    assert!(path.exists(), "capture should persist data to disk");
    let content = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(v["sessions"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_file(&path);
}

// ── should_capture() ──────────────────────────────────────────────────────────

#[test]
fn test_should_capture_race_with_completed_laps() {
    let session = make_session(5, vec![make_participant("Alice", 1, 5, 90.0, "")]);
    assert!(should_capture(&session));
}

#[test]
fn test_should_capture_race_zero_laps_returns_false() {
    let session = make_session(5, vec![make_participant("Alice", 1, 0, 0.0, "")]);
    assert!(!should_capture(&session));
}

#[test]
fn test_should_capture_qualify_with_zero_laps_returns_true() {
    // P/Q are not gated on laps — any participants present is enough
    let session = make_session(3, vec![make_participant("Alice", 1, 0, 0.0, "")]);
    assert!(should_capture(&session));
}

#[test]
fn test_should_capture_practice_with_participants_returns_true() {
    let session = make_session(1, vec![make_participant("Alice", 1, 2, 88.0, "")]);
    assert!(should_capture(&session));
}

#[test]
fn test_should_capture_no_participants_returns_false() {
    let mut s = make_session(5, vec![]);
    s.num_participants = 0;
    assert!(!should_capture(&s));
}

// ── replays ───────────────────────────────────────────────────────────────────

/// The same session as `make_session`, but as AMS2 reports it while a replay of it plays:
/// the rows are refilled with an earlier moment of the race.
fn as_replay(mut s: LiveSessionData, game_state: u32) -> LiveSessionData {
    s.game_state = game_state;
    s
}

fn finished_race() -> LiveSessionData {
    make_session(
        5,
        vec![
            make_participant("Alice", 1, 30, 90.0, "Ferrari"),
            make_participant("Bob", 2, 30, 91.0, "McLaren"),
        ],
    )
}

/// Mid-race, as a replay plays it back: Bob still leads and only 20 laps are done.
fn rewound_race() -> LiveSessionData {
    make_session(
        5,
        vec![
            make_participant("Alice", 2, 20, 90.0, "Ferrari"),
            make_participant("Bob", 1, 20, 91.0, "McLaren"),
        ],
    )
}

#[test]
fn test_replay_game_states_are_recognised() {
    assert!(is_replay(6), "in-game replay");
    assert!(is_replay(7), "front-end replay");
    assert!(!is_replay(2), "driving");
    assert!(!is_replay(4), "garage / results screen");
}

#[test]
fn test_a_replay_watched_after_the_race_does_not_change_the_result() {
    // Race, watch the replay to lap 20, quit. The recorded result must be the race's, not
    // the frame the replay happened to be showing when AMS2 went away.
    let mut state = RecorderState::new(true, true, true);
    state.poll(&finished_race());
    for gs in [6, 6, 7] {
        assert!(
            state.poll(&as_replay(rewound_race(), gs)).is_none(),
            "a replay poll must never capture"
        );
    }
    let mut gone = finished_race();
    gone.connected = false;
    let taken = state.poll(&gone).expect("the race is captured on disconnect");
    assert_eq!(taken.session.participants[0].race_position, 1, "Alice won");
    assert_eq!(taken.session.participants[0].laps_completed, 30);
}

#[test]
fn test_a_replay_does_not_rewind_the_lap_chart() {
    let mut state = RecorderState::new(true, true, true);
    for lap in 1..=5 {
        state.poll(&make_session(
            5,
            vec![make_participant("Alice", 1, lap, 90.0, "Ferrari")],
        ));
    }
    // A replay stepping back through laps 1-3 would otherwise look like a restart to
    // `accumulate_lap_chart`, which clears the chart on a falling lap count.
    for lap in 1..=3 {
        state.poll(&as_replay(
            make_session(5, vec![make_participant("Alice", 1, lap, 90.0, "Ferrari")]),
            6,
        ));
    }
    let mut gone = make_session(5, vec![make_participant("Alice", 1, 5, 90.0, "Ferrari")]);
    gone.connected = false;
    let taken = state.poll(&gone).expect("captured");
    let laps: std::collections::BTreeSet<u32> = taken.lap_chart.iter().map(|e| e.lap).collect();
    assert_eq!(laps.into_iter().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_a_session_change_during_a_replay_is_acted_on_when_it_ends() {
    // Watching a quali replay from the race lobby: the session state has already moved on,
    // but nothing may be captured until the game is live again — and then it is the quali
    // session that is written, not the replay's rewound rows.
    let mut state = RecorderState::new(true, true, true);
    state.poll(&make_session(
        3,
        vec![make_participant("Alice", 1, 4, 88.0, "Ferrari")],
    ));
    assert!(state
        .poll(&as_replay(
            make_session(5, vec![make_participant("Alice", 6, 1, 95.0, "Ferrari")]),
            6
        ))
        .is_none());
    let taken = state
        .poll(&make_session(
            5,
            vec![make_participant("Alice", 6, 0, 0.0, "Ferrari")],
        ))
        .expect("the qualifying session is captured on the change to race");
    assert_eq!(taken.session.session_state, 3);
    assert_eq!(taken.session.participants[0].fastest_lap_time, 88.0);
}

#[test]
fn test_live_polls_still_capture_on_a_session_change() {
    // The plain path, so the replay hold cannot be mistaken for it: P → Q captures practice.
    let mut state = RecorderState::new(true, true, true);
    state.poll(&make_session(
        1,
        vec![make_participant("Alice", 1, 3, 92.0, "Ferrari")],
    ));
    let taken = state
        .poll(&make_session(
            3,
            vec![make_participant("Alice", 1, 0, 0.0, "Ferrari")],
        ))
        .expect("practice is captured");
    assert_eq!(taken.session.session_state, 1);
}

#[test]
fn test_a_session_type_that_is_not_recorded_is_not_captured() {
    let mut state = RecorderState::new(false, true, true);
    state.poll(&make_session(
        1,
        vec![make_participant("Alice", 1, 3, 92.0, "Ferrari")],
    ));
    assert!(
        state
            .poll(&make_session(
                3,
                vec![make_participant("Alice", 1, 0, 0.0, "Ferrari")]
            ))
            .is_none(),
        "practice recording is off"
    );
}
