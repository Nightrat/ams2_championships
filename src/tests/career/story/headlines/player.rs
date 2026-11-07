use super::*;
use crate::career::story::facts::{PlayerHistory, SeasonFacts};
use crate::career::story::headlines::{for_season_with, HeadlineRule};
use crate::career::story::test_support::{ai, me, quali, race, season, season_for};

/// The rounds `rule` fired in, with the template it chose.
fn fired(facts: &SeasonFacts, rule: &dyn HeadlineRule) -> Vec<(usize, &'static str)> {
    for_season_with(facts, &[rule]).into_iter().map(|h| (h.round, h.key)).collect()
}

/// A race of the player at `pos`.
fn at(pos: u32) -> crate::career::story::facts::SessionFacts {
    race("r", vec![me(pos, false)])
}

fn out() -> crate::career::story::facts::SessionFacts {
    race("r", vec![me(4, true)])
}

fn with_history(mut facts: SeasonFacts, history: PlayerHistory) -> SeasonFacts {
    facts.history = history;
    facts
}

#[test]
fn test_a_win_is_maiden_only_the_first_time() {
    assert_eq!(fired(&season(vec![at(2), at(1), at(1)]), &Win), [(2, "win.maiden"), (3, "win")]);
    let veteran = with_history(season(vec![at(1)]), PlayerHistory { wins: 4, ..Default::default() });
    assert_eq!(fired(&veteran, &Win), [(1, "win")]);
}

#[test]
fn test_a_podium_is_second_or_third_and_classified() {
    let facts = season(vec![at(4), race("r", vec![me(2, true)]), at(3), at(1), at(2)]);
    assert_eq!(fired(&facts, &Podium), [(3, "podium.maiden"), (5, "podium")]);
    // A win is already a podium, so the first P2 after one is not a *first* podium.
    assert_eq!(fired(&season(vec![at(1), at(2)]), &Podium), [(2, "podium")]);
}

#[test]
fn test_first_points_come_once_and_never_on_the_podium() {
    assert_eq!(fired(&season(vec![at(8), at(5), at(4)]), &FirstPoints), [(2, "points.first")]);
    assert!(fired(&season(vec![at(2), at(5)]), &FirstPoints).is_empty());
}

#[test]
fn test_pole_is_qualifying_first() {
    let facts = season_for(
        Some("Osella"),
        vec![],
        vec![vec![quali("q", vec![me(1, false)])], vec![quali("q", vec![me(2, false)])], vec![quali("q", vec![me(1, false)])]],
    );
    assert_eq!(fired(&facts, &Pole), [(1, "pole.maiden"), (3, "pole")]);
}

#[test]
fn test_a_debut_is_the_first_race_of_a_career_or_with_a_new_team() {
    assert_eq!(fired(&season(vec![out(), at(5)]), &Debut), [(1, "debut.career")]);

    let moved = PlayerHistory { starts: 16, last_team: Some("Williams".into()), ..Default::default() };
    assert_eq!(fired(&with_history(season(vec![at(5), at(4)]), moved), &Debut), [(1, "debut.team")]);

    let stayed = PlayerHistory { starts: 16, last_team: Some("Osella".into()), ..Default::default() };
    assert!(fired(&with_history(season(vec![at(5)]), stayed), &Debut).is_empty());

    // A round that was only qualifying is not the debut; the first race is.
    let facts = season_for(Some("Osella"), vec![], vec![vec![quali("q", vec![me(3, false)])], vec![at(6)]]);
    assert_eq!(fired(&facts, &Debut), [(2, "debut.career")]);
}

#[test]
fn test_streaks_are_announced_as_they_reach_a_mark() {
    let podiums = season(vec![at(2), at(3), at(1), at(2), at(2)]);
    assert_eq!(fired(&podiums, &PodiumStreak), [(3, "streak.podium"), (5, "streak.podium")]);

    // A run that breaks and builds again is announced again.
    let points = season(vec![at(6), at(6), at(6), at(9), at(6), at(6), at(6)]);
    assert_eq!(fired(&points, &PointsStreak), [(3, "streak.points"), (7, "streak.points")]);
}

#[test]
fn test_best_result_is_an_improvement_below_the_podium() {
    let scored = PlayerHistory { scored: true, starts: 10, ..Default::default() };
    let facts = with_history(season(vec![at(10), at(8), at(9), at(5), at(2)]), scored);
    assert_eq!(fired(&facts, &BestResult), [(2, "best"), (4, "best")]);
    // First points say it instead.
    assert!(fired(&season(vec![at(10), at(5)]), &BestResult).is_empty());
}

#[test]
fn test_only_the_first_retirement_of_the_season_is_news() {
    assert_eq!(fired(&season(vec![at(5), out(), out()]), &FirstRetirement), [(2, "dnf.first")]);
}

#[test]
fn test_the_player_taking_the_lead_from_someone_else() {
    let facts = season(vec![
        race("r1", vec![ai("Senna", "Lotus", 1, false), me(2, false)]),
        race("r2", vec![me(1, false), ai("Senna", "Lotus", 3, false)]),
        race("r3", vec![me(1, false), ai("Senna", "Lotus", 2, false)]),
    ]);
    assert_eq!(fired(&facts, &LeadTaken), [(2, "lead.taken")]);
}

#[test]
fn test_every_player_headline_is_about_the_player_and_fully_worded() {
    let facts = season(vec![at(10), at(5), at(2), at(1), at(1), at(1), out()]);
    let rules: [&dyn HeadlineRule; 9] =
        [&Win, &Podium, &FirstPoints, &Pole, &Debut, &PodiumStreak, &PointsStreak, &BestResult, &FirstRetirement];
    for rule in rules {
        for h in for_season_with(&facts, &[rule]) {
            assert!(h.player, "{h:?}");
            assert_eq!(h.subject, "Nightrat");
            assert!(!h.text.contains('{'), "{h:?}");
        }
    }
}
