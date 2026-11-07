use super::*;
use crate::career::story::test_support::{ai, me, quali, race, season, season_for};

fn after(rounds: Vec<crate::career::story::facts::SessionFacts>) -> SeasonSoFar {
    SeasonSoFar::whole(&season(rounds))
}

#[test]
fn test_a_tie_for_the_lead_is_no_leader() {
    let so_far = after(vec![race("r1", vec![ai("Senna", "Lotus", 1, false), me(2, false)])]);
    assert_eq!(so_far.leader().map(|l| l.name.as_str()), Some("Senna"));
    assert_eq!(so_far.lead(), 3);

    let level = after(vec![
        race("r1", vec![ai("Senna", "Lotus", 1, false), me(2, false)]),
        race("r2", vec![ai("Senna", "Lotus", 2, false), me(1, false)]),
    ]);
    assert_eq!(level.leader(), None, "15 points each");
    assert_eq!(level.lead(), 0);
}

#[test]
fn test_the_table_breaks_a_points_tie_on_the_countback() {
    // 9 + 1 against 6 + 4: level on 10, but one has a win.
    let so_far = after(vec![
        race("r1", vec![ai("Prost", "McLaren", 2, false), ai("Senna", "Lotus", 1, false)]),
        race("r2", vec![ai("Prost", "McLaren", 3, false), ai("Senna", "Lotus", 6, false)]),
    ]);
    let names: Vec<&str> = so_far.table().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Senna", "Prost"]);
}

#[test]
fn test_rounds_raced_count_rounds_with_a_race_not_races() {
    let facts = season_for(
        Some("Osella"),
        vec![],
        vec![
            vec![quali("q", vec![me(1, false)]), race("r", vec![me(1, false)]), race("r2", vec![me(2, false)])],
            vec![quali("q", vec![me(1, false)])],
        ],
    );
    let so_far = SeasonSoFar::whole(&facts);
    assert_eq!((so_far.rounds_raced, so_far.races), (1, 2));
    assert_eq!(so_far.player_position(), Some(1));
}
