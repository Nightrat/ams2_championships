//! Positions outside the points (the table scores six), so everyone stays level on 0 and the
//! standings weigh the same for all: proximity alone decides who the rival is.

use crate::career::story::facts::{SeasonFacts, SessionFacts};
use crate::career::story::tally::SeasonSoFar;
use crate::career::story::test_support::{ai, driver, me, race, season, season_for};

/// The player 10th, `near` 11th, `far` 20th.
fn round(near: &str, far: &str) -> SessionFacts {
    race("r", vec![me(10, false), ai(near, "Lotus", 11, false), ai(far, "McLaren", 20, false)])
}

/// The rival after each round.
fn rivals(facts: &SeasonFacts) -> Vec<Option<String>> {
    let mut so_far = SeasonSoFar::start(facts);
    facts
        .rounds
        .iter()
        .map(|r| {
            so_far.record(r);
            so_far.rivalry.current().map(|c| c.name)
        })
        .collect()
}

fn named(n: &str) -> Option<String> {
    Some(n.to_string())
}

#[test]
fn test_one_race_is_not_enough_for_a_rival() {
    assert_eq!(rivals(&season(vec![round("Prost", "Senna")])), [None]);
}

#[test]
fn test_the_driver_who_keeps_finishing_beside_the_player_is_the_rival() {
    let facts = season(vec![round("Prost", "Senna"), round("Prost", "Senna"), round("Prost", "Senna")]);
    assert_eq!(rivals(&facts), [None, named("Prost"), named("Prost")]);
    let rival = SeasonSoFar::whole(&facts).rivalry.current().unwrap();
    assert_eq!(rival.since_round, 2);
    assert_eq!(rival.team.as_deref(), Some("Lotus"));
    assert_eq!((rival.duel.races, rival.duel.player, rival.duel.rival), (3, 3, 0));
}

#[test]
fn test_a_new_rival_must_stay_ahead_for_two_races_to_take_over() {
    let mut races = vec![round("Prost", "Senna"); 3];
    races.extend(vec![round("Senna", "Prost"); 4]);
    let facts = season(races);
    // Senna first out-weighs Prost by the margin after race 5, and is still ahead after race 6.
    assert_eq!(
        rivals(&facts),
        [None, named("Prost"), named("Prost"), named("Prost"), named("Prost"), named("Senna"), named("Senna")]
    );
    assert_eq!(SeasonSoFar::whole(&facts).rivalry.current().unwrap().since_round, 6);
}

#[test]
fn test_a_teammate_is_never_the_rival() {
    let row = || race("r", vec![me(10, false), ai("Allen Berg", "Osella", 11, false), ai("Prost", "McLaren", 13, false)]);
    let facts = season_for(Some("Osella"), vec![driver("Allen Berg", "Osella", false)], vec![vec![row()], vec![row()]]);
    assert_eq!(rivals(&facts), [None, named("Prost")]);
}

#[test]
fn test_a_race_the_player_missed_changes_nothing() {
    let missed = race("r", vec![ai("Prost", "Lotus", 1, false), ai("Senna", "McLaren", 2, false)]);
    let facts = season(vec![round("Prost", "Senna"), missed, round("Prost", "Senna")]);
    assert_eq!(rivals(&facts), [None, None, named("Prost")]);
}
