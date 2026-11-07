use super::*;
use crate::career::story::facts::SeasonFacts;
use crate::career::story::headlines::{for_season_with, HeadlineRule};
use crate::career::story::test_support::{ai, me, race, season};

fn fired(facts: &SeasonFacts, rule: &dyn HeadlineRule) -> Vec<(usize, String)> {
    for_season_with(facts, &[rule]).into_iter().map(|h| (h.round, h.subject)).collect()
}

fn senna(pos: u32) -> crate::career::story::facts::DriverResult {
    ai("Ayrton Senna", "Lotus", pos, false)
}

fn prost(pos: u32) -> crate::career::story::facts::DriverResult {
    ai("Alain Prost", "McLaren", pos, false)
}

#[test]
fn test_a_lead_change_names_the_new_leader() {
    let facts = season(vec![
        race("r1", vec![senna(1), prost(2), me(4, false)]),
        race("r2", vec![prost(1), senna(3), me(4, false)]),
        race("r3", vec![prost(1), senna(2), me(4, false)]),
    ]);
    // After r2: Prost 15, Senna 13.
    assert_eq!(fired(&facts, &LeadChange), [(2, "Alain Prost".to_string())]);
}

#[test]
fn test_the_player_losing_the_lead_is_news_too() {
    let facts = season(vec![
        race("r1", vec![me(1, false), senna(2)]),
        race("r2", vec![senna(1), me(4, false)]),
    ]);
    assert_eq!(fired(&facts, &LeadChange), [(2, "Ayrton Senna".to_string())]);
}

#[test]
fn test_a_first_win_of_the_season_needs_a_race_before_it() {
    let facts = season(vec![
        race("r1", vec![senna(1), prost(2)]),
        race("r2", vec![prost(1), senna(2)]),
        race("r3", vec![senna(1), prost(2)]),
        race("r4", vec![prost(1), senna(2)]),
    ]);
    assert_eq!(fired(&facts, &FirstWinOfSeason), [(2, "Alain Prost".to_string())]);
}

#[test]
fn test_a_winner_who_still_trails_the_leader() {
    let facts = season(vec![
        race("r1", vec![senna(1), prost(9)]),
        race("r2", vec![prost(1), senna(2)]),
    ]);
    // Senna 15, Prost 9 after r2. Senna winning r1 while leading is not this headline.
    assert_eq!(fired(&facts, &WinnerNotLeader), [(2, "Alain Prost".to_string())]);
}

#[test]
fn test_the_players_own_win_is_not_field_news() {
    let facts = season(vec![race("r1", vec![senna(1)]), race("r2", vec![me(1, false), senna(2)])]);
    assert!(fired(&facts, &FirstWinOfSeason).is_empty());
    assert!(fired(&facts, &WinnerNotLeader).is_empty());
}

#[test]
fn test_an_upset_is_the_biggest_finish_far_above_the_car() {
    let berg = |pos| ai("Allen Berg", "Osella", pos, false);
    let ghinzani = |pos| ai("Piercarlo Ghinzani", "Osella", pos, false);
    let mut facts = season(vec![
        race("r1", vec![berg(10), ghinzani(12), me(9, false)]),
        race("r2", vec![berg(14), ghinzani(15)]),
        race("r3", vec![ai("Allen Berg", "Osella", 2, true)]),
    ]);
    // A team at the back expects about 18th.
    facts.expected.insert("Osella".into(), 18.0);
    assert_eq!(fired(&facts, &Upset), [(1, "Allen Berg".to_string())]);

    let text = &for_season_with(&facts, &[&Upset])[0].text;
    assert!(text.contains("Osella") && text.contains("10th"), "{text}");

    facts.expected.clear();
    assert!(fired(&facts, &Upset).is_empty(), "no expected positions, no upsets");
}
