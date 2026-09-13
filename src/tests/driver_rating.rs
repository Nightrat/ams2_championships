use super::*;
use crate::custom_ai::parse_seats_str;
use crate::data_store::{RecordedSession, SessionResult};

const ROSTER: &str = r#"<custom_ai_drivers>
    <driver livery_name="1986 Williams #5 - N. Mansell"><name>Nigel Mansell</name><race_skill>0.98</race_skill></driver>
    <driver livery_name="1986 Williams #6 - N. Piquet"><name>Nelson Piquet</name><race_skill>0.98</race_skill></driver>
    <driver livery_name="1986 Brabham #7 - R. Patrese"><name>Riccardo Patrese</name><race_skill>0.81</race_skill></driver>
    <driver livery_name="1986 Brabham #8 - D. Warwick"><name>Derek Warwick</name><race_skill>0.78</race_skill></driver>
    <driver livery_name="1986 Osella #21 - P. Ghinzani"><name>Piercarlo Ghinzani</name><race_skill>0.66</race_skill></driver>
    <driver livery_name="1986 Osella #22 - A. Berg"><name>Allan Berg</name><race_skill>0.67</race_skill></driver>
</custom_ai_drivers>"#;

const M1: &str = "Formula Classic Gen1 Model1";

fn pace() -> HashMap<String, f32> {
    [("Williams", 0.0f32), ("Brabham", 0.85), ("Osella", 5.6)]
        .into_iter()
        .map(|(t, p)| (t.to_string(), p))
        .collect()
}

fn result(name: &str, pos: u32, laps: u32) -> SessionResult {
    SessionResult {
        name: name.into(),
        car_name: M1.into(),
        car_class: "F-Classic_Gen1".into(),
        race_position: pos,
        laps_completed: laps,
        fastest_lap: 90.0,
        last_lap: 90.0,
        dnf: false,
        is_player: false,
    }
}

/// A single-player grid where Brabham #7 is the free seat, so the player is a Brabham.
fn sp_session(
    id: &str,
    at: u64,
    session_type: u32,
    player_pos: u32,
    player_laps: u32,
) -> RecordedSession {
    let mut results = vec![
        result("Nigel Mansell", 1, 15),
        result("Nelson Piquet", 2, 15),
        result("Derek Warwick", 3, 15),
        result("Piercarlo Ghinzani", 4, 15),
        result("Allan Berg", 5, 15),
        result("Nightrat", player_pos, player_laps),
    ];
    // Positions must be distinct enough for the ordering to be meaningful; leave as authored.
    results.sort_by_key(|r| r.race_position);
    RecordedSession {
        id: id.into(),
        recorded_at: at,
        track: "Monza".into(),
        track_variation: String::new(),
        car_name: M1.into(),
        car_class: "F-Classic_Gen1".into(),
        session_type,
        results,
        lap_chart: vec![],
    }
}

fn mp_session(id: &str, at: u64, player_pos: u32, rival_pos: u32) -> RecordedSession {
    // Online, no human is in the roster, so the player can only be identified by the recorded
    // is_player flag (or a known name) — exactly as the live recorder writes it.
    let mut me = result("Nightrat", player_pos, 15);
    me.is_player = true;
    RecordedSession {
        id: id.into(),
        recorded_at: at,
        track: "Hockenheim".into(),
        track_variation: String::new(),
        car_name: M1.into(),
        car_class: "F-Classic_Gen1".into(),
        session_type: 5,
        results: vec![
            me,
            result("Wiper", rival_pos, 15),
            result("Sandro Martini  (AI)", 3, 15),
        ],
        lap_chart: vec![],
    }
}

#[test]
fn test_is_multiplayer_detects_ai_suffix() {
    assert!(is_multiplayer(&mp_session("m", 1, 1, 2)));
    assert!(!is_multiplayer(&sp_session("s", 1, 5, 1, 15)));
}

#[test]
fn test_retired_separates_retirement_from_lapped_finisher() {
    // Down a couple of laps is a lapped finisher, not a retirement.
    assert!(!retired(&result("x", 20, 14), 15));
    assert!(retired(&result("x", 20, 5), 15));
    // No leader laps recorded — nothing can be concluded.
    assert!(!retired(&result("x", 20, 0), 0));
}

#[test]
fn test_retired_does_not_fire_two_laps_down_on_a_sprint() {
    // A real 15-lap race: repairs cost two laps and the car still took the flag in P9. The bare
    // 10% test puts the line at 1.5 laps and calls that a retirement, which then costs both the
    // pace credit and a point of finish rate.
    assert!(!retired(&result("x", 9, 13), 15));
    // Three laps down on the same race is far enough back to read as a retirement.
    assert!(retired(&result("x", 12, 12), 15));
}

#[test]
fn test_retired_keeps_the_proportional_rule_on_long_races() {
    // Over 50 laps, 10% is 5 laps and the absolute floor never binds first.
    assert!(!retired(&result("x", 15, 46), 50));
    assert!(
        !retired(&result("x", 15, 45), 50),
        "exactly 90% is classified"
    );
    assert!(retired(&result("x", 20, 44), 50));
    // Three laps down is not enough on its own — it must also miss 90% of the distance.
    assert!(!retired(&result("x", 15, 47), 50));
}

#[test]
fn test_expected_positions_rank_teams_by_car_pace() {
    let seats = parse_seats_str(ROSTER);
    let exp = expected_positions(&pace(), &seats);
    // Williams holds seats 1 and 2, Brabham 3 and 4, Osella 5 and 6.
    assert_eq!(exp["Williams"], 1.5);
    assert_eq!(exp["Brabham"], 3.5);
    assert_eq!(exp["Osella"], 5.5);
}

#[test]
fn test_positional_score_does_not_saturate_across_cars() {
    // A win from an expected P21 must outrank a win from an expected P7 — the failure mode of
    // normalising by headroom, which caps both at +1.
    let slow_car_win = positional_score(21.5, 1.0, 26.0);
    let fast_car_win = positional_score(7.5, 1.0, 26.0);
    assert!(
        slow_car_win > fast_car_win,
        "{slow_car_win} vs {fast_car_win}"
    );
    assert!(slow_car_win <= 1.0 && fast_car_win > 0.0);
}

#[test]
fn test_reputation_rewards_beating_the_car() {
    let seats = parse_seats_str(ROSTER);
    // Brabham expects P3.5; winning every race is a clear overperformance.
    let winning: Vec<RecordedSession> = (0..8)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    let losing: Vec<RecordedSession> = (0..8)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 6, 15))
        .collect();
    let hi = compute_reputation(&winning, &seats, &pace(), Some("Brabham"));
    let lo = compute_reputation(&losing, &seats, &pace(), Some("Brabham"));
    assert!(hi.value > lo.value, "{} vs {}", hi.value, lo.value);
    assert!(hi.pace > 0.0 && lo.pace < 0.0);
    assert_eq!(hi.sp_races, 8);
}

#[test]
fn test_reputation_shrinks_toward_neutral_on_a_small_sample() {
    let seats = parse_seats_str(ROSTER);
    let one = vec![sp_session("a", 100, 5, 1, 15)];
    let many: Vec<RecordedSession> = (0..20)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    let a = compute_reputation(&one, &seats, &pace(), Some("Brabham"));
    let b = compute_reputation(&many, &seats, &pace(), Some("Brabham"));
    assert!(
        a.value < b.value,
        "one race must not rate as highly as twenty"
    );
}

#[test]
fn test_retirements_land_in_reliability_not_pace() {
    let seats = parse_seats_str(ROSTER);
    let mut sessions: Vec<RecordedSession> = (0..6)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    let clean = compute_reputation(&sessions, &seats, &pace(), Some("Brabham"));
    // Two retirements: starts rise, finishes do not, and pace is untouched.
    sessions.push(sp_session("r1", 200, 5, 6, 2));
    sessions.push(sp_session("r2", 201, 5, 6, 2));
    let with_dnf = compute_reputation(&sessions, &seats, &pace(), Some("Brabham"));
    assert_eq!(with_dnf.sp_races, 8);
    assert!(
        (with_dnf.pace - clean.pace).abs() < 0.001,
        "pace must ignore retirements"
    );
    assert!(with_dnf.finish_rate < clean.finish_rate);
    assert!(
        with_dnf.value < clean.value,
        "reliability still costs reputation"
    );
}

#[test]
fn test_multiplayer_scored_head_to_head_and_capped() {
    let seats = parse_seats_str(ROSTER);
    let sp: Vec<RecordedSession> = (0..8)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 3, 15))
        .collect();
    let baseline = compute_reputation(&sp, &seats, &pace(), Some("Brabham"));

    let mut with_mp = sp.clone();
    for i in 0..6 {
        with_mp.push(mp_session(&format!("m{i}"), 300 + i, 1, 2));
    }
    let won = compute_reputation(&with_mp, &seats, &pace(), Some("Brabham"));
    assert_eq!(won.mp_races, 6);
    assert_eq!(won.mp_wins, 6);
    assert!(won.value > baseline.value);
    // However dominant, online racing may not move the rating by more than the cap.
    assert!(
        (won.value - baseline.value) <= 5.0 + 0.001,
        "delta {}",
        won.value - baseline.value
    );
}

#[test]
fn test_multiplayer_ignores_lobby_ai_and_absolute_position() {
    let seats = parse_seats_str(ROSTER);
    // Finishing P8 overall but ahead of the only human rival is still a win: lobby AI run at an
    // unrecorded difficulty, so only the human comparison is trustworthy.
    let sessions = vec![mp_session("m", 300, 8, 9)];
    let r = compute_reputation(&sessions, &seats, &pace(), None);
    assert_eq!(r.mp_wins, 1);
    assert_eq!(r.mp_losses, 0);
}

/// A second class with its own roster and its own pace order.
const ROSTER_B: &str = r#"<custom_ai_drivers>
    <driver livery_name="1967 Lotus-Ford #5 - J. Clark"><name>Jim Clark</name><race_skill>0.97</race_skill></driver>
    <driver livery_name="1967 Lotus-Ford #6 - G. Hill"><name>Graham Hill</name><race_skill>0.92</race_skill></driver>
    <driver livery_name="1967 Honda #14 - J. Surtees"><name>John Surtees</name><race_skill>0.70</race_skill></driver>
</custom_ai_drivers>"#;

fn contexts() -> Vec<RatingContext> {
    let pace_b: HashMap<String, f32> = [("Lotus-Ford", 0.0f32), ("Honda", 5.0)]
        .into_iter()
        .map(|(t, p)| (t.into(), p))
        .collect();
    vec![
        RatingContext::new("F-Classic_Gen1", parse_seats_str(ROSTER), &pace()),
        RatingContext::new("F-Vintage_Gen1", parse_seats_str(ROSTER_B), &pace_b),
    ]
}

/// Session in class B where the free seat is Honda #14.
fn class_b_session(id: &str, at: u64, player_pos: u32) -> RecordedSession {
    let mut me = result("Nightrat", player_pos, 15);
    me.is_player = true;
    let mut s = RecordedSession {
        id: id.into(),
        recorded_at: at,
        track: "Kyalami".into(),
        track_variation: String::new(),
        car_name: "Lotus 49".into(),
        car_class: "F-Vintage_Gen1".into(),
        session_type: 5,
        results: vec![result("Jim Clark", 1, 15), result("Graham Hill", 2, 15), me],
        lap_chart: vec![],
    };
    s.results.sort_by_key(|r| r.race_position);
    s
}

#[test]
fn test_assigned_sessions_keeps_only_what_a_championship_claims() {
    use crate::data_store::{Championship, ChampionshipStatus, Round};
    let sessions: Vec<RecordedSession> = (0..4)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    let champ = Championship {
        id: "c1".into(),
        name: "Season".into(),
        status: ChampionshipStatus::Progress,
        points_system: vec![25, 18],
        manufacturer_scoring: false,
        rounds: vec![
            Round {
                session_ids: vec!["0".into()],
            },
            Round {
                session_ids: vec!["2".into()],
            },
        ],
        session_ids: vec![],
        custom_ai_file: None,
        player_team: None,
    };
    let kept = assigned_sessions(&[champ], &sessions);
    assert_eq!(
        kept.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        vec!["0", "2"]
    );
    // No championships at all means nothing is rateable — a fresh install starts from neutral.
    assert!(assigned_sessions(&[], &sessions).is_empty());
}

#[test]
fn test_rating_ignores_unassigned_sessions() {
    use crate::data_store::{Championship, ChampionshipStatus, Round};
    let ctxs = contexts();
    let winning: Vec<RecordedSession> = (0..6)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    // Six wins recorded, but only three claimed by a championship.
    let champ = Championship {
        id: "c1".into(),
        name: "Season".into(),
        status: ChampionshipStatus::Progress,
        points_system: vec![25],
        manufacturer_scoring: false,
        rounds: vec![Round {
            session_ids: vec!["0".into(), "1".into(), "2".into()],
        }],
        session_ids: vec![],
        custom_ai_file: None,
        player_team: None,
    };
    let rated = assigned_sessions(&[champ], &winning);
    let r = compute_reputation_global(Some("Nightrat"), &rated, &ctxs, None);
    assert_eq!(r.sp_races, 3, "unassigned races must not count");
    let all = compute_reputation_global(Some("Nightrat"), &winning, &ctxs, None);
    assert!(
        all.value > r.value,
        "shrinkage means the larger claimed sample rates higher"
    );
}

#[test]
fn test_global_rating_combines_classes() {
    let ctxs = contexts();
    let mut sessions: Vec<RecordedSession> = (0..6)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    let one_class = compute_reputation_global(Some("Nightrat"), &sessions, &ctxs, None);
    assert_eq!(one_class.sp_races, 6);

    // Races in a second class must count toward the same career rating.
    for i in 0..4 {
        sessions.push(class_b_session(&format!("b{i}"), 200 + i, 1));
    }
    let both = compute_reputation_global(Some("Nightrat"), &sessions, &ctxs, None);
    assert_eq!(both.sp_races, 10, "races from every class feed one rating");
    assert!(
        both.value > one_class.value,
        "more evidence of winning must not lower the rating"
    );
}

#[test]
fn test_global_rating_skips_classes_with_no_roster() {
    let ctxs = contexts();
    let mut sessions: Vec<RecordedSession> = (0..6)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    let before = compute_reputation_global(Some("Nightrat"), &sessions, &ctxs, None);
    // F-Junior has no Custom AI file, so there is no expectation to score it against.
    let mut orphan = class_b_session("j", 300, 1);
    orphan.car_class = "F-Junior".into();
    sessions.push(orphan);
    let after = compute_reputation_global(Some("Nightrat"), &sessions, &ctxs, None);
    assert_eq!(after.sp_races, before.sp_races);
}

#[test]
fn test_recorded_players_global_excludes_every_roster_and_lobby_ai() {
    let ctxs = contexts();
    let mut sessions = vec![sp_session("a", 100, 5, 1, 15), class_b_session("b", 200, 1)];
    sessions.push(mp_session("m", 300, 1, 2));
    let players = recorded_players_global(&sessions, &ctxs);
    // Drivers from either roster, and "(AI)" lobby fill-ins, are not players.
    assert_eq!(players, vec!["Nightrat".to_string(), "Wiper".to_string()]);
}

// ── Reference career ─────────────────────────────────────────────────────────
//
// `fixtures/career_reference.json` is a real recorded career (72 sessions, 5 championships)
// kept so the rating can be exercised against genuine data rather than only hand-built grids.
// It pairs with the Custom AI files already committed under `docs/`, which are byte-identical
// to the ones AMS2 ships, so these tests need no game install and run on CI.
//
// The expected values below are a snapshot of current behaviour, not a specification. Changing
// the rating maths is *supposed* to move them — update them deliberately when it does.

fn reference_career() -> crate::data_store::CareerData {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tests/fixtures/career_reference.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture missing")).unwrap()
}

fn reference_contexts() -> Vec<RatingContext> {
    let dir = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/custom_ai_files_with_perf_scalars"
    ));
    crate::custom_ai::class_performance(dir)
        .into_iter()
        .map(|perf| {
            let path = dir.join(format!("{}.xml", perf.class));
            let pace: HashMap<String, f32> = perf
                .cars
                .iter()
                .map(|c| (c.team.clone(), c.pace_delta_pct))
                .collect();
            RatingContext::new(&perf.class, crate::custom_ai::parse_seats(&path), &pace)
        })
        .collect()
}

/// The fixture's sessions, filtered exactly as the server filters them.
fn reference_rated() -> Vec<RecordedSession> {
    let data = reference_career();
    assigned_sessions(&data.championships, &data.sessions)
}

#[test]
fn test_reference_career_is_fully_assigned() {
    let data = reference_career();
    // The snapshots below only mean something while every session is claimed by a championship.
    assert_eq!(
        reference_rated().len(),
        data.sessions.len(),
        "fixture should be fully assigned"
    );
    assert_eq!(data.championships.len(), 5);
}

#[test]
fn test_reference_career_finds_only_the_human_drivers() {
    let players = recorded_players_global(&reference_rated(), &reference_contexts());
    // Every AI is either a roster entry or carries the "(AI)" lobby marker.
    assert_eq!(players, vec!["Nightrat".to_string(), "Wiper".to_string()]);
}

#[test]
fn test_reference_career_rating_snapshot() {
    let ctxs = reference_contexts();
    let r = compute_reputation_global(Some("Nightrat"), &reference_rated(), &ctxs, None);

    assert_eq!(r.sp_races, 24, "race starts");
    assert_eq!(r.mp_races, 7, "online races");
    assert_eq!((r.mp_wins, r.mp_losses), (2, 5), "online record");
    // 7 of 24 starts ended in a retirement.
    assert!(
        (r.finish_rate - 17.0 / 24.0).abs() < 0.001,
        "finish_rate {}",
        r.finish_rate
    );
    assert!(r.pace > 0.0 && r.quali > 0.0, "beat the car on average");
    assert!(
        r.mp_bonus < 0.0,
        "a losing online record must cost, not pay"
    );
    // Beat the team-mate in 34 of 38 comparisons, which is worth most of the available bonus.
    assert_eq!((r.mate_wins, r.mate_losses), (34, 4), "team-mate record");
    assert!(r.teammate > 0.7, "head-to-head {}", r.teammate);
    assert!(
        (r.teammate_bonus - 5.50).abs() < 0.1,
        "teammate_bonus {}",
        r.teammate_bonus
    );
    // Snapshot value shifts whenever docs/custom_ai_files_with_perf_scalars/*.xml's scalars change
    // (power_scalar was rescaled so the fastest car in each class is 1.00, not up to 1.10).
    assert!((r.value - 57.16).abs() < 0.5, "rating {}", r.value);
}

#[test]
fn test_reference_career_skips_classes_without_a_roster() {
    let rated = reference_rated();
    let ctxs = reference_contexts();
    // The career contains F-Junior sessions, and no F-Junior Custom AI file exists, so they
    // cannot be scored — they must not silently count as races.
    assert!(
        rated.iter().any(|s| s.car_class == "F-Junior"),
        "fixture should still contain the unrateable class"
    );
    assert!(!ctxs.iter().any(|c| c.class == "F-Junior"));
    let all = compute_reputation_global(Some("Nightrat"), &rated, &ctxs, None);
    let without: Vec<_> = rated
        .iter()
        .filter(|s| s.car_class != "F-Junior")
        .cloned()
        .collect();
    let trimmed = compute_reputation_global(Some("Nightrat"), &without, &ctxs, None);
    assert_eq!(all.sp_races, trimmed.sp_races);
    assert!((all.value - trimmed.value).abs() < 0.001);
}

#[test]
fn test_reference_career_second_human_is_online_only() {
    let rated = reference_rated();
    let r = compute_reputation_global(Some("Wiper"), &rated, &reference_contexts(), None);
    // Wiper appears only in multiplayer, so there is no car-relative pace to rate — the value
    // is the neutral 50 plus his online record, which mirrors Nightrat's exactly.
    assert_eq!(r.sp_races, 0);
    assert_eq!((r.mp_wins, r.mp_losses), (5, 2));
    let me = compute_reputation_global(Some("Nightrat"), &rated, &reference_contexts(), None);
    assert!(
        (r.mp_bonus + me.mp_bonus).abs() < 0.001,
        "head-to-head must be zero-sum"
    );
    assert!((r.value - (50.0 + r.mp_bonus)).abs() < 0.001);
}

#[test]
fn test_eligibility_locks_top_teams_and_opens_the_back() {
    let seats = parse_seats_str(ROSTER);
    let exp = expected_positions(&pace(), &seats);
    let skills = crate::custom_ai::parse_team_skills_str(ROSTER);
    let low = team_eligibility(40.0, &exp, &skills);
    let williams = low.iter().find(|e| e.team == "Williams").unwrap();
    let osella = low.iter().find(|e| e.team == "Osella").unwrap();
    assert_eq!(williams.tier, Tier::Locked);
    // Osella's own bar (Ghinzani at 0.66) is above a 40 rating, but the slowest team is always
    // open so a new driver has somewhere to start.
    assert_eq!(osella.tier, Tier::Available);
    assert!(
        osella.required > 40.0,
        "the floor should override the bar, not lower it"
    );
    // The bar is the weaker incumbent: Brabham asks for Warwick's 0.78, not Patrese's 0.81.
    let brabham = low.iter().find(|e| e.team == "Brabham").unwrap();
    assert!((brabham.incumbent_skill.unwrap() - 0.78).abs() < 0.001);

    // A top rating opens everything.
    let high = team_eligibility(99.0, &exp, &skills);
    assert!(high.iter().all(|e| e.tier == Tier::Available));
}

#[test]
fn test_is_allowed_permits_unknown_teams() {
    let seats = parse_seats_str(ROSTER);
    let exp = expected_positions(&pace(), &seats);
    let skills = crate::custom_ai::parse_team_skills_str(ROSTER);
    let e = team_eligibility(40.0, &exp, &skills);
    assert!(!is_allowed(&e, "Williams"));
    assert!(is_allowed(&e, "Osella"));
    // Case-insensitive, and a team with no pace data must never be blocked.
    assert!(is_allowed(&e, "osella"));
    assert!(is_allowed(&e, "Some Team We Know Nothing About"));
}

// ── Phantom seats and the expected-position model ─────────────────────────────

#[test]
fn test_expected_positions_shrink_when_phantom_seats_are_removed() {
    use crate::custom_ai::without_phantom_seats;

    let seats = parse_seats_str(ROSTER);
    let pace = pace();

    // Every seat counted: 6 cars, so Osella's two sit at P5 and P6, a mean of 5.5.
    let all = expected_positions(&pace, &seats);
    assert_eq!(all.get("Williams"), Some(&1.5));
    assert_eq!(all.get("Brabham"), Some(&3.5));
    assert_eq!(all.get("Osella"), Some(&5.5));

    // AMS2 has no livery for Brabham #8, so that car never starts. The field is 5, and every
    // seat behind the gap moves up — Osella now expects P4.5, not P5.5.
    let installed: std::collections::HashSet<String> = seats
        .iter()
        .map(|s| s.livery.clone())
        .filter(|l| !l.contains("D. Warwick"))
        .collect();
    let real = without_phantom_seats(seats, Some(&installed));
    assert_eq!(real.len(), 5);

    let trimmed = expected_positions(&pace, &real);
    assert_eq!(trimmed.get("Williams"), Some(&1.5), "the front is unmoved");
    assert_eq!(
        trimmed.get("Brabham"),
        Some(&3.0),
        "Brabham is now a one-car team at P3, not a two-car team averaging P3.5"
    );
    assert_eq!(
        trimmed.get("Osella"),
        Some(&4.5),
        "a phantom ahead of you inflates what your car is expected to beat"
    );
}

#[test]
fn test_expected_positions_unchanged_when_liveries_cannot_be_verified() {
    use crate::custom_ai::without_phantom_seats;

    let seats = parse_seats_str(ROSTER);
    let before = expected_positions(&pace(), &seats);
    // No manifests readable: ratings must stay exactly as they were rather than shift on a guess.
    let after = expected_positions(&pace(), &without_phantom_seats(seats, None));
    assert_eq!(before, after);
}

// ── Team-mate head-to-head ────────────────────────────────────────────────────
//
// In `sp_session` the player is the free Brabham #7 seat and Derek Warwick holds Brabham #8 in
// P3, so the player's position relative to 3 decides each head-to-head. Identical car, so this
// is the one signal in the rating that owes nothing to the car-pace estimate.

#[test]
fn test_beating_the_teammate_raises_the_rating() {
    let seats = parse_seats_str(ROSTER);
    // Same finishing position in both careers — only the team-mate comparison differs, because
    // in one the player is ahead of Warwick (P3) and in the other behind.
    let ahead: Vec<RecordedSession> = (0..8)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 2, 15))
        .collect();
    let behind: Vec<RecordedSession> = (0..8)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 4, 15))
        .collect();

    let hi = compute_reputation(&ahead, &seats, &pace(), Some("Brabham"));
    let lo = compute_reputation(&behind, &seats, &pace(), Some("Brabham"));
    assert_eq!((hi.mate_wins, hi.mate_losses), (8, 0));
    assert_eq!((lo.mate_wins, lo.mate_losses), (0, 8));
    assert!(hi.teammate_bonus > 0.0 && lo.teammate_bonus < 0.0);
    assert!(hi.value > lo.value, "{} vs {}", hi.value, lo.value);
}

#[test]
fn test_qualifying_head_to_head_counts_too() {
    let seats = parse_seats_str(ROSTER);
    // Qualifying only: out-qualifying the team-mate every time must still pay.
    let sessions: Vec<RecordedSession> = (0..8)
        .map(|i| sp_session(&i.to_string(), 100 + i, 3, 2, 15))
        .collect();
    let r = compute_reputation(&sessions, &seats, &pace(), Some("Brabham"));
    assert_eq!((r.mate_wins, r.mate_losses), (8, 0));
    assert!(r.teammate_bonus > 0.0, "{}", r.teammate_bonus);
}

#[test]
fn test_teammate_bonus_is_capped_and_damped_on_a_thin_record() {
    let seats = parse_seats_str(ROSTER);
    let one = vec![sp_session("a", 100, 5, 1, 15)];
    let many: Vec<RecordedSession> = (0..30)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    let a = compute_reputation(&one, &seats, &pace(), Some("Brabham"));
    let b = compute_reputation(&many, &seats, &pace(), Some("Brabham"));
    assert!(
        a.teammate_bonus < b.teammate_bonus,
        "one comparison must not pay like thirty: {} vs {}",
        a.teammate_bonus,
        b.teammate_bonus
    );
    // Even a perfect record cannot outweigh the rest of the rating.
    assert!(b.teammate_bonus <= 8.0 + 0.001, "{}", b.teammate_bonus);
}

#[test]
fn test_no_teammate_means_no_bonus_either_way() {
    // A one-car team has nobody to measure against, so the term must sit out rather than
    // scoring the player as if they had lost.
    let roster = r#"<custom_ai_drivers>
        <driver livery_name="1986 Williams #5 - N. Mansell"><name>Nigel Mansell</name></driver>
        <driver livery_name="1986 Brabham #7 - R. Patrese"><name>Riccardo Patrese</name></driver>
    </custom_ai_drivers>"#;
    let seats = parse_seats_str(roster);
    let mut s = sp_session("a", 100, 5, 2, 15);
    // Only Mansell and the player on the grid: the player is the free Brabham, alone.
    s.results
        .retain(|r| r.name == "Nigel Mansell" || r.name == "Nightrat");
    let r = compute_reputation(&[s], &seats, &pace(), Some("Brabham"));
    assert_eq!((r.mate_wins, r.mate_losses), (0, 0));
    assert_eq!(r.teammate_bonus, 0.0);
}

#[test]
fn test_a_retired_teammate_is_not_counted_as_beaten() {
    let seats = parse_seats_str(ROSTER);
    // Warwick stops on lap 2 of 15. Finishing ahead of a parked car says nothing about pace,
    // and reliability already has its own term.
    let mut s = sp_session("a", 100, 5, 2, 15);
    for r in s.results.iter_mut() {
        if r.name == "Derek Warwick" {
            r.laps_completed = 2;
        }
    }
    let r = compute_reputation(&[s], &seats, &pace(), Some("Brabham"));
    assert_eq!((r.mate_wins, r.mate_losses), (0, 0));
    assert_eq!(r.teammate_bonus, 0.0);
}

// ── Configurable tuning (`RatingParams`) ─────────────────────────────────────
//
// These rest on one guarantee: `RatingParams::default()` reproduces the behaviour these numbers
// were hard-coded to. Every test above calls the wrappers that use it, and
// `test_reference_career_rating_snapshot` pins real figures against a recorded career.

fn params(f: impl FnOnce(&mut RatingParams)) -> RatingParams {
    let mut p = RatingParams::default();
    f(&mut p);
    p
}

#[test]
fn test_starting_rating_anchors_a_driver_with_no_results() {
    let seats = parse_seats_str(ROSTER);
    let p = params(|p| p.starting_rating = 20.0);
    let r = compute_reputation_with(&[], &seats, &pace(), Some("Brabham"), &p);
    assert!(
        (r.value - 20.0).abs() < 0.001,
        "an unproven driver sits exactly where the config puts them, got {}",
        r.value
    );
    // The default is the midpoint the per-session scores are expressed around, as before.
    let d = compute_reputation(&[], &seats, &pace(), Some("Brabham"));
    assert!((d.value - 50.0).abs() < 0.001);
}

#[test]
fn test_starting_rating_washes_out_as_races_accumulate() {
    let seats = parse_seats_str(ROSTER);
    let low = params(|p| p.starting_rating = 20.0);
    // Brabham expects about P3 and the player wins every time, so the evidence is strongly
    // positive and must eventually outweigh where they started.
    let one: Vec<RecordedSession> = vec![sp_session("a", 100, 5, 1, 15)];
    let many: Vec<RecordedSession> = (0..40)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();

    let gap_thin = compute_reputation(&one, &seats, &pace(), Some("Brabham")).value
        - compute_reputation_with(&one, &seats, &pace(), Some("Brabham"), &low).value;
    let gap_thick = compute_reputation(&many, &seats, &pace(), Some("Brabham")).value
        - compute_reputation_with(&many, &seats, &pace(), Some("Brabham"), &low).value;

    assert!(gap_thin > 20.0, "one race leaves the prior dominant");
    assert!(
        gap_thick < 4.0,
        "forty races should have all but erased it, gap was {gap_thick}"
    );
}

#[test]
fn test_strictness_shifts_every_requirement_by_the_same_amount() {
    let seats = parse_seats_str(ROSTER);
    let exp = expected_positions(&pace(), &seats);
    let skills = crate::custom_ai::parse_team_skills_str(ROSTER);

    let base = team_requirements(&exp, &skills);
    let easier = team_requirements_with(&params(|p| p.strictness = -15.0), &exp, &skills);
    for ((team, was), (team2, now)) in base.iter().zip(easier.iter()) {
        assert_eq!(team, team2, "ordering must not change");
        // Clamped at zero, so a team already asking for nothing cannot go negative.
        assert!(
            (now - (was - 15.0).max(0.0)).abs() < 0.001,
            "{team}: {was} -> {now}"
        );
    }
}

#[test]
fn test_strictness_opens_seats_the_default_locks() {
    let seats = parse_seats_str(ROSTER);
    let exp = expected_positions(&pace(), &seats);
    let skills = crate::custom_ai::parse_team_skills_str(ROSTER);

    // Brabham's bar is Warwick's 0.78, so 73, out of reach at 60.
    assert_eq!(
        team_eligibility(60.0, &exp, &skills)
            .iter()
            .find(|e| e.team == "Brabham")
            .unwrap()
            .tier,
        Tier::Locked
    );
    let lenient = team_eligibility_with(&params(|p| p.strictness = -20.0), 60.0, &exp, &skills);
    assert_eq!(
        lenient.iter().find(|e| e.team == "Brabham").unwrap().tier,
        Tier::Available
    );
    // The top seat asks 93 by incumbent skill, so 20 points off is not enough to hand it over.
    assert_eq!(
        lenient.iter().find(|e| e.team == "Williams").unwrap().tier,
        Tier::Locked
    );
}

#[test]
fn test_grid_gate_alone_ignores_incumbent_skill() {
    let seats = parse_seats_str(ROSTER);
    let exp = expected_positions(&pace(), &seats);
    let skills = crate::custom_ai::parse_team_skills_str(ROSTER);

    let grid: HashMap<String, f32> =
        team_requirements_with(&params(|p| p.gates = Gates::Grid), &exp, &skills)
            .into_iter()
            .collect();
    // Slowest of three teams: nothing asked of the driver, where Ghinzani's 0.66 asked 61.
    assert!(grid["Osella"] < 0.001, "got {}", grid["Osella"]);
    assert!((grid["Brabham"] - 100.0 / 3.0).abs() < 0.5);
}

#[test]
fn test_incumbent_gate_alone_ignores_car_pace() {
    let seats = parse_seats_str(ROSTER);
    let exp = expected_positions(&pace(), &seats);
    let skills = crate::custom_ai::parse_team_skills_str(ROSTER);

    let inc: HashMap<String, f32> =
        team_requirements_with(&params(|p| p.gates = Gates::Incumbent), &exp, &skills)
            .into_iter()
            .collect();
    // Ghinzani's 0.66 less the margin: the slowest car on the grid still asks for 61.
    assert!((inc["Osella"] - 61.0).abs() < 0.5, "got {}", inc["Osella"]);
    assert!((inc["Williams"] - 93.0).abs() < 0.5);
}

#[test]
fn test_incumbent_gate_leaves_a_skill_less_roster_wide_open() {
    const NO_SKILLS: &str = r#"<custom_ai_drivers>
    <driver livery_name="1986 Williams #5 - N. Mansell"><name>Nigel Mansell</name></driver>
    <driver livery_name="1986 Osella #21 - P. Ghinzani"><name>Piercarlo Ghinzani</name></driver>
</custom_ai_drivers>"#;
    let seats = parse_seats_str(NO_SKILLS);
    let exp = expected_positions(&pace(), &seats);
    let skills = crate::custom_ai::parse_team_skills_str(NO_SKILLS);
    assert!(skills.is_empty(), "fixture must declare no race_skill");

    // Documented consequence: with no bar left to clear, every seat is free. The Config hint
    // says so, which is why this is a test rather than a bug.
    let e = team_eligibility_with(&params(|p| p.gates = Gates::Incumbent), 0.0, &exp, &skills);
    assert!(e.iter().all(|t| t.tier == Tier::Available));
}

#[test]
fn test_zero_half_life_weighs_the_whole_career_equally() {
    let seats = parse_seats_str(ROSTER);
    // Six strong races long ago, six weak ones recently. Brabham expects about P3.
    let mut sessions: Vec<RecordedSession> = (0..6)
        .map(|i| sp_session(&format!("old{i}"), 100 + i, 5, 1, 15))
        .collect();
    sessions.extend((0..6).map(|i| sp_session(&format!("new{i}"), 200 + i, 5, 6, 15)));

    let decayed = compute_reputation(&sessions, &seats, &pace(), Some("Brabham"));
    let flat = compute_reputation_with(
        &sessions,
        &seats,
        &pace(),
        Some("Brabham"),
        &params(|p| p.recency_half_life = 0.0),
    );
    assert!(
        flat.value > decayed.value,
        "recent bad form must count for less without decay: {} vs {}",
        flat.value,
        decayed.value
    );
    // A zero half-life must not divide its way to NaN.
    assert!(flat.value.is_finite());
}

#[test]
fn test_retirements_can_be_excluded_entirely() {
    let seats = parse_seats_str(ROSTER);
    let mut sessions: Vec<RecordedSession> = (0..6)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    let ignore_dnf = params(|p| p.count_retirements = false);
    let clean = compute_reputation_with(&sessions, &seats, &pace(), Some("Brabham"), &ignore_dnf);

    sessions.push(sp_session("r1", 200, 5, 6, 2));
    sessions.push(sp_session("r2", 201, 5, 6, 2));
    let with_dnf =
        compute_reputation_with(&sessions, &seats, &pace(), Some("Brabham"), &ignore_dnf);

    assert_eq!(with_dnf.sp_races, 6, "a skipped retirement is not a start");
    assert!((with_dnf.finish_rate - clean.finish_rate).abs() < 0.001);
    assert!(
        (with_dnf.value - clean.value).abs() < 0.001,
        "retirements must cost nothing when switched off"
    );
    // And the default still charges for them, so the switch is doing the work.
    let counted = compute_reputation(&sessions, &seats, &pace(), Some("Brabham"));
    assert_eq!(counted.sp_races, 8);
    assert!(counted.value < with_dnf.value);
}

#[test]
fn test_retirement_threshold_is_configurable() {
    // Two laps down over 15 is a lapped finisher by default, and both tests must still agree:
    // 13 of 15 laps is under 90%, so lowering the threshold to 2 is what flips it.
    let r = result("x", 12, 13);
    assert!(!retired(&r, 15));
    assert!(retired_with(
        &r,
        15,
        &params(|p| p.retirement_min_laps_down = 2)
    ));

    // Raising it past the gap makes an obvious retirement read as a finish, which is the point
    // for long races: six laps down over 50 is a bad afternoon, not a DNF.
    let long = result("x", 20, 44);
    assert!(retired(&long, 50));
    assert!(!retired_with(
        &long,
        50,
        &params(|p| p.retirement_min_laps_down = 7)
    ));
}

#[test]
fn test_zero_threshold_leaves_the_distance_test_to_decide() {
    // With no lap floor the 90% rule stands alone — and still protects a classified finisher.
    let no_floor = params(|p| p.retirement_min_laps_down = 0);
    assert!(
        retired_with(&result("x", 20, 13), 15, &no_floor),
        "13/15 is under 90%"
    );
    assert!(
        !retired_with(&result("x", 12, 14), 15, &no_floor),
        "14/15 is over 90%, so never a retirement whatever the floor"
    );
    assert!(
        !retired_with(&result("x", 20, 0), 0, &no_floor),
        "no leader, no verdict"
    );
}

#[test]
fn test_raising_the_threshold_reclassifies_a_dnf_as_a_finish() {
    let seats = parse_seats_str(ROSTER);
    let mut sessions: Vec<RecordedSession> = (0..6)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    // Four laps down of fifteen: a retirement by default, a lapped finisher at a floor of five.
    sessions.push(sp_session("late", 200, 5, 6, 11));

    let strict = compute_reputation(&sessions, &seats, &pace(), Some("Brabham"));
    let lenient = compute_reputation_with(
        &sessions,
        &seats,
        &pace(),
        Some("Brabham"),
        &params(|p| p.retirement_min_laps_down = 5),
    );

    assert_eq!(
        (strict.sp_races, lenient.sp_races),
        (7, 7),
        "a start either way"
    );
    assert!(
        (strict.finish_rate - 6.0 / 7.0).abs() < 0.001,
        "the default calls it a retirement"
    );
    assert!(
        (lenient.finish_rate - 1.0).abs() < 0.001,
        "raising the floor makes it a classified finish"
    );
    // And as a finish it now contributes race pace, where a retirement contributed none.
    assert!(
        lenient.pace < strict.pace,
        "a P6 finish drags the pace average down"
    );
}

#[test]
fn test_retirement_distance_is_configurable() {
    // 45 of 50 laps is exactly 90%, so the default calls it a finish — the test is strict.
    let borderline = result("x", 15, 45);
    assert!(!retired(&borderline, 50));
    // Demanding 95% of the distance instead turns the same result into a retirement.
    assert!(retired_with(
        &borderline,
        50,
        &params(|p| p.retirement_distance = 0.95)
    ));

    // And loosening it the other way rescues a car that is well down but still running.
    let well_down = result("x", 20, 40);
    assert!(retired(&well_down, 50), "80% is a retirement by default");
    assert!(!retired_with(
        &well_down,
        50,
        &params(|p| p.retirement_distance = 0.75)
    ));
}

#[test]
fn test_both_retirement_thresholds_must_still_agree() {
    // Two laps down of fifteen: inside the default lap floor, outside 90% of the distance.
    let r = result("x", 12, 13);
    assert!(!retired(&r, 15), "the lap floor alone keeps this a finish");
    // Loosening only the distance changes nothing — the lap test still says no.
    assert!(!retired_with(
        &r,
        15,
        &params(|p| p.retirement_distance = 0.99)
    ));
    // Both have to move.
    assert!(retired_with(
        &r,
        15,
        &params(|p| {
            p.retirement_distance = 0.99;
            p.retirement_min_laps_down = 2;
        })
    ));
}

#[test]
fn test_zero_distance_means_nothing_is_ever_a_retirement() {
    let off = params(|p| p.retirement_distance = 0.0);
    // Not even a car that completed no laps at all: `0 < 0.0 * 50` is false.
    assert!(!retired_with(&result("x", 20, 0), 50, &off));
    assert!(!retired_with(&result("x", 20, 1), 50, &off));
}

#[test]
fn test_distance_threshold_feeds_through_to_the_rating() {
    let seats = parse_seats_str(ROSTER);
    let mut sessions: Vec<RecordedSession> = (0..6)
        .map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15))
        .collect();
    // Four laps down of fifteen — 73% — a retirement under both defaults.
    sessions.push(sp_session("late", 200, 5, 6, 11));

    let strict = compute_reputation(&sessions, &seats, &pace(), Some("Brabham"));
    let lenient = compute_reputation_with(
        &sessions,
        &seats,
        &pace(),
        Some("Brabham"),
        &params(|p| p.retirement_distance = 0.7),
    );
    assert!((strict.finish_rate - 6.0 / 7.0).abs() < 0.001);
    assert!(
        (lenient.finish_rate - 1.0).abs() < 0.001,
        "73% clears a 70% bar, so it is a classified finish"
    );
}

// ── The offer margin ─────────────────────────────────────────────────────────

/// A three-team grid where the player sits 5 points under the middle team's bar.
fn margin_grid() -> (HashMap<String, f32>, HashMap<String, f32>) {
    let expected: HashMap<String, f32> = [("Fast", 1.5f32), ("Mid", 3.5), ("Slow", 5.5)]
        .into_iter()
        .map(|(t, p)| (t.to_string(), p))
        .collect();
    let skills: HashMap<String, f32> = [("Fast", 0.95f32), ("Mid", 0.60), ("Slow", 0.30)]
        .into_iter()
        .map(|(t, s)| (t.to_string(), s))
        .collect();
    (expected, skills)
}

fn tier_of(margin: f32, team: &str) -> Tier {
    let (expected, skills) = margin_grid();
    let params = RatingParams {
        offer_margin: margin,
        ..RatingParams::default()
    };
    // Mid's bar is max(grid 33.3, incumbent 55) = 55; the driver is 5 short of it.
    team_eligibility_with(&params, 50.0, &expected, &skills)
        .into_iter()
        .find(|e| e.team == team)
        .expect("team on the grid")
        .tier
}

#[test]
fn test_the_default_margin_reproduces_the_hard_coded_behaviour() {
    assert_eq!(RatingParams::default().offer_margin, OFFER_MARGIN);
}

#[test]
fn test_a_wider_margin_brings_a_locked_seat_within_reach() {
    // Five points short of Mid's bar of 55.
    assert_eq!(tier_of(10.0, "Mid"), Tier::OfferPossible);
    assert_eq!(tier_of(4.0, "Mid"), Tier::Locked, "narrower than the gap");
    assert_eq!(
        tier_of(5.0, "Mid"),
        Tier::OfferPossible,
        "exactly the gap counts"
    );
}

#[test]
fn test_a_zero_margin_means_every_bar_must_be_cleared_outright() {
    // The middle tier disappears: a seat is earned or it is locked, nothing between.
    assert_eq!(tier_of(0.0, "Mid"), Tier::Locked);
    // Slow's bar is max(grid 0, incumbent 25) = 25, which the driver clears, so it stays open —
    // a zero margin removes the tier, it does not close the grid.
    assert_eq!(tier_of(0.0, "Slow"), Tier::Available);
}

#[test]
fn test_a_margin_wide_enough_reaches_the_whole_grid() {
    // Fast asks 90; at a margin of 100 nothing on the grid is out of reach.
    assert_eq!(tier_of(100.0, "Fast"), Tier::OfferPossible);
    assert_eq!(tier_of(10.0, "Fast"), Tier::Locked);
}

#[test]
fn test_the_margin_moves_how_far_short_is_looked_at_not_the_bar() {
    // The difference from `strictness`: requirements are untouched, only the tier moves.
    let (expected, skills) = margin_grid();
    // Wide enough to cross a boundary: Fast asks 90 and the driver is 40 short of it.
    let wide = RatingParams {
        offer_margin: 45.0,
        ..RatingParams::default()
    };
    let a = team_eligibility_with(&RatingParams::default(), 50.0, &expected, &skills);
    let b = team_eligibility_with(&wide, 50.0, &expected, &skills);
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.required, y.required, "{} moved its bar", x.team);
    }
    assert_ne!(
        a.iter().map(|e| e.tier).collect::<Vec<_>>(),
        b.iter().map(|e| e.tier).collect::<Vec<_>>(),
        "but the tiers must have moved"
    );
}
