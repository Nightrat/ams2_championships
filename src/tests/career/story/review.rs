use super::*;
use crate::career::story::test_support::{ai, me, race_at, season};

fn senna(pos: u32) -> crate::career::story::facts::DriverResult {
    ai("Ayrton Senna", "Lotus", pos, false)
}

#[test]
fn test_a_review_names_the_champion_the_players_place_and_the_weekends_that_stood_out() {
    let facts = season(vec![
        race_at("r1", "Monza", vec![senna(1), me(4, false)]),
        race_at("r2", "Imola", vec![me(2, false), senna(1)]),
        race_at("r3", "Spa", vec![senna(1), me(5, true)]),
        race_at("r4", "Monaco", vec![me(2, false), senna(1)]),
    ]);
    let r = review(&facts);
    assert_eq!(r.champion.as_ref().map(|c| c.name.as_str()), Some("Ayrton Senna"));
    let me = r.player.unwrap();
    assert_eq!((me.position, me.points), (2, 3 + 6 + 6));
    assert_eq!(r.field, 2);
    // Best: the first time 2nd came. Worst: the retirement.
    assert_eq!(r.best.map(|w| (w.round, w.track)), Some((2, "Imola".to_string())));
    assert_eq!(r.worst.map(|w| (w.round, w.dnf)), Some((3, true)));
}

#[test]
fn test_without_a_retirement_the_worst_weekend_is_the_lowest_finish() {
    let facts = season(vec![
        race_at("r1", "Monza", vec![me(4, false)]),
        race_at("r2", "Imola", vec![me(9, false)]),
        race_at("r3", "Spa", vec![me(9, false)]),
    ]);
    assert_eq!(review(&facts).worst.map(|w| w.round), Some(2), "the first time it was that bad");
}

#[test]
fn test_a_review_keeps_only_the_biggest_headlines_in_order() {
    let races = (1..=10).map(|i| race_at(&format!("r{i}"), "Monza", vec![me(1, false), senna(2)])).collect();
    let r = review(&season(races));
    assert!(r.headlines.len() <= REVIEW_HEADLINES);
    assert!(r.headlines.windows(2).all(|w| w[0].round <= w[1].round));
    assert_eq!(r.headlines[0].key, "win.maiden");
}

#[test]
fn test_no_team_no_mood_arc() {
    let mut facts = season(vec![race_at("r1", "Monza", vec![me(1, false)])]);
    facts.player_team = None;
    assert_eq!(review(&facts).mood, None);
}
