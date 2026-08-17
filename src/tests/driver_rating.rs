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
fn sp_session(id: &str, at: u64, session_type: u32, player_pos: u32, player_laps: u32) -> RecordedSession {
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
        results: vec![me, result("Wiper", rival_pos, 15), result("Sandro Martini  (AI)", 3, 15)],
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
    assert!(slow_car_win > fast_car_win, "{slow_car_win} vs {fast_car_win}");
    assert!(slow_car_win <= 1.0 && fast_car_win > 0.0);
}

#[test]
fn test_reputation_rewards_beating_the_car() {
    let seats = parse_seats_str(ROSTER);
    // Brabham expects P3.5; winning every race is a clear overperformance.
    let winning: Vec<RecordedSession> =
        (0..8).map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15)).collect();
    let losing: Vec<RecordedSession> =
        (0..8).map(|i| sp_session(&i.to_string(), 100 + i, 5, 6, 15)).collect();
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
    let many: Vec<RecordedSession> =
        (0..20).map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15)).collect();
    let a = compute_reputation(&one, &seats, &pace(), Some("Brabham"));
    let b = compute_reputation(&many, &seats, &pace(), Some("Brabham"));
    assert!(a.value < b.value, "one race must not rate as highly as twenty");
}

#[test]
fn test_retirements_land_in_reliability_not_pace() {
    let seats = parse_seats_str(ROSTER);
    let mut sessions: Vec<RecordedSession> =
        (0..6).map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15)).collect();
    let clean = compute_reputation(&sessions, &seats, &pace(), Some("Brabham"));
    // Two retirements: starts rise, finishes do not, and pace is untouched.
    sessions.push(sp_session("r1", 200, 5, 6, 2));
    sessions.push(sp_session("r2", 201, 5, 6, 2));
    let with_dnf = compute_reputation(&sessions, &seats, &pace(), Some("Brabham"));
    assert_eq!(with_dnf.sp_races, 8);
    assert!((with_dnf.pace - clean.pace).abs() < 0.001, "pace must ignore retirements");
    assert!(with_dnf.finish_rate < clean.finish_rate);
    assert!(with_dnf.value < clean.value, "reliability still costs reputation");
}

#[test]
fn test_multiplayer_scored_head_to_head_and_capped() {
    let seats = parse_seats_str(ROSTER);
    let sp: Vec<RecordedSession> =
        (0..8).map(|i| sp_session(&i.to_string(), 100 + i, 5, 3, 15)).collect();
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
    assert!((won.value - baseline.value) <= 5.0 + 0.001, "delta {}", won.value - baseline.value);
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
    let pace_b: HashMap<String, f32> =
        [("Lotus-Ford", 0.0f32), ("Honda", 5.0)].into_iter().map(|(t, p)| (t.into(), p)).collect();
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
fn test_global_rating_combines_classes() {
    let ctxs = contexts();
    let mut sessions: Vec<RecordedSession> =
        (0..6).map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15)).collect();
    let one_class = compute_reputation_global(Some("Nightrat"), &sessions, &ctxs, None);
    assert_eq!(one_class.sp_races, 6);

    // Races in a second class must count toward the same career rating.
    for i in 0..4 {
        sessions.push(class_b_session(&format!("b{i}"), 200 + i, 1));
    }
    let both = compute_reputation_global(Some("Nightrat"), &sessions, &ctxs, None);
    assert_eq!(both.sp_races, 10, "races from every class feed one rating");
    assert!(both.value > one_class.value, "more evidence of winning must not lower the rating");
}

#[test]
fn test_global_rating_skips_classes_with_no_roster() {
    let ctxs = contexts();
    let mut sessions: Vec<RecordedSession> =
        (0..6).map(|i| sp_session(&i.to_string(), 100 + i, 5, 1, 15)).collect();
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
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/fixtures/career_reference.json");
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
            let pace: HashMap<String, f32> =
                perf.cars.iter().map(|c| (c.team.clone(), c.pace_delta_pct)).collect();
            RatingContext::new(&perf.class, crate::custom_ai::parse_seats(&path), &pace)
        })
        .collect()
}

#[test]
fn test_reference_career_finds_only_the_human_drivers() {
    let data = reference_career();
    let players = recorded_players_global(&data.sessions, &reference_contexts());
    // Every AI is either a roster entry or carries the "(AI)" lobby marker.
    assert_eq!(players, vec!["Nightrat".to_string(), "Wiper".to_string()]);
}

#[test]
fn test_reference_career_rating_snapshot() {
    let data = reference_career();
    let ctxs = reference_contexts();
    let r = compute_reputation_global(Some("Nightrat"), &data.sessions, &ctxs, None);

    assert_eq!(r.sp_races, 24, "race starts");
    assert_eq!(r.mp_races, 7, "online races");
    assert_eq!((r.mp_wins, r.mp_losses), (2, 5), "online record");
    // 7 of 24 starts ended in a retirement.
    assert!((r.finish_rate - 17.0 / 24.0).abs() < 0.001, "finish_rate {}", r.finish_rate);
    assert!(r.pace > 0.0 && r.quali > 0.0, "beat the car on average");
    assert!(r.mp_bonus < 0.0, "a losing online record must cost, not pay");
    assert!((r.value - 56.4).abs() < 0.5, "rating {}", r.value);
}

#[test]
fn test_reference_career_skips_classes_without_a_roster() {
    let data = reference_career();
    let ctxs = reference_contexts();
    // The career contains F-Junior sessions, and no F-Junior Custom AI file exists, so they
    // cannot be scored — they must not silently count as races.
    assert!(
        data.sessions.iter().any(|s| s.car_class == "F-Junior"),
        "fixture should still contain the unrateable class"
    );
    assert!(!ctxs.iter().any(|c| c.class == "F-Junior"));
    let all = compute_reputation_global(Some("Nightrat"), &data.sessions, &ctxs, None);
    let without: Vec<_> =
        data.sessions.iter().filter(|s| s.car_class != "F-Junior").cloned().collect();
    let trimmed = compute_reputation_global(Some("Nightrat"), &without, &ctxs, None);
    assert_eq!(all.sp_races, trimmed.sp_races);
    assert!((all.value - trimmed.value).abs() < 0.001);
}

#[test]
fn test_reference_career_second_human_is_online_only() {
    let data = reference_career();
    let r = compute_reputation_global(Some("Wiper"), &data.sessions, &reference_contexts(), None);
    // Wiper appears only in multiplayer, so there is no car-relative pace to rate — the value
    // is the neutral 50 plus his online record, which mirrors Nightrat's exactly.
    assert_eq!(r.sp_races, 0);
    assert_eq!((r.mp_wins, r.mp_losses), (5, 2));
    let me = compute_reputation_global(Some("Nightrat"), &data.sessions, &reference_contexts(), None);
    assert!((r.mp_bonus + me.mp_bonus).abs() < 0.001, "head-to-head must be zero-sum");
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
    assert!(osella.required > 40.0, "the floor should override the bar, not lower it");
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
