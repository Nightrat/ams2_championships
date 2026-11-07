use super::*;
use crate::career::story::facts::{RoundFacts, SessionFacts};
use crate::career::story::test_support::{ai, driver, me, race, season_for};

/// A 20-car race: the player at `mine`, the teammate Berg at `berg` (or absent), the rest filling
/// the other places.
fn grid(mine: Option<u32>, dnf: bool, berg: Option<u32>) -> SessionFacts {
    let mut rows = Vec::new();
    if let Some(p) = mine {
        rows.push(me(p, dnf));
    }
    if let Some(p) = berg {
        rows.push(ai("Allen Berg", "Osella", p, false));
    }
    let taken: Vec<u32> = [mine, berg].into_iter().flatten().collect();
    rows.extend(
        (1..=20)
            .filter(|p| !taken.contains(p))
            .take(20 - taken.len())
            .map(|p| ai(&format!("Driver {p}"), "Field", p, false)),
    );
    race("r", rows)
}

/// An Osella season, with Berg as teammate and the car expected around `expected`. The roster
/// names the whole 20-car grid, as a real one does — before the first race it is what the field
/// size is taken from.
fn osella(expected: f32, races: Vec<SessionFacts>) -> SeasonFacts {
    let mut roster = vec![driver("Allen Berg", "Osella", false)];
    roster.extend((1..=19).map(|p| driver(&format!("Driver {p}"), "Field", false)));
    let mut facts = season_for(Some("Osella"), roster, races.into_iter().map(|r| vec![r]).collect());
    facts.expected.insert("Osella".into(), expected);
    facts
}

fn verdicts(t: &SeasonTargets) -> Vec<(usize, &'static str, Verdict)> {
    t.judged.iter().map(|j| (j.target.round, j.target.kind, j.verdict)).collect()
}

#[test]
fn test_top_n_asks_for_the_cars_expected_place_with_room_to_spare() {
    let facts = osella(9.6, vec![grid(Some(12), false, Some(5)), grid(Some(13), false, Some(5)), grid(Some(3), true, Some(5))]);
    let t = season_with(&facts, &[&TopN]);
    // Expected 9.6 → 10th, plus two places of slack.
    assert!(t.judged.iter().all(|j| j.target.place == Some(12)));
    assert_eq!(
        verdicts(&t),
        [(1, "top_n", Verdict::Met), (2, "top_n", Verdict::Missed), (3, "top_n", Verdict::Missed)]
    );
    assert_eq!(t.judged[0].target.text, "finish 12th or better");
}

#[test]
fn test_top_n_does_not_ask_a_back_marker_to_beat_nobody() {
    let facts = osella(19.0, vec![grid(Some(18), false, None)]);
    assert!(season_with(&facts, &[&TopN]).judged.is_empty());
}

#[test]
fn test_beating_the_teammate_is_void_when_they_sat_it_out() {
    let facts = osella(10.0, vec![grid(Some(8), false, Some(9)), grid(Some(9), false, Some(8)), grid(Some(8), false, None)]);
    let t = season_with(&facts, &[&BeatTeammate]);
    assert_eq!(
        verdicts(&t),
        [
            (1, "beat_teammate", Verdict::Met),
            (2, "beat_teammate", Verdict::Missed),
            (3, "beat_teammate", Verdict::Void),
        ]
    );
    assert_eq!(t.judged[0].target.text, "finish ahead of Allen Berg");
}

#[test]
fn test_bringing_it_home_is_asked_only_after_a_retirement() {
    let facts = osella(10.0, vec![grid(Some(8), true, Some(9)), grid(Some(15), false, Some(9)), grid(Some(8), false, Some(9))]);
    assert_eq!(verdicts(&season_with(&facts, &[&BringItHome])), [(2, "bring_it_home", Verdict::Met)]);
}

#[test]
fn test_every_target_is_one_that_fits_and_the_choice_never_rerolls() {
    let races = (0..8).map(|i| grid(Some(8 + i % 3), i == 3, Some(10))).collect();
    let facts = osella(10.0, races);
    let t = season(&facts);
    assert_eq!(t.judged.len(), 8);
    // Only the round after the retirement in race 4 may ask to bring the car home.
    for j in &t.judged {
        assert!(j.target.kind != "bring_it_home" || j.target.round == 5, "{j:?}");
    }
    assert_eq!(t, season(&facts));
}

#[test]
fn test_the_next_target_is_for_the_first_round_not_yet_raced() {
    let mut facts = osella(10.0, vec![grid(Some(8), false, Some(9))]);
    assert_eq!(season(&facts).next.map(|n| n.round), Some(2));
    // A round already added but not raced yet is the next one.
    facts.rounds.push(RoundFacts { number: 2, sessions: vec![] });
    facts.rounds.push(RoundFacts { number: 3, sessions: vec![] });
    assert_eq!(season(&facts).next.map(|n| n.round), Some(2));
}

#[test]
fn test_no_team_no_targets() {
    let mut facts = osella(10.0, vec![grid(Some(8), false, Some(9))]);
    facts.player_team = None;
    assert_eq!(season(&facts), SeasonTargets::default());
}
