use super::*;
use crate::career::story::facts::SeasonFacts;
use crate::career::story::headlines::{for_season_with, HeadlineRule};
use crate::career::story::test_support::{ai, me, race, season};

fn fired(facts: &SeasonFacts, rule: &dyn HeadlineRule) -> Vec<(usize, String, bool)> {
    for_season_with(facts, &[rule]).into_iter().map(|h| (h.round, h.subject, h.player)).collect()
}

fn senna(pos: u32) -> crate::career::story::facts::DriverResult {
    ai("Ayrton Senna", "Lotus", pos, false)
}

#[test]
fn test_the_title_is_announced_once_on_the_round_it_is_won() {
    let mut facts = season(vec![
        race("r1", vec![senna(1), me(4, false)]),
        race("r2", vec![senna(1), me(5, false)]),
        race("r3", vec![senna(2), me(1, false)]),
        race("r4", vec![senna(1), me(2, false)]),
    ]);
    facts.planned_rounds = Some(4);
    // After r2: Senna 18, player 5, two races left (18): still open. After r3: Senna 24,
    // player 14, one left (9): settled.
    assert_eq!(fired(&facts, &TitleClinched), [(3, "Ayrton Senna".to_string(), false)]);
    assert_eq!(fired(&facts, &PlayerOutOfTheFight), [(3, "Nightrat".to_string(), true)]);
}

#[test]
fn test_the_player_winning_the_title_is_their_headline() {
    let mut facts = season(vec![race("r1", vec![me(1, false), senna(2)]), race("r2", vec![me(1, false), senna(2)])]);
    facts.planned_rounds = Some(2);
    assert_eq!(fired(&facts, &TitleClinched), [(2, "Nightrat".to_string(), true)]);
    assert!(fired(&facts, &PlayerOutOfTheFight).is_empty());
}

#[test]
fn test_no_calendar_no_title_headlines() {
    let facts = season(vec![race("r1", vec![me(1, false), senna(2)])]);
    assert!(fired(&facts, &TitleClinched).is_empty());
}
