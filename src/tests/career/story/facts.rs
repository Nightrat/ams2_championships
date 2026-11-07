use super::*;
use crate::career::data_store::{ChampionshipStatus, Round, SeatLock, SessionResult, SESSION_PRACTICE};

fn result(name: &str, position: u32, dnf: bool, is_player: bool) -> SessionResult {
    SessionResult {
        name: name.into(),
        car_name: String::new(),
        car_class: String::new(),
        race_position: position,
        laps_completed: 10,
        fastest_lap: 90.0,
        last_lap: 90.0,
        dnf,
        is_player,
    }
}

fn session(id: &str, session_type: u32, recorded_at: u64, results: Vec<SessionResult>) -> RecordedSession {
    RecordedSession {
        id: id.into(),
        recorded_at,
        track: "Monza".into(),
        track_variation: String::new(),
        car_name: String::new(),
        car_class: String::new(),
        session_type,
        results,
        lap_chart: vec![],
    }
}

fn champ(rounds: Vec<Vec<&str>>) -> Championship {
    Championship {
        id: "c1".into(),
        name: "1986".into(),
        status: ChampionshipStatus::Active,
        points_system: vec![9, 6, 4],
        manufacturer_scoring: false,
        rounds: rounds
            .into_iter()
            .map(|ids| Round {
                session_ids: ids.into_iter().map(String::from).collect(),
            })
            .collect(),
        session_ids: vec![],
        custom_ai_file: Some("F-Classic_Gen1.xml".into()),
        player_team: Some("Osella".into()),
        player_seat: Some(SeatLock {
            seat: "Osella #21".into(),
            replaces: "Piercarlo Ghinzani".into(),
        }),
        planned_rounds: None,
        team_lineage: Vec::new(),
    }
}

fn roster() -> Vec<RosterDriver> {
    let d = |name: &str, team: &str, stand_in: bool| RosterDriver {
        name: name.into(),
        team: team.into(),
        stand_in,
    };
    vec![
        d("Nigel Mansell", "Williams", false),
        d("Piercarlo Ghinzani", "Osella", false),
        d("Allan Berg", "Osella", false),
        // A stand-in sharing a regular driver's name key: the regular entry must win.
        d("Nigel Mansell", "Osella", true),
    ]
}

#[test]
fn test_practice_is_dropped_and_qualifying_comes_before_the_race() {
    let sessions = vec![
        session("r1", SESSION_RACE, 30, vec![result("Nightrat", 1, false, true)]),
        session("p1", SESSION_PRACTICE, 10, vec![result("Nightrat", 1, false, true)]),
        session("q1", SESSION_QUALIFY, 20, vec![result("Nightrat", 1, false, true)]),
        session("p2", SESSION_PRACTICE, 40, vec![result("Nightrat", 1, false, true)]),
    ];
    let facts = SeasonFacts::new(&champ(vec![vec!["r1", "p1", "q1"], vec!["p2"], vec![]]), &sessions, roster());

    let ids: Vec<&str> = facts.rounds[0].sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["q1", "r1"]);
    // Rounds with nothing counted are kept, so round numbers match the tabs.
    assert_eq!(facts.rounds.len(), 3);
    assert_eq!(facts.rounds[2].number, 3);
    assert!(facts.rounds[1].sessions.is_empty());
    assert_eq!(facts.sessions().count(), 2);
}

#[test]
fn test_results_are_matched_to_the_roster_by_name_key() {
    let sessions = vec![session(
        "r1",
        SESSION_RACE,
        1,
        vec![
            // Telemetry's spelling, not the file's.
            result("Allen Berg", 1, false, false),
            result("Nigel Mansell", 2, false, false),
            result("Nilton Pereira (AI)", 3, false, false),
            result("Nightrat", 4, false, true),
        ],
    )];
    let facts = SeasonFacts::new(&champ(vec![vec!["r1"]]), &sessions, roster());
    let r = &facts.rounds[0].sessions[0].results;

    assert_eq!(r[0].roster.as_ref().unwrap().name, "Allan Berg");
    assert_eq!(r[0].team(facts.player_team.as_deref()), Some("Osella"));
    // The regular entry wins over a stand-in of the same name.
    assert_eq!(r[1].roster.as_ref().unwrap().team, "Williams");
    assert!(!r[1].roster.as_ref().unwrap().stand_in);
    // Stock AI is nobody's.
    assert_eq!(r[2].roster, None);
    // The player is never matched to a roster entry, and drives for the declared team.
    assert_eq!(r[3].roster, None);
    assert_eq!(r[3].team(facts.player_team.as_deref()), Some("Osella"));
}

#[test]
fn test_points_are_scored_in_races_only_and_never_for_a_retirement() {
    let sessions = vec![
        session("q1", SESSION_QUALIFY, 1, vec![result("Nightrat", 1, false, true)]),
        session(
            "r1",
            SESSION_RACE,
            2,
            vec![
                result("Nightrat", 1, false, true),
                result("Allen Berg", 2, true, false),
                result("Nigel Mansell", 9, false, false),
            ],
        ),
    ];
    let facts = SeasonFacts::new(&champ(vec![vec!["q1", "r1"]]), &sessions, roster());
    let [q, r] = &facts.rounds[0].sessions[..] else { panic!() };

    assert_eq!(q.player().unwrap().points, 0);
    assert_eq!(r.player().unwrap().points, 9);
    assert_eq!(r.results[1].points, 0, "a retirement scores nothing");
    assert_eq!(r.results[2].points, 0, "past the end of the table");
}

#[test]
fn test_the_team_and_the_replaced_driver_come_from_the_season() {
    let mut c = champ(vec![]);
    let facts = SeasonFacts::new(&c, &[], roster());
    assert_eq!(facts.player_team.as_deref(), Some("Osella"));
    assert_eq!(facts.replaced.as_deref(), Some("Piercarlo Ghinzani"));

    c.player_team = Some(String::new());
    c.player_seat = None;
    let facts = SeasonFacts::new(&c, &[], roster());
    assert_eq!(facts.player_team, None, "an empty team is no team");
    assert_eq!(facts.replaced, None);
}

#[test]
fn test_the_history_counts_only_seasons_begun_earlier() {
    use crate::career::story::test_support::{me, quali, race as race_of, season_for};
    let mut earlier = season_for(
        Some("Williams"),
        vec![],
        vec![vec![quali("q", vec![me(1, false)]), race_of("r", vec![me(1, false)])], vec![race_of("r", vec![me(3, true)])]],
    );
    earlier.champ_id = "1985".into();
    let mut this = season_for(Some("Osella"), vec![], vec![vec![race_of("r", vec![me(8, false)])]]);
    this.champ_id = "1986".into();
    let mut later = season_for(Some("Lotus"), vec![], vec![vec![race_of("r", vec![me(2, false)])]]);
    later.champ_id = "1987".into();
    // Recorded in that order.
    for (season, base) in [(&mut earlier, 100), (&mut this, 200), (&mut later, 300)] {
        for round in &mut season.rounds {
            for s in &mut round.sessions {
                s.recorded_at += base;
            }
        }
    }

    let history = PlayerHistory::before(&this, &[later.clone(), earlier.clone(), this.clone()]);
    assert_eq!(
        history,
        PlayerHistory {
            starts: 2,
            wins: 1,
            podiums: 1,
            poles: 1,
            scored: true,
            last_team: Some("Williams".into()),
        }
    );
    // A season with nothing recorded has no start to count from.
    let empty = season_for(Some("Osella"), vec![], vec![]);
    assert_eq!(PlayerHistory::before(&empty, &[earlier]), PlayerHistory::default());
}
