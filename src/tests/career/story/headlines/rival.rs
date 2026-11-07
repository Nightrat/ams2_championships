use super::*;
use crate::career::story::facts::{SeasonFacts, SessionFacts};
use crate::career::story::headlines::{for_season_with, HeadlineRule};
use crate::career::story::test_support::{ai, me, race, season};

fn fired(facts: &SeasonFacts, rule: &dyn HeadlineRule) -> Vec<(usize, &'static str)> {
    for_season_with(facts, &[rule]).into_iter().map(|h| (h.round, h.key)).collect()
}

/// The player at `mine`, Prost at `prost`, Senna far behind. All outside the points.
fn at(mine: u32, prost: u32) -> SessionFacts {
    race("r", vec![me(mine, false), ai("Prost", "Lotus", prost, false), ai("Senna", "McLaren", 20, false)])
}

#[test]
fn test_a_new_rival_is_announced_once_and_a_replacement_names_who_they_replace() {
    let mut races = vec![at(10, 11); 3];
    races.extend((0..4).map(|_| race("r", vec![me(10, false), ai("Senna", "McLaren", 11, false), ai("Prost", "Lotus", 20, false)])));
    let feed = for_season_with(&season(races), &[&NewRival]);
    let got: Vec<(usize, &str, &str)> = feed.iter().map(|h| (h.round, h.key, h.subject.as_str())).collect();
    assert_eq!(got, [(2, "rival.new", "Prost"), (6, "rival.replaced", "Senna")]);
    assert!(feed[1].text.contains("Prost"), "{}", feed[1].text);
}

#[test]
fn test_the_head_to_head_turning_is_news_when_it_changes_hands() {
    // Prost ahead in the first two, then the player three times: 0–2, 1–2, 2–2, 3–2.
    let facts = season(vec![at(10, 9), at(10, 9), at(9, 10), at(9, 10), at(9, 10)]);
    assert_eq!(fired(&facts, &RivalDuelTurns), [(5, "rival.duel.ahead")]);
}

#[test]
fn test_no_rival_no_rivalry_news() {
    assert!(fired(&season(vec![at(10, 11)]), &NewRival).is_empty());
}
