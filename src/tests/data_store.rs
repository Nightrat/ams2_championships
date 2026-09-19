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
        rounds: vec![Round {
            session_ids: vec!["100".into()],
        }],
        session_ids: vec!["100".into()],
        custom_ai_file: None,
        player_team: None,
        planned_rounds: None,
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
                is_player: false,
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
                is_player: false,
            },
        ],
        lap_chart: vec![],
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
    store
        .write()
        .unwrap()
        .championships
        .push(sample_championship());
    persist(&store, &path).expect("the save must actually be written");

    let store2 = load_store(&path);
    let data = store2.read().unwrap();
    assert_eq!(data.championships.len(), 1);
    assert_eq!(data.championships[0].name, "Formula Test");
    assert_eq!(
        data.championships[0].points_system,
        vec![25, 18, 15, 12, 10]
    );
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
    persist(&store, &path).expect("the save must actually be written");

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
    store
        .write()
        .unwrap()
        .championships
        .push(sample_championship());
    persist(&store, &path).expect("the save must actually be written");

    // Add a second championship and persist again.
    let mut c2 = sample_championship();
    c2.id = "2".into();
    c2.name = "Second Champ".into();
    store.write().unwrap().championships.push(c2);
    persist(&store, &path).expect("the save must actually be written");

    let store3 = load_store(&path);
    assert_eq!(store3.read().unwrap().championships.len(), 2);
    fs::remove_file(&path).ok();
}

#[test]
fn test_persist_writes_valid_json() {
    let path = tmp();
    let store = load_store(&path);
    store.write().unwrap().sessions.push(sample_session());
    persist(&store, &path).expect("the save must actually be written");

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
    persist(&store, &path).expect("the save must actually be written");

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
        id: "c1".into(),
        name: "Test".into(),
        status: ChampionshipStatus::Active,
        points_system: pts,
        manufacturer_scoring: false,
        rounds: sessions
            .iter()
            .map(|&id| Round {
                session_ids: vec![id.into()],
            })
            .collect(),
        session_ids: vec![],
        custom_ai_file: None,
        player_team: None,
        planned_rounds: None,
    }
}

/// Marks the result belonging to `name` as the human player's row.
fn mark_player(mut session: RecordedSession, name: &str) -> RecordedSession {
    for r in &mut session.results {
        if r.name == name {
            r.is_player = true;
        }
    }
    session
}

fn make_session(
    id: &str,
    session_type: u32,
    results: Vec<(&str, u32, bool, &str)>,
) -> RecordedSession {
    RecordedSession {
        id: id.into(),
        recorded_at: 0,
        track: "Test Track".into(),
        track_variation: "".into(),
        car_name: "".into(),
        car_class: "".into(),
        session_type,
        results: results
            .into_iter()
            .map(|(name, pos, dnf, car)| SessionResult {
                name: name.into(),
                car_name: car.into(),
                car_class: "".into(),
                race_position: pos,
                laps_completed: 10,
                fastest_lap: 0.0,
                last_lap: 0.0,
                dnf,
                is_player: false,
            })
            .collect(),
        lap_chart: vec![],
    }
}

#[test]
fn test_standings_basic_points() {
    let champ = make_champ(vec![25, 18, 15], &["s1"]);
    let sessions = vec![make_session(
        "s1",
        5,
        vec![
            ("Alice", 1, false, ""),
            ("Bob", 2, false, ""),
            ("Carol", 3, false, ""),
        ],
    )];
    let st = standings(&champ, &sessions);
    assert_eq!(st[0].name, "Alice");
    assert_eq!(st[0].points, 25);
    assert_eq!(st[0].wins, 1);
    assert_eq!(st[1].name, "Bob");
    assert_eq!(st[1].points, 18);
    assert_eq!(st[1].wins, 0);
    assert_eq!(st[2].name, "Carol");
    assert_eq!(st[2].points, 15);
}

#[test]
fn test_standings_dnf_gets_no_points_and_no_win() {
    let champ = make_champ(vec![25, 18], &["s1"]);
    let sessions = vec![make_session(
        "s1",
        5,
        vec![
            ("Alice", 1, true, ""), // DNF even in P1
            ("Bob", 2, false, ""),
        ],
    )];
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
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
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
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
        make_session(
            "r2",
            5,
            vec![("Bob", 1, false, ""), ("Alice", 2, false, "")],
        ),
    ];
    let st = standings(&champ, &sessions);
    let alice = st.iter().find(|e| e.name == "Alice").unwrap();
    assert_eq!(alice.points, 25 + 18); // won r1, 2nd in r2
    let bob = st.iter().find(|e| e.name == "Bob").unwrap();
    assert_eq!(bob.points, 18 + 25); // 2nd in r1, won r2
}

#[test]
fn test_standings_sorted_by_points_then_wins() {
    let champ = make_champ(vec![10, 10], &["r1", "r2"]);
    // Alice and Bob both get 20 pts, but Alice has 2 wins vs Bob's 0
    let sessions = vec![
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
        make_session(
            "r2",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
    ];
    let st = standings(&champ, &sessions);
    assert_eq!(st[0].name, "Alice");
    assert_eq!(st[0].wins, 2);
    assert_eq!(st[1].name, "Bob");
}

#[test]
fn test_standings_position_beyond_points_system_gets_zero() {
    let champ = make_champ(vec![25, 18], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, false, ""),
            ("Bob", 2, false, ""),
            ("Carol", 3, false, ""), // P3 but only 2 points defined
        ],
    )];
    let st = standings(&champ, &sessions);
    let carol = st.iter().find(|e| e.name == "Carol").unwrap();
    assert_eq!(carol.points, 0);
}

// ── constructors ──────────────────────────────────────────────────────────────

#[test]
fn test_constructors_groups_by_car_name() {
    let champ = make_champ(vec![25, 18, 15], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, false, "Ferrari"),
            ("Bob", 2, false, "Ferrari"),
            ("Carol", 3, false, "McLaren"),
        ],
    )];
    let ct = constructors(&champ, &sessions, &HashMap::new());
    let ferrari = ct.iter().find(|e| e.name == "Ferrari").unwrap();
    assert_eq!(ferrari.points, 25 + 18); // Alice + Bob
    let mclaren = ct.iter().find(|e| e.name == "McLaren").unwrap();
    assert_eq!(mclaren.points, 15);
}

#[test]
fn test_constructors_dnf_excluded_from_points() {
    let champ = make_champ(vec![25, 18], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, true, "Ferrari"), // DNF
            ("Bob", 2, false, "McLaren"),
        ],
    )];
    let ct = constructors(&champ, &sessions, &HashMap::new());
    let ferrari = ct.iter().find(|e| e.name == "Ferrari").unwrap();
    assert_eq!(ferrari.points, 0);
}

#[test]
fn test_constructors_empty_car_name_uses_car_class() {
    let mut champ = make_champ(vec![25], &["r1"]);
    champ.manufacturer_scoring = true;
    let mut sess = make_session("r1", 5, vec![("Alice", 1, false, "")]);
    sess.results[0].car_class = "GT3".into();
    let ct = constructors(&champ, &[sess], &HashMap::new());
    assert!(ct.iter().any(|e| e.name == "GT3"));
}

#[test]
fn test_constructors_no_car_info_excluded() {
    let champ = make_champ(vec![25], &["r1"]);
    // car_name and car_class both empty — should not appear in constructors
    let sessions = vec![make_session("r1", 5, vec![("Alice", 1, false, "")])];
    let ct = constructors(&champ, &sessions, &HashMap::new());
    assert!(ct.is_empty());
}

// ── player_team override ─────────────────────────────────────────────────────

#[test]
fn test_constructors_player_team_override_used_for_player_row() {
    let mut champ = make_champ(vec![25, 18], &["r1"]);
    champ.player_team = Some("My Custom Team".into());
    let sessions = vec![mark_player(
        make_session(
            "r1",
            5,
            vec![
                ("Nightrat", 1, false, "Formula Junior"),
                ("Bob", 2, false, "McLaren"),
            ],
        ),
        "Nightrat",
    )];
    let ct = constructors(&champ, &sessions, &HashMap::new());
    assert!(ct
        .iter()
        .any(|e| e.name == "My Custom Team" && e.points == 25));
    assert!(!ct.iter().any(|e| e.name == "Formula Junior"));
}

#[test]
fn test_constructors_player_team_ignored_for_non_player_rows() {
    let mut champ = make_champ(vec![25], &["r1"]);
    champ.player_team = Some("My Custom Team".into());
    // No result is marked is_player — the override must not leak onto Alice's row.
    let sessions = vec![make_session("r1", 5, vec![("Alice", 1, false, "Ferrari")])];
    let ct = constructors(&champ, &sessions, &HashMap::new());
    assert!(ct.iter().any(|e| e.name == "Ferrari"));
    assert!(!ct.iter().any(|e| e.name == "My Custom Team"));
}

#[test]
fn test_constructors_custom_ai_map_takes_priority_over_player_team() {
    let mut champ = make_champ(vec![25], &["r1"]);
    champ.player_team = Some("Manual Team".into());
    let sessions = vec![mark_player(
        make_session("r1", 5, vec![("Jack Brabham", 1, false, "Formula Junior")]),
        "Jack Brabham",
    )];
    let mut team_map = HashMap::new();
    team_map.insert("Jack Brabham".to_string(), "Brabham-Repco".to_string());
    let ct = constructors(&champ, &sessions, &team_map);
    assert!(ct.iter().any(|e| e.name == "Brabham-Repco"));
    assert!(!ct.iter().any(|e| e.name == "Manual Team"));
}

#[test]
fn test_compute_career_player_team_shown_in_result_view() {
    let mut champ = make_champ(vec![25, 18], &["r1"]);
    champ.player_team = Some("My Custom Team".into());
    let sessions = vec![mark_player(
        make_session(
            "r1",
            5,
            vec![
                ("Nightrat", 1, false, "Formula Junior"),
                ("Bob", 2, false, "McLaren"),
            ],
        ),
        "Nightrat",
    )];
    let resp = compute_career(&[champ], &sessions);
    let race = &resp.championships[0].rounds[0].sessions[0];
    let player = race.results.iter().find(|r| r.name == "Nightrat").unwrap();
    let bob = race.results.iter().find(|r| r.name == "Bob").unwrap();
    assert_eq!(player.car_name, "My Custom Team");
    assert_eq!(bob.car_name, "McLaren");
}

#[test]
fn test_player_team_blank_string_treated_as_unset() {
    let mut champ = make_champ(vec![25], &["r1"]);
    champ.player_team = Some("".into());
    let sessions = vec![mark_player(
        make_session("r1", 5, vec![("Nightrat", 1, false, "Formula Junior")]),
        "Nightrat",
    )];
    let ct = constructors(&champ, &sessions, &HashMap::new());
    // Falls through to car_name since the override is blank.
    assert!(ct.iter().any(|e| e.name == "Formula Junior"));
}

// ── compute_career ────────────────────────────────────────────────────────────

#[test]
fn test_compute_career_race_stats_accumulated() {
    let champ = make_champ(vec![25, 18, 15], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, false, ""),
            ("Bob", 2, false, ""),
            ("Carol", 3, false, ""),
        ],
    )];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
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
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, true, ""), // DNF
            ("Bob", 2, false, ""),
        ],
    )];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
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
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
        make_session(
            "r2",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
    ];
    let resp = compute_career(&[active, finished], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
    assert_eq!(alice.champ_wins, 1); // only the Final one counts
}

#[test]
fn test_compute_career_avg_pos() {
    let champ = make_champ(vec![25, 18], &["r1", "r2"]);
    let sessions = vec![
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
        make_session(
            "r2",
            5,
            vec![("Alice", 3, false, ""), ("Bob", 1, false, "")],
        ),
    ];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
    // (1 + 3) / 2 = 2.0
    assert!((alice.avg_pos - 2.0).abs() < f32::EPSILON);
}

#[test]
fn test_compute_career_driver_stats_sorted_by_wins_then_races() {
    let champ = make_champ(vec![25, 18], &["r1", "r2"]);
    let sessions = vec![
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
        make_session(
            "r2",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
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
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
    ];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
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
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, false, ""),
            ("Bob", 2, false, ""),
            ("Carol", 3, false, ""),
        ],
    )];
    let resp = compute_career(&[champ], &sessions);
    let race = &resp.championships[0].rounds[0].sessions[0];
    let alice = race.results.iter().find(|r| r.name == "Alice").unwrap();
    let bob = race.results.iter().find(|r| r.name == "Bob").unwrap();
    let carol = race.results.iter().find(|r| r.name == "Carol").unwrap();
    assert_eq!(alice.points_earned, 25);
    assert_eq!(bob.points_earned, 18);
    assert_eq!(carol.points_earned, 15);
}

#[test]
fn test_compute_career_dnf_earns_no_points_in_view() {
    let champ = make_champ(vec![25, 18], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, true, ""), // DNF
            ("Bob", 2, false, ""),
        ],
    )];
    let resp = compute_career(&[champ], &sessions);
    let race = &resp.championships[0].rounds[0].sessions[0];
    let alice = race.results.iter().find(|r| r.name == "Alice").unwrap();
    assert_eq!(alice.points_earned, 0);
}

#[test]
fn test_compute_career_position_beyond_points_earns_zero_in_view() {
    let champ = make_champ(vec![25, 18], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, false, ""),
            ("Bob", 2, false, ""),
            ("Carol", 3, false, ""), // P3 but only 2 positions in points system
        ],
    )];
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
        make_session(
            "q1",
            3,
            vec![
                ("Alice", 1, false, ""),
                ("Bob", 2, false, ""),
                ("Carol", 3, false, ""),
            ],
        ),
        make_session(
            "r1",
            5,
            vec![
                ("Alice", 1, false, ""),
                ("Bob", 2, false, ""),
                ("Carol", 3, false, ""),
            ],
        ),
    ];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
    let bob = resp.driver_stats.iter().find(|d| d.name == "Bob").unwrap();
    let carol = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Carol")
        .unwrap();
    assert_eq!(alice.quali_p1, 1);
    assert_eq!(alice.quali_p2, 0);
    assert_eq!(alice.quali_p3, 0);
    assert_eq!(bob.quali_p1, 0);
    assert_eq!(bob.quali_p2, 1);
    assert_eq!(bob.quali_p3, 0);
    assert_eq!(carol.quali_p1, 0);
    assert_eq!(carol.quali_p2, 0);
    assert_eq!(carol.quali_p3, 1);
}

#[test]
fn test_compute_career_quali_top10_boundary() {
    let champ = make_champ(vec![25], &["q1"]);
    let sessions = vec![make_session(
        "q1",
        3,
        vec![
            ("Alice", 1, false, ""),
            ("Bob", 10, false, ""),
            ("Carol", 11, false, ""), // just outside top 10
        ],
    )];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
    let bob = resp.driver_stats.iter().find(|d| d.name == "Bob").unwrap();
    let carol = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Carol")
        .unwrap();
    assert_eq!(alice.quali_top10, 1);
    assert_eq!(bob.quali_top10, 1);
    assert_eq!(carol.quali_top10, 0);
}

#[test]
fn test_compute_career_quali_not_counted_as_race() {
    let champ = make_champ(vec![25], &["q1"]);
    let sessions = vec![make_session("q1", 3, vec![("Alice", 1, false, "")])];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
    assert_eq!(alice.races, 0);
    assert_eq!(alice.p1, 0);
    assert_eq!(alice.quali_p1, 1);
}

// ── champ_p2 / champ_p3 ──────────────────────────────────────────────────────

#[test]
fn test_compute_career_champ_p2_p3_for_final_championship() {
    let mut champ = make_champ(vec![25, 18, 15], &["r1"]);
    champ.status = ChampionshipStatus::Final;
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, false, ""),
            ("Bob", 2, false, ""),
            ("Carol", 3, false, ""),
        ],
    )];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
    let bob = resp.driver_stats.iter().find(|d| d.name == "Bob").unwrap();
    let carol = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Carol")
        .unwrap();
    assert_eq!(alice.champ_wins, 1);
    assert_eq!(alice.champ_p2, 0);
    assert_eq!(alice.champ_p3, 0);
    assert_eq!(bob.champ_wins, 0);
    assert_eq!(bob.champ_p2, 1);
    assert_eq!(bob.champ_p3, 0);
    assert_eq!(carol.champ_wins, 0);
    assert_eq!(carol.champ_p2, 0);
    assert_eq!(carol.champ_p3, 1);
}

#[test]
fn test_compute_career_champ_standings_not_counted_for_active() {
    let champ = make_champ(vec![25, 18, 15], &["r1"]);
    // status defaults to Active
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, false, ""),
            ("Bob", 2, false, ""),
            ("Carol", 3, false, ""),
        ],
    )];
    let resp = compute_career(&[champ], &sessions);
    let alice = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Alice")
        .unwrap();
    let bob = resp.driver_stats.iter().find(|d| d.name == "Bob").unwrap();
    let carol = resp
        .driver_stats
        .iter()
        .find(|d| d.name == "Carol")
        .unwrap();
    assert_eq!(alice.champ_wins, 0);
    assert_eq!(bob.champ_p2, 0);
    assert_eq!(carol.champ_p3, 0);
}

// ── track_stats ───────────────────────────────────────────────────────────────

/// Helper: a race session at a specific track/timestamp with (name, pos, fastest_lap, car_name).
/// session_car is the player's car at the session level (used for per-car track stats grouping).
fn make_track_session(
    id: &str,
    session_type: u32,
    track: &str,
    variation: &str,
    recorded_at: u64,
    results: Vec<(&str, u32, f32, &str)>,
) -> RecordedSession {
    make_track_session_car(id, session_type, track, variation, recorded_at, "", results)
}

fn make_track_session_car(
    id: &str,
    session_type: u32,
    track: &str,
    variation: &str,
    recorded_at: u64,
    session_car: &str,
    results: Vec<(&str, u32, f32, &str)>,
) -> RecordedSession {
    RecordedSession {
        id: id.into(),
        recorded_at,
        track: track.into(),
        track_variation: variation.into(),
        car_name: session_car.into(),
        car_class: "".into(),
        session_type,
        results: results
            .into_iter()
            .map(|(name, pos, fl, car)| SessionResult {
                name: name.into(),
                car_name: car.into(),
                car_class: "".into(),
                race_position: pos,
                laps_completed: 10,
                fastest_lap: fl,
                last_lap: 0.0,
                dnf: false,
                is_player: false,
            })
            .collect(),
        lap_chart: vec![],
    }
}

#[test]
fn test_track_stats_race_and_qualifying_counts() {
    let sessions = vec![
        make_track_session(
            "r1",
            5,
            "Silverstone",
            "GP",
            1000,
            vec![("Alice", 1, 90.0, "Car")],
        ),
        make_track_session(
            "q1",
            3,
            "Silverstone",
            "GP",
            900,
            vec![("Alice", 1, 89.5, "Car")],
        ),
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
    let sessions = vec![make_track_session(
        "r1",
        5,
        "Spa",
        "GP",
        1000,
        vec![
            ("Alice", 1, 120.0, "Ferrari"),
            ("Bob", 2, 118.5, "McLaren"), // Bob sets the fastest lap
        ],
    )];
    let resp = compute_career(&[], &sessions);
    let ts = &resp.track_stats[0];
    assert!((ts.best_lap - 118.5).abs() < 0.001);
    assert_eq!(ts.best_lap_driver, "Bob");
    assert_eq!(ts.best_lap_car, "McLaren");
}

#[test]
fn test_track_stats_best_lap_car_class_fallback() {
    let mut sess = make_track_session("r1", 5, "Monza", "GP", 1000, vec![("Alice", 1, 90.0, "")]);
    sess.results[0].car_class = "GT3".into();
    let resp = compute_career(&[], &[sess]);
    assert_eq!(resp.track_stats[0].best_lap_car, "GT3");
}

#[test]
fn test_track_stats_best_lap_updated_across_sessions() {
    let sessions = vec![
        make_track_session(
            "r1",
            5,
            "Spa",
            "GP",
            1000,
            vec![("Alice", 1, 120.0, "Ferrari")],
        ),
        make_track_session(
            "r2",
            5,
            "Spa",
            "GP",
            2000,
            vec![("Bob", 1, 118.0, "McLaren")],
        ),
    ];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats.len(), 1);
    let ts = &resp.track_stats[0];
    assert_eq!(ts.races, 2);
    assert!((ts.best_lap - 118.0).abs() < 0.001);
    assert_eq!(ts.best_lap_driver, "Bob");
    assert_eq!(ts.best_lap_car, "McLaren");
}

#[test]
fn test_track_stats_track_variation_is_separate_key() {
    let sessions = vec![
        make_track_session(
            "r1",
            5,
            "Silverstone",
            "GP",
            1000,
            vec![("Alice", 1, 90.0, "")],
        ),
        make_track_session(
            "r2",
            5,
            "Silverstone",
            "National",
            2000,
            vec![("Alice", 1, 70.0, "")],
        ),
    ];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats.len(), 2);
}

#[test]
fn test_track_stats_sorted_by_last_visited_desc() {
    let sessions = vec![
        make_track_session(
            "r1",
            5,
            "Silverstone",
            "GP",
            1000,
            vec![("Alice", 1, 90.0, "")],
        ),
        make_track_session("r2", 5, "Monza", "GP", 2000, vec![("Alice", 1, 85.0, "")]),
        make_track_session("r3", 5, "Spa", "GP", 500, vec![("Alice", 1, 95.0, "")]),
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
        make_track_session("r2", 5, "Spa", "GP", 3000, vec![("Bob", 1, 95.0, "")]),
        make_track_session("r3", 5, "Spa", "GP", 2000, vec![("Carol", 1, 88.0, "")]),
    ];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats[0].last_visited, 3000);
}

#[test]
fn test_track_stats_practice_not_counted_as_race_or_qualifying() {
    let sessions = vec![make_track_session(
        "p1",
        1,
        "Spa",
        "GP",
        1000,
        vec![("Alice", 1, 90.0, "")],
    )];
    let resp = compute_career(&[], &sessions);
    assert_eq!(resp.track_stats.len(), 1);
    let ts = &resp.track_stats[0];
    assert_eq!(ts.races, 0);
    assert_eq!(ts.qualifyings, 0);
}

#[test]
fn test_track_stats_top3_laps_ranked_by_driver_best() {
    let sessions = vec![make_track_session(
        "r1",
        5,
        "Spa",
        "GP",
        1000,
        vec![
            ("Alice", 1, 90.0, "Ferrari"),  // best for Alice
            ("Bob", 2, 88.0, "McLaren"),    // best for Bob
            ("Carol", 3, 92.0, "Williams"), // best for Carol
            ("Dave", 4, 85.0, "RedBull"),   // best for Dave — fastest overall
        ],
    )];
    let resp = compute_career(&[], &sessions);
    let ts = &resp.track_stats[0];
    assert!((ts.best_lap - 85.0).abs() < 0.001);
    assert_eq!(ts.best_lap_driver, "Dave");
    assert!((ts.second_lap - 88.0).abs() < 0.001);
    assert_eq!(ts.second_lap_driver, "Bob");
    assert!((ts.third_lap - 90.0).abs() < 0.001);
    assert_eq!(ts.third_lap_driver, "Alice");
}

#[test]
fn test_track_stats_top3_uses_per_driver_best_across_sessions() {
    let sessions = vec![
        make_track_session(
            "r1",
            5,
            "Spa",
            "GP",
            1000,
            vec![("Alice", 1, 90.0, "Ferrari")],
        ),
        make_track_session(
            "r2",
            5,
            "Spa",
            "GP",
            2000,
            vec![("Alice", 1, 88.5, "Ferrari"), ("Bob", 2, 89.0, "McLaren")],
        ),
    ];
    // Alice appears in both sessions; her per-driver best is 88.5
    let resp = compute_career(&[], &sessions);
    let ts = &resp.track_stats[0];
    assert!((ts.best_lap - 88.5).abs() < 0.001);
    assert_eq!(ts.best_lap_driver, "Alice");
    assert!((ts.second_lap - 89.0).abs() < 0.001);
    assert_eq!(ts.second_lap_driver, "Bob");
    assert_eq!(ts.third_lap, 0.0);
}

#[test]
fn test_track_stats_fewer_than_3_drivers_fills_zeros() {
    let sessions = vec![make_track_session(
        "r1",
        5,
        "Spa",
        "GP",
        1000,
        vec![("Alice", 1, 90.0, "Ferrari")],
    )];
    let resp = compute_career(&[], &sessions);
    let ts = &resp.track_stats[0];
    assert!((ts.best_lap - 90.0).abs() < 0.001);
    assert_eq!(ts.second_lap, 0.0);
    assert_eq!(ts.third_lap, 0.0);
}

#[test]
fn test_track_stats_different_session_cars_are_separate_entries() {
    let sessions = vec![
        make_track_session_car(
            "r1",
            5,
            "Spa",
            "GP",
            1000,
            "Ferrari",
            vec![("Alice", 1, 90.0, "Ferrari")],
        ),
        make_track_session_car(
            "r2",
            5,
            "Spa",
            "GP",
            2000,
            "McLaren",
            vec![("Alice", 1, 88.0, "McLaren")],
        ),
        make_track_session_car(
            "r3",
            5,
            "Spa",
            "GP",
            3000,
            "Ferrari",
            vec![("Alice", 1, 89.5, "Ferrari")],
        ),
    ];
    let resp = compute_career(&[], &sessions);
    // Two cars → two entries for Spa
    assert_eq!(resp.track_stats.len(), 2);
    let ferrari = resp
        .track_stats
        .iter()
        .find(|t| t.car == "Ferrari")
        .unwrap();
    let mclaren = resp
        .track_stats
        .iter()
        .find(|t| t.car == "McLaren")
        .unwrap();
    assert_eq!(ferrari.races, 2);
    assert_eq!(mclaren.races, 1);
    assert!((ferrari.best_lap - 89.5).abs() < 0.001);
    assert!((mclaren.best_lap - 88.0).abs() < 0.001);
}

// ── Pre-selecting the performance tabs' class filter ──────────────────────────

fn champ_with(id: &str, status: ChampionshipStatus, file: Option<&str>) -> Championship {
    Championship {
        id: id.into(),
        name: id.into(),
        status,
        points_system: vec![25, 18, 15],
        manufacturer_scoring: false,
        rounds: vec![],
        session_ids: vec![],
        custom_ai_file: file.map(str::to_string),
        player_team: None,
        planned_rounds: None,
    }
}

#[test]
fn test_active_classes_prefers_championships_under_way() {
    let champs = vec![
        champ_with("a", ChampionshipStatus::Active, Some("F-Vintage_Gen1.xml")),
        champ_with("b", ChampionshipStatus::Progress, Some("F-Retro_Gen2.xml")),
        champ_with("c", ChampionshipStatus::Final, Some("F-Classic_Gen1.xml")),
    ];
    // Rounds are under way in exactly one, so that is the season being raced.
    assert_eq!(active_classes(&champs), vec!["F-Retro_Gen2"]);
}

#[test]
fn test_active_classes_falls_back_to_not_yet_started() {
    let champs = vec![
        champ_with("a", ChampionshipStatus::Active, Some("F-Vintage_Gen1.xml")),
        champ_with("c", ChampionshipStatus::Final, Some("F-Classic_Gen1.xml")),
    ];
    // Nothing under way: the one being set up is the next best answer.
    assert_eq!(active_classes(&champs), vec!["F-Vintage_Gen1"]);
}

#[test]
fn test_active_classes_never_offers_a_finished_season() {
    let champs = vec![champ_with(
        "c",
        ChampionshipStatus::Final,
        Some("F-Classic_Gen1.xml"),
    )];
    assert!(active_classes(&champs).is_empty(), "no preference is right");
}

#[test]
fn test_active_classes_dedupes_and_skips_championships_without_a_roster() {
    let champs = vec![
        champ_with("a", ChampionshipStatus::Progress, Some("F-Retro_Gen2.xml")),
        champ_with("b", ChampionshipStatus::Progress, Some("F-Retro_Gen2.xml")),
        champ_with("c", ChampionshipStatus::Progress, Some("F-Retro_Gen3.xml")),
        champ_with("d", ChampionshipStatus::Progress, None),
    ];
    assert_eq!(
        active_classes(&champs),
        vec!["F-Retro_Gen2", "F-Retro_Gen3"]
    );
}

#[test]
fn test_active_classes_empty_for_no_championships() {
    assert!(active_classes(&[]).is_empty());
}

// ── Loading a damaged or BOM-prefixed save ───────────────────────────────────

fn load_tmp(tag: &str) -> PathBuf {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("ams2_load_test_{tag}_{ns}.json"))
}

const ONE_CHAMP: &str = r#"{"sessions":[],"championships":[{"id":"c1","name":"Season","points_system":[25],"rounds":[]}]}"#;

#[test]
fn test_a_missing_save_is_an_empty_career_not_an_error() {
    // That is how a new save begins, so it must not read as damage.
    let path = load_tmp("missing");
    let data = try_load_data(&path).expect("a file that is not there yet is not an error");
    assert!(data.sessions.is_empty() && data.championships.is_empty());
}

#[test]
fn test_a_byte_order_mark_is_stripped_before_parsing() {
    // Notepad and PowerShell both write one by default on Windows, and serde_json rejects it.
    let path = load_tmp("bom");
    fs::write(&path, format!("\u{feff}{ONE_CHAMP}")).unwrap();
    let data = try_load_data(&path).expect("a BOM must not make a save unreadable");
    assert_eq!(data.championships.len(), 1);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_a_damaged_save_is_an_error_rather_than_an_empty_career() {
    // The whole point: defaulting here would present an empty career as if it were real.
    let path = load_tmp("damaged");
    fs::write(&path, "{ this is not json").unwrap();
    assert!(try_load_data(&path).is_err());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_persist_refuses_to_overwrite_a_save_it_could_not_read() {
    let path = load_tmp("guard");
    let broken = "{ not json at all";
    fs::write(&path, broken).unwrap();

    // An empty store, exactly as the startup path would leave it after a failed load.
    let store = load_store(&path);
    let err = persist(&store, &path).expect_err("writing over an unread career must be refused");
    assert!(err.contains("refusing to overwrite"), "{err}");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        broken,
        "the file must be byte-for-byte untouched"
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn test_persist_refuses_when_there_is_no_active_career() {
    // The saves folder was empty at startup, so `resolve_active` found nothing and the app runs
    // without a career rather than inventing one. There is nowhere for this to go.
    let store = load_store(&PathBuf::new());
    let err = persist(&store, &PathBuf::new()).expect_err("there is nothing to write to");
    assert!(err.contains("no active career"), "{err}");
}

#[test]
fn test_persist_writes_a_save_it_can_read() {
    let path = load_tmp("ok");
    fs::write(&path, ONE_CHAMP).unwrap();
    let store = load_store(&path);
    store.write().unwrap().championships.clear();

    persist(&store, &path).expect("a readable save may be written");
    assert!(try_load_data(&path).unwrap().championships.is_empty());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_persist_creates_a_save_that_does_not_exist_yet() {
    // The guard must not stop a brand new career from ever being written.
    let path = load_tmp("new");
    let store = load_store(&path);
    persist(&store, &path).expect("a missing file is not a career to protect");
    assert!(path.exists());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_persist_accepts_a_save_that_only_had_a_byte_order_mark() {
    // The guard parses the same way the loader does, so a BOM must not make a file permanently
    // unwritable — the career was read fine, and the rewrite drops the mark.
    let path = load_tmp("bomguard");
    fs::write(&path, format!("\u{feff}{ONE_CHAMP}")).unwrap();
    let store = load_store(&path);
    assert_eq!(store.read().unwrap().championships.len(), 1);
    persist(&store, &path).expect("a BOM is not damage");
    assert!(!fs::read_to_string(&path).unwrap().starts_with('\u{feff}'));
    let _ = fs::remove_file(&path);
}

#[test]
fn test_driver_standings_name_the_team_falling_back_to_the_car() {
    let champ = make_champ(vec![25, 18, 15], &["s1"]);
    let mut sessions = vec![make_session(
        "s1",
        5,
        vec![
            ("Alice", 1, false, "Lotus 49"),
            ("Bob", 2, false, "Brabham BT26"),
            ("Carol", 3, false, "Matra MS80"),
        ],
    )];
    // Carol is the player, and the seat is named on the championship rather than in
    // any roster — the one case the Custom AI file can never answer.
    sessions[0].results[2].is_player = true;

    let mut champ = champ;
    champ.player_team = Some("Tyrrell".into());

    let mut roster = std::collections::HashMap::new();
    roster.insert("Alice".to_string(), "Team Lotus".to_string());

    let st = standings_with(&champ, &sessions, &roster);
    let team_of = |n: &str| {
        st.iter()
            .find(|e| e.name == n)
            .unwrap()
            .team
            .clone()
            .unwrap()
    };
    assert_eq!(team_of("Alice"), "Team Lotus"); // roster wins
    assert_eq!(team_of("Carol"), "Tyrrell"); // player override, absent from the roster
    assert_eq!(team_of("Bob"), "Brabham BT26"); // nothing named it, so the car
}

#[test]
fn test_standings_without_a_roster_names_no_team() {
    let champ = make_champ(vec![25, 18], &["s1"]);
    let sessions = vec![make_session("s1", 5, vec![("Alice", 1, false, "")])];
    // Neither a roster nor a car to fall back to: the field is absent rather than
    // an empty string, so the standings table draws no team span at all.
    assert_eq!(standings(&champ, &sessions)[0].team, None);
}



// ── FIA countback ────────────────────────────────────────────────────────────
// Equal points are separated by most wins, then most seconds, then most thirds,
// and so on. A retirement is not a place and so never enters the countback.

#[test]
fn test_countback_separates_equal_points_on_second_places() {
    // Only a win scores, so both drivers finish the season level on 10.
    let champ = make_champ(vec![10], &["r1", "r2", "r3"]);
    let sessions = vec![
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
        make_session(
            "r2",
            5,
            vec![("Bob", 1, false, ""), ("Alice", 2, false, "")],
        ),
        make_session(
            "r3",
            5,
            vec![("Alice", 2, false, ""), ("Bob", 3, false, "")],
        ),
    ];
    let st = standings(&champ, &sessions);
    assert_eq!(st[0].points, st[1].points, "the tie is the point of the test");
    assert_eq!(st[0].wins, st[1].wins, "and it survives the win count");
    // Two seconds against one.
    assert_eq!(st[0].name, "Alice");
    assert_eq!(st[1].name, "Bob");
}

#[test]
fn test_countback_walks_on_down_the_order_until_it_finds_a_difference() {
    // Level on points, wins and seconds — only the third places separate them.
    let champ = make_champ(vec![10], &["r1", "r2", "r3"]);
    let sessions = vec![
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, ""), ("Bob", 2, false, "")],
        ),
        make_session(
            "r2",
            5,
            vec![("Bob", 1, false, ""), ("Alice", 2, false, "")],
        ),
        make_session(
            "r3",
            5,
            vec![("Alice", 3, false, ""), ("Bob", 4, false, "")],
        ),
    ];
    let st = standings(&champ, &sessions);
    assert_eq!(st[0].points, st[1].points);
    assert_eq!(st[0].wins, st[1].wins);
    assert_eq!(st[0].name, "Alice"); // a third beats a fourth
}

#[test]
fn test_a_retirement_is_not_a_place_in_the_countback() {
    // Bob is classified P1 but retired, so it is neither a win nor a place —
    // the same rule that stops it scoring. Alice's P2 is worth no points here
    // and still puts her ahead.
    let champ = make_champ(vec![10], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![("Bob", 1, true, ""), ("Alice", 2, false, "")],
    )];
    let st = standings(&champ, &sessions);
    assert_eq!(st[0].points, 0);
    assert_eq!(st[1].points, 0);
    assert_eq!(st[0].wins, 0, "a retirement is not a win");
    assert_eq!(st[0].name, "Alice");
}

#[test]
fn test_anyone_who_finished_outranks_someone_who_never_did() {
    // Both are pointless. Carol has a finish to her name; Dave has nothing.
    let champ = make_champ(vec![10], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Alice", 1, false, ""),
            ("Carol", 2, false, ""),
            ("Dave", 3, true, ""),
        ],
    )];
    let st = standings(&champ, &sessions);
    assert_eq!(st[1].name, "Carol");
    assert_eq!(st[2].name, "Dave");
}

#[test]
fn test_a_table_with_nothing_to_separate_it_is_still_stable() {
    // Every driver retired: no points, no places, nothing for the countback to
    // read. This used to fall through to `HashMap` order, which is seeded per
    // map — so the tail of the table reshuffled on every request.
    let champ = make_champ(vec![25, 18], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![
            ("Erin", 1, true, ""),
            ("Carol", 2, true, ""),
            ("Alice", 3, true, ""),
            ("Dave", 4, true, ""),
            ("Bob", 5, true, ""),
        ],
    )];
    let first = standings(&champ, &sessions);
    let names: Vec<&str> = first.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Alice", "Bob", "Carol", "Dave", "Erin"]);
    for _ in 0..8 {
        let again = standings(&champ, &sessions);
        let again: Vec<&str> = again.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, again, "same career, same order, every time");
    }
}

#[test]
fn test_a_hand_edited_position_off_the_grid_is_ignored_not_allocated() {
    // Career files are hand-edited, and the countback indexes by position. A
    // number past the grid is not a place; it must not become a 4-billion-entry
    // resize either.
    let champ = make_champ(vec![10], &["r1"]);
    let sessions = vec![make_session(
        "r1",
        5,
        vec![("Alice", 1, false, ""), ("Bob", 999_999, false, "")],
    )];
    let st = standings(&champ, &sessions);
    let bob = st.iter().find(|e| e.name == "Bob").unwrap();
    assert_eq!(bob.points, 0);
    assert_eq!(bob.wins, 0);
}

#[test]
fn test_constructor_standings_use_the_same_countback() {
    // Both teams level on points and wins; one has the better second place.
    let champ = make_champ(vec![10], &["r1", "r2", "r3"]);
    let sessions = vec![
        make_session(
            "r1",
            5,
            vec![("Alice", 1, false, "Lotus"), ("Bob", 2, false, "Brabham")],
        ),
        make_session(
            "r2",
            5,
            vec![("Bob", 1, false, "Brabham"), ("Alice", 2, false, "Lotus")],
        ),
        make_session(
            "r3",
            5,
            vec![("Alice", 2, false, "Lotus"), ("Bob", 3, false, "Brabham")],
        ),
    ];
    let career = compute_career(&[champ], &sessions);
    let cs = &career.championships[0].constructor_standings;
    assert_eq!(cs[0].points, cs[1].points);
    assert_eq!(cs[0].name, "Lotus");
}

// ── The career view flags a session raced on the wrong grid ──────────────────
//
// The Career tab's copy of the grid warning. It is attached per session rather than per season
// because a season can be raced half on its roster and half not, and which half counts is the
// whole point.

/// A four-car roster, written where a championship can point at it.
fn grid_roster_dir() -> PathBuf {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_grid_note_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("grid.xml"),
        r#"<custom_ai_drivers>
    <driver livery_name="1986 Williams #5 - N. Mansell"><name>Nigel Mansell</name></driver>
    <driver livery_name="1986 Williams #6 - N. Piquet"><name>Nelson Piquet</name></driver>
    <driver livery_name="1986 Osella #21 - P. Ghinzani"><name>Piercarlo Ghinzani</name></driver>
    <driver livery_name="1986 Osella #22 - A. Berg"><name>Allan Berg</name></driver>
</custom_ai_drivers>"#,
    )
    .unwrap();
    dir
}

fn grid_note_session(id: &str, session_type: u32, names: &[(&str, bool)]) -> RecordedSession {
    RecordedSession {
        id: id.into(),
        recorded_at: 1_700_000_000,
        track: "Monza".into(),
        track_variation: String::new(),
        car_name: "Formula Classic Gen1".into(),
        car_class: "F-Classic_Gen1".into(),
        session_type,
        results: names
            .iter()
            .enumerate()
            .map(|(i, (name, is_player))| SessionResult {
                name: (*name).into(),
                car_name: "Formula Classic Gen1".into(),
                car_class: "F-Classic_Gen1".into(),
                race_position: i as u32 + 1,
                laps_completed: 10,
                fastest_lap: 90.0,
                last_lap: 90.0,
                dnf: false,
                is_player: *is_player,
            })
            .collect(),
        lap_chart: vec![],
    }
}

fn grid_note_champ(ids: &[&str]) -> Championship {
    let mut champ = sample_championship();
    champ.custom_ai_file = Some("grid.xml".into());
    champ.rounds = vec![Round {
        session_ids: ids.iter().map(|s| (*s).to_string()).collect(),
    }];
    champ.session_ids = ids.iter().map(|s| (*s).to_string()).collect();
    champ
}

#[test]
fn test_career_view_flags_a_race_that_did_not_fill_the_roster() {
    let dir = grid_roster_dir();
    let short = grid_note_session("r1", SESSION_RACE, &[("Nigel Mansell", false), ("Me", true)]);
    let view = compute_career_full(&[grid_note_champ(&["r1"])], &[short], Some(&dir));

    let note = view.championships[0].rounds[0].sessions[0]
        .grid_note
        .as_deref()
        .expect("two cars of a four-car roster is a short grid");
    assert!(note.contains("Short grid"), "{note}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_career_view_leaves_a_full_grid_unflagged() {
    let dir = grid_roster_dir();
    let full = grid_note_session(
        "r1",
        SESSION_RACE,
        &[
            ("Nigel Mansell", false),
            ("Nelson Piquet", false),
            ("Piercarlo Ghinzani", false),
            ("Me", true),
        ],
    );
    let view = compute_career_full(&[grid_note_champ(&["r1"])], &[full], Some(&dir));

    assert!(view.championships[0].rounds[0].sessions[0]
        .grid_note
        .is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Nothing is derived from practice, so a short practice grid is a choice rather than a
/// mistake — flagging it would put a warning on the one session type it cannot affect.
#[test]
fn test_career_view_does_not_flag_practice() {
    let dir = grid_roster_dir();
    let practice = grid_note_session(
        "p1",
        SESSION_PRACTICE,
        &[("Nigel Mansell", false), ("Me", true)],
    );
    let view = compute_career_full(&[grid_note_champ(&["p1"])], &[practice], Some(&dir));

    assert!(view.championships[0].rounds[0].sessions[0]
        .grid_note
        .is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// With no roster there is nothing to compare against, and silence is the only honest answer —
/// the same reason the Manage tab says why instead of reporting all clear.
#[test]
fn test_career_view_says_nothing_without_a_roster() {
    let short = grid_note_session("r1", SESSION_RACE, &[("Nigel Mansell", false), ("Me", true)]);
    let view = compute_career_full(&[grid_note_champ(&["r1"])], &[short], None);

    assert!(view.championships[0].rounds[0].sessions[0]
        .grid_note
        .is_none());
}
