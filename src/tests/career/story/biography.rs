use super::*;
use crate::career::story::facts::SeasonFacts;
use crate::career::story::test_support::{ai, me, quali, race_at, season_for};

/// A season named `id`, recorded from `start` on, one session per round.
fn season(id: &str, team: &str, start: u64, sessions: Vec<crate::career::story::facts::SessionFacts>) -> SeasonFacts {
    let mut f = season_for(Some(team), vec![], sessions.into_iter().map(|s| vec![s]).collect());
    f.champ_id = id.into();
    for r in &mut f.rounds {
        for s in &mut r.sessions {
            s.recorded_at += start;
        }
    }
    f
}

#[test]
fn test_seasons_are_told_in_the_order_they_were_raced_with_their_results() {
    let later = season("1984", "Spirit-Hart", 200, vec![race_at("r", "Rio", vec![me(1, false), ai("Prost", "McLaren", 2, false)])]);
    let earlier = season("1983", "Spirit-Honda", 100, vec![race_at("r", "Monza", vec![ai("Prost", "Renault", 1, false), me(3, false)])]);
    let unstarted = season("1985", "Spirit-Hart", 0, vec![]);
    let bio = biography(&[(later, true), (unstarted, false), (earlier, true)]);
    let ids: Vec<&str> = bio.seasons.iter().map(|s| s.champ_id.as_str()).collect();
    assert_eq!(ids, ["1983", "1984"], "a season with nothing recorded has not begun");
    assert_eq!(bio.seasons[0].position, Some(2));
    assert_eq!((bio.seasons[0].podiums, bio.seasons[0].wins), (1, 0));
    assert!(!bio.seasons[0].champion);
    assert_eq!((bio.seasons[1].wins, bio.seasons[1].champion), (1, true));
}

#[test]
fn test_each_first_is_recorded_once_where_it_happened() {
    let y83 = season(
        "1983",
        "Spirit-Honda",
        100,
        vec![
            race_at("r1", "Rio", vec![me(12, true)]),
            race_at("r2", "Long Beach", vec![ai("Prost", "Renault", 1, false), me(5, false)]),
            quali("q3", vec![me(1, false)]),
        ],
    );
    let y84 = season("1984", "Spirit-Hart", 200, vec![race_at("r1", "Rio", vec![me(1, false)])]);
    let bio = biography(&[(y83, true), (y84, false)]);
    let at = |kind: &str| bio.milestones.iter().find(|m| m.kind == kind).map(|m| (m.champ_id.as_str(), m.round));
    assert_eq!(at("first_start"), Some(("1983", 1)));
    assert_eq!(at("first_points"), Some(("1983", 2)));
    assert_eq!(at("first_pole"), Some(("1983", 3)));
    assert_eq!(at("first_win"), Some(("1984", 1)));
    assert_eq!(at("first_title"), None, "Prost won 1983, and 1984 is not finished");
    assert_eq!(bio.milestones.iter().filter(|m| m.kind == "first_start").count(), 1);
}
