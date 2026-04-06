use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns a unique temp path that does not yet exist.
fn tmp() -> PathBuf {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    std::env::temp_dir().join(format!("ams2_test_{ns}.json"))
}

fn sample_championship() -> Championship {
    Championship {
        id: "1".into(),
        name: "Formula Test".into(),
        status: ChampionshipStatus::Active,
        points_system: vec![25, 18, 15, 12, 10],
        manufacturer_scoring: false,
        rounds: vec![Round { session_ids: vec!["100".into()] }],
        session_ids: vec!["100".into()],
    }
}

fn sample_session() -> RecordedSession {
    RecordedSession {
        id: "100".into(),
        recorded_at: 1_700_000_000,
        track: "Silverstone".into(),
        track_variation: "Grand Prix".into(),
        car_name: "Formula Classic Gen2".into(),
        car_class: "Formula Classic".into(),
        session_type: 5,
        results: vec![
            SessionResult {
                name: "Alice".into(),
                car_name: "Formula Classic Gen2".into(),
                car_class: "Formula Classic".into(),
                race_position: 1,
                laps_completed: 20,
                fastest_lap: 89.5,
                last_lap: 90.1,
                dnf: false,
            },
            SessionResult {
                name: "Bob".into(),
                car_name: "Formula Classic Gen2".into(),
                car_class: "Formula Classic".into(),
                race_position: 2,
                laps_completed: 20,
                fastest_lap: 90.0,
                last_lap: 91.0,
                dnf: false,
            },
        ],
    }
}

// ── load_store ────────────────────────────────────────────────────────────────

#[test]
fn test_load_store_nonexistent_file_returns_empty_default() {
    let path = tmp();
    let store = load_store(&path);
    let data = store.read().unwrap();
    assert!(data.sessions.is_empty());
    assert!(data.championships.is_empty());
}

#[test]
fn test_load_store_invalid_json_returns_empty_default() {
    let path = tmp();
    fs::write(&path, "not { valid } json %%%").unwrap();
    let store = load_store(&path);
    let data = store.read().unwrap();
    assert!(data.sessions.is_empty());
    fs::remove_file(&path).ok();
}

#[test]
fn test_load_store_empty_object_returns_empty_default() {
    let path = tmp();
    fs::write(&path, "{}").unwrap();
    let store = load_store(&path);
    let data = store.read().unwrap();
    assert!(data.sessions.is_empty());
    assert!(data.championships.is_empty());
    fs::remove_file(&path).ok();
}

// ── persist ───────────────────────────────────────────────────────────────────

#[test]
fn test_persist_and_reload_championship() {
    let path = tmp();
    let store = load_store(&path);
    store.write().unwrap().championships.push(sample_championship());
    persist(&store, &path);

    let store2 = load_store(&path);
    let data = store2.read().unwrap();
    assert_eq!(data.championships.len(), 1);
    assert_eq!(data.championships[0].name, "Formula Test");
    assert_eq!(data.championships[0].points_system, vec![25, 18, 15, 12, 10]);
    // session_ids is skip_serializing; rounds persist instead
    assert_eq!(data.championships[0].rounds.len(), 1);
    assert_eq!(data.championships[0].rounds[0].session_ids, vec!["100"]);
    fs::remove_file(&path).ok();
}

#[test]
fn test_persist_and_reload_session() {
    let path = tmp();
    let store = load_store(&path);
    store.write().unwrap().sessions.push(sample_session());
    persist(&store, &path);

    let store2 = load_store(&path);
    let data = store2.read().unwrap();
    assert_eq!(data.sessions.len(), 1);
    assert_eq!(data.sessions[0].track, "Silverstone");
    assert_eq!(data.sessions[0].results.len(), 2);
    assert_eq!(data.sessions[0].results[0].name, "Alice");
    assert_eq!(data.sessions[0].results[0].fastest_lap, 89.5);
    assert!(!data.sessions[0].results[0].dnf);
    fs::remove_file(&path).ok();
}

#[test]
fn test_persist_overwrites_previous_contents() {
    let path = tmp();
    let store = load_store(&path);
    store.write().unwrap().championships.push(sample_championship());
    persist(&store, &path);

    // Add a second championship and persist again.
    let mut c2 = sample_championship();
    c2.id = "2".into();
    c2.name = "Second Champ".into();
    store.write().unwrap().championships.push(c2);
    persist(&store, &path);

    let store3 = load_store(&path);
    assert_eq!(store3.read().unwrap().championships.len(), 2);
    fs::remove_file(&path).ok();
}

#[test]
fn test_persist_writes_valid_json() {
    let path = tmp();
    let store = load_store(&path);
    store.write().unwrap().sessions.push(sample_session());
    persist(&store, &path);

    let raw = fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(parsed.get("sessions").is_some());
    assert!(parsed.get("championships").is_some());
    fs::remove_file(&path).ok();
}

#[test]
fn test_dnf_result_round_trips() {
    let path = tmp();
    let store = load_store(&path);
    let mut session = sample_session();
    session.results[1].dnf = true;
    store.write().unwrap().sessions.push(session);
    persist(&store, &path);

    let store2 = load_store(&path);
    let data = store2.read().unwrap();
    assert!(data.sessions[0].results[1].dnf);
    fs::remove_file(&path).ok();
}

#[test]
fn test_load_store_migrates_legacy_session_ids_to_rounds() {
    let path = tmp();
    // Write legacy format: session_ids at championship level, no rounds
    let json = r#"{
        "sessions": [],
        "championships": [{
            "id": "1", "name": "Legacy", "status": "Active",
            "points_system": [25,18], "manufacturer_scoring": false,
            "rounds": [], "session_ids": ["a", "b", "c"]
        }]
    }"#;
    fs::write(&path, json).unwrap();

    let store = load_store(&path);
    let data = store.read().unwrap();
    let champ = &data.championships[0];
    // Each legacy session_id should become its own round
    assert_eq!(champ.rounds.len(), 3);
    assert_eq!(champ.rounds[0].session_ids, vec!["a"]);
    assert_eq!(champ.rounds[1].session_ids, vec!["b"]);
    assert_eq!(champ.rounds[2].session_ids, vec!["c"]);
    fs::remove_file(&path).ok();
}

// ── standings ─────────────────────────────────────────────────────────────────

fn make_champ(pts: Vec<i32>, sessions: &[&str]) -> Championship {
    Championship {
        id: "c1".into(), name: "Test".into(), status: ChampionshipStatus::Active,
        points_system: pts,
        manufacturer_scoring: false,
        rounds: sessions.iter().map(|&id| Round { session_ids: vec![id.into()] }).collect(),
        session_ids: vec![],
    }
}

fn make_session(id: &str, session_type: u32, results: Vec<(&str, u32, bool, &str)>) -> RecordedSession {
    RecordedSession {
        id: id.into(), recorded_at: 0,
        track: "Test Track".into(), track_variation: "".into(),
        car_name: "".into(), car_class: "".into(),
        session_type,
        results: results.into_iter().map(|(name, pos, dnf, car)| SessionResult {
            name: name.into(), car_name: car.into(), car_class: "".into(),
            race_position: pos, laps_completed: 10, fastest_lap: 0.0, last_lap: 0.0, dnf,
        }).collect(),
    }
}

#[test]
fn test_standings_basic_points() {
    let champ = make_champ(vec![25, 18, 15], &["s1"]);
    let sessions = vec![make_session("s1", 5, vec![
        ("Alice", 1, false, ""),
        ("Bob",   2, false, ""),
        ("Carol", 3, false, ""),
    ])];
    let st = standings(&champ, &sessions);
    assert_eq!(st[0].name, "Alice"); assert_eq!(st[0].points, 25); assert_eq!(st[0].wins, 1);
    assert_eq!(st[1].name, "Bob");   assert_eq!(st[1].points, 18); assert_eq!(st[1].wins, 0);
    assert_eq!(st[2].name, "Carol"); assert_eq!(st[2].points, 15);
}

#[test]
fn test_standings_dnf_gets_no_points_and_no_win() {
    let champ = make_champ(vec![25, 18], &["s1"]);
    let sessions = vec![make_session("s1", 5, vec![
        ("Alice", 1, true,  ""), // DNF even in P1
        ("Bob",   2, false, ""),
    ])];
    let st = standings(&champ, &sessions);
    let alice = st.iter().find(|e| e.name == "Alice").unwrap();
    assert_eq!(alice.points, 0);
    assert_eq!(alice.wins, 0);
    let bob = st.iter().find(|e| e.name == "Bob").unwrap();
    assert_eq!(bob.points, 18);
}

#[test]
fn test_standings_ignores_practice_and_qualify() {
    let champ = make_champ(vec![25, 18], &["p1", "q1", "r1"]);
    let sessions = vec![
        make_session("p1", 1, vec![("Alice", 1, false, "")]),
        make_session("q1", 3, vec![("Alice", 1, false, "")]),
        make_session("r1", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
    ];
    let st = standings(&champ, &sessions);
    // Points only from the race session
    let alice = st.iter().find(|e| e.name == "Alice").unwrap();
    assert_eq!(alice.points, 25);
    assert_eq!(alice.wins, 1);
}

#[test]
fn test_standings_multiple_rounds_accumulate() {
    let champ = make_champ(vec![25, 18], &["r1", "r2"]);
    let sessions = vec![
        make_session("r1", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
        make_session("r2", 5, vec![("Bob", 1, false, ""), ("Alice", 2, false, "")]),
    ];
    let st = standings(&champ, &sessions);
    let alice = st.iter().find(|e| e.name == "Alice").unwrap();
    assert_eq!(alice.points, 25 + 18); // won r1, 2nd in r2
    let bob = st.iter().find(|e| e.name == "Bob").unwrap();
    assert_eq!(bob.points, 18 + 25);   // 2nd in r1, won r2
}

#[test]
fn test_standings_sorted_by_points_then_wins() {
    let champ = make_champ(vec![10, 10], &["r1", "r2"]);
    // Alice and Bob both get 20 pts, but Alice has 2 wins vs Bob's 0
    let sessions = vec![
        make_session("r1", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
        make_session("r2", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
    ];
    let st = standings(&champ, &sessions);
    assert_eq!(st[0].name, "Alice");
    assert_eq!(st[0].wins, 2);
    assert_eq!(st[1].name, "Bob");
}

#[test]
fn test_standings_position_beyond_points_system_gets_zero() {
    let champ = make_champ(vec![25, 18], &["r1"]);
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, false, ""),
        ("Bob",   2, false, ""),
        ("Carol", 3, false, ""), // P3 but only 2 points defined
    ])];
    let st = standings(&champ, &sessions);
    let carol = st.iter().find(|e| e.name == "Carol").unwrap();
    assert_eq!(carol.points, 0);
}

// ── constructors ──────────────────────────────────────────────────────────────

#[test]
fn test_constructors_groups_by_car_name() {
    let champ = make_champ(vec![25, 18, 15], &["r1"]);
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, false, "Ferrari"),
        ("Bob",   2, false, "Ferrari"),
        ("Carol", 3, false, "McLaren"),
    ])];
    let ct = constructors(&champ, &sessions);
    let ferrari = ct.iter().find(|e| e.name == "Ferrari").unwrap();
    assert_eq!(ferrari.points, 25 + 18); // Alice + Bob
    let mclaren = ct.iter().find(|e| e.name == "McLaren").unwrap();
    assert_eq!(mclaren.points, 15);
}

#[test]
fn test_constructors_dnf_excluded_from_points() {
    let champ = make_champ(vec![25, 18], &["r1"]);
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, true,  "Ferrari"), // DNF
        ("Bob",   2, false, "McLaren"),
    ])];
    let ct = constructors(&champ, &sessions);
    let ferrari = ct.iter().find(|e| e.name == "Ferrari").unwrap();
    assert_eq!(ferrari.points, 0);
}

#[test]
fn test_constructors_empty_car_name_uses_car_class() {
    let mut champ = make_champ(vec![25], &["r1"]);
    champ.manufacturer_scoring = true;
    let mut sess = make_session("r1", 5, vec![("Alice", 1, false, "")]);
    sess.results[0].car_class = "GT3".into();
    let ct = constructors(&champ, &[sess]);
    assert!(ct.iter().any(|e| e.name == "GT3"));
}

#[test]
fn test_constructors_no_car_info_excluded() {
    let champ = make_champ(vec![25], &["r1"]);
    // car_name and car_class both empty — should not appear in constructors
    let sessions = vec![make_session("r1", 5, vec![("Alice", 1, false, "")])];
    let ct = constructors(&champ, &sessions);
    assert!(ct.is_empty());
}

// ── compute_career ────────────────────────────────────────────────────────────

#[test]
fn test_compute_career_race_stats_accumulated() {
    let champ = make_champ(vec![25, 18, 15], &["r1"]);
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, false, ""),
        ("Bob",   2, false, ""),
        ("Carol", 3, false, ""),
    ])];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    assert_eq!(alice.races, 1);
    assert_eq!(alice.p1, 1);
    assert_eq!(alice.p2, 0);
    assert_eq!(alice.p3, 0);
    assert_eq!(alice.top10, 1);
    assert_eq!(alice.dnf, 0);
    let bob = resp.driver_stats.iter().find(|d| d.name == "Bob").unwrap();
    assert_eq!(bob.p1, 0);
    assert_eq!(bob.p2, 1);
    assert_eq!(bob.p3, 0);
}

#[test]
fn test_compute_career_dnf_not_counted_in_wins_or_top3() {
    let champ = make_champ(vec![25], &["r1"]);
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, true,  ""), // DNF
        ("Bob",   2, false, ""),
    ])];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    assert_eq!(alice.races, 1);
    assert_eq!(alice.dnf, 1);
    assert_eq!(alice.p1, 0);
    assert_eq!(alice.p2, 0);
    assert_eq!(alice.p3, 0);
    assert_eq!(alice.top10, 0);
}

#[test]
fn test_compute_career_champ_wins_only_for_finished() {
    let mut active = make_champ(vec![25, 18], &["r1"]);
    active.status = ChampionshipStatus::Active;
    let mut finished = make_champ(vec![25, 18], &["r2"]);
    finished.id = "c2".into();
    finished.status = ChampionshipStatus::Final;
    let sessions = vec![
        make_session("r1", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
        make_session("r2", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
    ];
    let resp = compute_career(&[active, finished], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    assert_eq!(alice.champ_wins, 1); // only the Final one counts
}

#[test]
fn test_compute_career_avg_pos() {
    let champ = make_champ(vec![25, 18], &["r1", "r2"]);
    let sessions = vec![
        make_session("r1", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
        make_session("r2", 5, vec![("Alice", 3, false, ""), ("Bob", 1, false, "")]),
    ];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    // (1 + 3) / 2 = 2.0
    assert!((alice.avg_pos - 2.0).abs() < f32::EPSILON);
}

#[test]
fn test_compute_career_driver_stats_sorted_by_wins_then_races() {
    let champ = make_champ(vec![25, 18], &["r1", "r2"]);
    let sessions = vec![
        make_session("r1", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
        make_session("r2", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
    ];
    let resp = compute_career(&[champ], &sessions);
    // Alice has 2 wins, Bob has 0 — Alice should be first
    assert_eq!(resp.driver_stats[0].name, "Alice");
}

#[test]
fn test_compute_career_practice_and_qualify_not_counted() {
    let champ = make_champ(vec![25], &["p1", "q1", "r1"]);
    let sessions = vec![
        make_session("p1", 1, vec![("Alice", 1, false, "")]),
        make_session("q1", 3, vec![("Alice", 1, false, "")]),
        make_session("r1", 5, vec![("Alice", 1, false, ""), ("Bob", 2, false, "")]),
    ];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    assert_eq!(alice.races, 1); // only the race session counted
}

#[test]
fn test_compute_career_sessions_resolved_into_rounds() {
    let champ = make_champ(vec![25], &["r1"]);
    let sessions = vec![make_session("r1", 5, vec![("Alice", 1, false, "")])];
    let resp = compute_career(&[champ], &sessions);
    assert_eq!(resp.championships[0].rounds.len(), 1);
    assert_eq!(resp.championships[0].rounds[0].sessions.len(), 1);
    assert_eq!(resp.championships[0].rounds[0].sessions[0].id, "r1");
}

// ── points_earned in SessionResultView ───────────────────────────────────────

#[test]
fn test_compute_career_points_earned_in_result_view() {
    let champ = make_champ(vec![25, 18, 15], &["r1"]);
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, false, ""),
        ("Bob",   2, false, ""),
        ("Carol", 3, false, ""),
    ])];
    let resp = compute_career(&[champ], &sessions);
    let race = &resp.championships[0].rounds[0].sessions[0];
    let alice = race.results.iter().find(|r| r.name == "Alice").unwrap();
    let bob   = race.results.iter().find(|r| r.name == "Bob").unwrap();
    let carol = race.results.iter().find(|r| r.name == "Carol").unwrap();
    assert_eq!(alice.points_earned, 25);
    assert_eq!(bob.points_earned, 18);
    assert_eq!(carol.points_earned, 15);
}

#[test]
fn test_compute_career_dnf_earns_no_points_in_view() {
    let champ = make_champ(vec![25, 18], &["r1"]);
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, true,  ""), // DNF
        ("Bob",   2, false, ""),
    ])];
    let resp = compute_career(&[champ], &sessions);
    let race = &resp.championships[0].rounds[0].sessions[0];
    let alice = race.results.iter().find(|r| r.name == "Alice").unwrap();
    assert_eq!(alice.points_earned, 0);
}

#[test]
fn test_compute_career_position_beyond_points_earns_zero_in_view() {
    let champ = make_champ(vec![25, 18], &["r1"]);
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, false, ""),
        ("Bob",   2, false, ""),
        ("Carol", 3, false, ""), // P3 but only 2 positions in points system
    ])];
    let resp = compute_career(&[champ], &sessions);
    let race = &resp.championships[0].rounds[0].sessions[0];
    let carol = race.results.iter().find(|r| r.name == "Carol").unwrap();
    assert_eq!(carol.points_earned, 0);
}

// ── qualifying position stats ─────────────────────────────────────────────────

#[test]
fn test_compute_career_quali_podium_positions() {
    let champ = make_champ(vec![25], &["q1", "r1"]);
    let sessions = vec![
        make_session("q1", 3, vec![
            ("Alice", 1, false, ""),
            ("Bob",   2, false, ""),
            ("Carol", 3, false, ""),
        ]),
        make_session("r1", 5, vec![
            ("Alice", 1, false, ""),
            ("Bob",   2, false, ""),
            ("Carol", 3, false, ""),
        ]),
    ];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    let bob   = resp.driver_stats.iter().find(|d| d.name == "Bob").unwrap();
    let carol = resp.driver_stats.iter().find(|d| d.name == "Carol").unwrap();
    assert_eq!(alice.quali_p1, 1); assert_eq!(alice.quali_p2, 0); assert_eq!(alice.quali_p3, 0);
    assert_eq!(bob.quali_p1,   0); assert_eq!(bob.quali_p2,   1); assert_eq!(bob.quali_p3,   0);
    assert_eq!(carol.quali_p1, 0); assert_eq!(carol.quali_p2, 0); assert_eq!(carol.quali_p3, 1);
}

#[test]
fn test_compute_career_quali_top10_boundary() {
    let champ = make_champ(vec![25], &["q1"]);
    let sessions = vec![make_session("q1", 3, vec![
        ("Alice",  1, false, ""),
        ("Bob",   10, false, ""),
        ("Carol", 11, false, ""), // just outside top 10
    ])];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    let bob   = resp.driver_stats.iter().find(|d| d.name == "Bob").unwrap();
    let carol = resp.driver_stats.iter().find(|d| d.name == "Carol").unwrap();
    assert_eq!(alice.quali_top10, 1);
    assert_eq!(bob.quali_top10,   1);
    assert_eq!(carol.quali_top10, 0);
}

#[test]
fn test_compute_career_quali_not_counted_as_race() {
    let champ = make_champ(vec![25], &["q1"]);
    let sessions = vec![make_session("q1", 3, vec![("Alice", 1, false, "")])];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    assert_eq!(alice.races, 0);
    assert_eq!(alice.p1,    0);
    assert_eq!(alice.quali_p1, 1);
}

// ── champ_p2 / champ_p3 ──────────────────────────────────────────────────────

#[test]
fn test_compute_career_champ_p2_p3_for_final_championship() {
    let mut champ = make_champ(vec![25, 18, 15], &["r1"]);
    champ.status = ChampionshipStatus::Final;
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, false, ""),
        ("Bob",   2, false, ""),
        ("Carol", 3, false, ""),
    ])];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    let bob   = resp.driver_stats.iter().find(|d| d.name == "Bob").unwrap();
    let carol = resp.driver_stats.iter().find(|d| d.name == "Carol").unwrap();
    assert_eq!(alice.champ_wins, 1); assert_eq!(alice.champ_p2, 0); assert_eq!(alice.champ_p3, 0);
    assert_eq!(bob.champ_wins,   0); assert_eq!(bob.champ_p2,   1); assert_eq!(bob.champ_p3,   0);
    assert_eq!(carol.champ_wins, 0); assert_eq!(carol.champ_p2, 0); assert_eq!(carol.champ_p3, 1);
}

#[test]
fn test_compute_career_champ_standings_not_counted_for_active() {
    let champ = make_champ(vec![25, 18, 15], &["r1"]);
    // status defaults to Active
    let sessions = vec![make_session("r1", 5, vec![
        ("Alice", 1, false, ""),
        ("Bob",   2, false, ""),
        ("Carol", 3, false, ""),
    ])];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp.driver_stats.iter().find(|d| d.name == "Alice").unwrap();
    let bob   = resp.driver_stats.iter().find(|d| d.name == "Bob").unwrap();
    let carol = resp.driver_stats.iter().find(|d| d.name == "Carol").unwrap();
    assert_eq!(alice.champ_wins, 0);
    assert_eq!(bob.champ_p2,     0);
    assert_eq!(carol.champ_p3,   0);
}

// ── track_stats ───────────────────────────────────────────────────────────────

/// Helper: a race session at a specific track/timestamp with (name, pos, fastest_lap, car_name).
fn make_track_session(id: &str, session_type: u32, track: &str, variation: &str, recorded_at: u64,
                      results: Vec<(&str, u32, f32, &str)>) -> RecordedSession {
    RecordedSession {
        id: id.into(), recorded_at,
        track: track.into(), track_variation: variation.into(),
        car_name: "".into(), car_class: "".into(),
        session_type,
        results: results.into_iter().map(|(name, pos, fl, car)| SessionResult {
            name: name.into(), car_name: car.into(), car_class: "".into(),
            race_position: pos, laps_completed: 10, fastest_lap: fl, last_lap: 0.0, dnf: false,
        }).collect(),
    }
}

#[test]
fn test_track_stats_race_and_qualifying_counts() {
    let sessions = vec![
        make_track_session("r1", 5, "Silverstone", "GP", 1000, vec![("Alice", 1, 90.0, "Car")]),
        make_track_session("q1", 3, "Silverstone", "GP",  900, vec![("Alice", 1, 89.5, "Car")]),
    ];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats.len(), 1);
    let ts = &resp.track_stats[0];
    assert_eq!(ts.track, "Silverstone");
    assert_eq!(ts.races, 1);
    assert_eq!(ts.qualifyings, 1);
}

#[test]
fn test_track_stats_best_lap_driver_and_car() {
    let sessions = vec![make_track_session("r1", 5, "Spa", "GP", 1000, vec![
        ("Alice", 1, 120.0, "Ferrari"),
        ("Bob",   2, 118.5, "McLaren"), // Bob sets the fastest lap
    ])];
    let resp = compute_career(&[], &sessions);
    let ts = &resp.track_stats[0];
    assert!((ts.best_lap - 118.5).abs() < 0.001);
    assert_eq!(ts.best_lap_driver, "Bob");
    assert_eq!(ts.best_lap_car,    "McLaren");
}

#[test]
fn test_track_stats_best_lap_car_class_fallback() {
    let mut sess = make_track_session("r1", 5, "Monza", "GP", 1000,
                                      vec![("Alice", 1, 90.0, "")]);
    sess.results[0].car_class = "GT3".into();
    let resp = compute_career(&[], &[sess]);
    assert_eq!(resp.track_stats[0].best_lap_car, "GT3");
}

#[test]
fn test_track_stats_best_lap_updated_across_sessions() {
    let sessions = vec![
        make_track_session("r1", 5, "Spa", "GP", 1000, vec![("Alice", 1, 120.0, "Ferrari")]),
        make_track_session("r2", 5, "Spa", "GP", 2000, vec![("Bob",   1, 118.0, "McLaren")]),
    ];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats.len(), 1);
    let ts = &resp.track_stats[0];
    assert_eq!(ts.races, 2);
    assert!((ts.best_lap - 118.0).abs() < 0.001);
    assert_eq!(ts.best_lap_driver, "Bob");
    assert_eq!(ts.best_lap_car,    "McLaren");
}

#[test]
fn test_track_stats_track_variation_is_separate_key() {
    let sessions = vec![
        make_track_session("r1", 5, "Silverstone", "GP",       1000, vec![("Alice", 1, 90.0, "")]),
        make_track_session("r2", 5, "Silverstone", "National", 2000, vec![("Alice", 1, 70.0, "")]),
    ];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats.len(), 2);
}

#[test]
fn test_track_stats_sorted_by_last_visited_desc() {
    let sessions = vec![
        make_track_session("r1", 5, "Silverstone", "GP", 1000, vec![("Alice", 1, 90.0, "")]),
        make_track_session("r2", 5, "Monza",       "GP", 2000, vec![("Alice", 1, 85.0, "")]),
        make_track_session("r3", 5, "Spa",         "GP",  500, vec![("Alice", 1, 95.0, "")]),
    ];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats[0].track, "Monza");
    assert_eq!(resp.track_stats[1].track, "Silverstone");
    assert_eq!(resp.track_stats[2].track, "Spa");
}

#[test]
fn test_track_stats_last_visited_is_most_recent_session() {
    let sessions = vec![
        make_track_session("r1", 5, "Spa", "GP", 1000, vec![("Alice", 1, 90.0, "")]),
        make_track_session("r2", 5, "Spa", "GP", 3000, vec![("Bob",   1, 95.0, "")]),
        make_track_session("r3", 5, "Spa", "GP", 2000, vec![("Carol", 1, 88.0, "")]),
    ];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats[0].last_visited, 3000);
}

#[test]
fn test_track_stats_practice_not_counted_as_race_or_qualifying() {
    let sessions = vec![
        make_track_session("p1", 1, "Spa", "GP", 1000, vec![("Alice", 1, 90.0, "")]),
    ];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats.len(), 1);
    let ts = &resp.track_stats[0];
    assert_eq!(ts.races,      0);
    assert_eq!(ts.qualifyings, 0);
}
