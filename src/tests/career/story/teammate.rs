//! Built from hand-made [`SeasonFacts`] — no roster file, no career — which is the point of the
//! facts boundary.

use super::*;
use crate::career::story::facts::{RosterDriver, RoundFacts, SessionFacts};
use crate::career::story::test_support::driver;

/// Brabham with three regular entries (Warwick and De Angelis share #8), a stand-in, and the
/// player in Warwick's car.
fn roster() -> Vec<RosterDriver> {
    vec![
        driver("Nelson Piquet", "Williams", false),
        driver("Riccardo Patrese", "Brabham", false),
        driver("Derek Warwick", "Brabham", false),
        driver("Elio de Angelis", "Brabham", false),
        driver("Tino Brambilla", "Brabham", true),
        // A second livery for the same driver is still one teammate.
        driver("Riccardo Patrese", "Brabham", false),
    ]
}

/// A row for roster driver `name`, or the player when `name` is `"me"`.
fn row(name: &str, position: u32, dnf: bool, points: i32) -> DriverResult {
    let is_player = name == "me";
    DriverResult {
        name: name.into(),
        position,
        dnf,
        is_player,
        points,
        roster: if is_player {
            None
        } else {
            roster().into_iter().find(|d| d.name == name)
        },
    }
}

/// Points are given per row here rather than scored, so each test states exactly what it adds up.
fn session(kind: SessionKind, rows: Vec<DriverResult>) -> SessionFacts {
    SessionFacts {
        id: "s".into(),
        kind,
        track: "Monza".into(),
        recorded_at: 0,
        results: rows,
    }
}

fn quali(rows: Vec<DriverResult>) -> SessionFacts {
    session(SessionKind::Qualifying, rows)
}

fn race(rows: Vec<DriverResult>) -> SessionFacts {
    session(SessionKind::Race, rows)
}

/// A Brabham season, one session per round.
fn season(sessions: Vec<SessionFacts>) -> SeasonFacts {
    SeasonFacts {
        champ_id: "c1".into(),
        player_team: Some("Brabham".into()),
        replaced: Some("Derek Warwick".into()),
        roster: roster(),
        rounds: sessions
            .into_iter()
            .enumerate()
            .map(|(i, s)| RoundFacts {
                number: i + 1,
                sessions: vec![s],
            })
            .collect(),
        ..Default::default()
    }
}

fn with(all: &[HeadToHead], teammate: &str) -> HeadToHead {
    all.iter().find(|h| h.teammate == teammate).cloned().unwrap()
}

#[test]
fn test_teammates_are_the_teams_regular_drivers_but_not_the_one_replaced() {
    let facts = season(vec![]);
    // No Warwick (the player has his car), no stand-in, Patrese once despite two liveries.
    assert_eq!(teammates(&facts), ["Riccardo Patrese", "Elio de Angelis"]);
}

#[test]
fn test_a_season_without_a_team_has_no_head_to_head() {
    let mut facts = season(vec![race(vec![row("me", 1, false, 9)])]);
    facts.player_team = None;
    assert!(head_to_head(&facts).is_empty());
}

#[test]
fn test_qualifying_and_races_are_counted_with_points_and_finishes() {
    let facts = season(vec![
        quali(vec![row("me", 3, false, 0), row("Riccardo Patrese", 5, false, 0)]),
        race(vec![row("me", 2, false, 6), row("Riccardo Patrese", 1, false, 9)]),
        quali(vec![row("me", 4, false, 0), row("Riccardo Patrese", 2, false, 0)]),
        race(vec![row("me", 1, false, 9), row("Riccardo Patrese", 4, false, 0)]),
    ]);
    let h = with(&head_to_head(&facts), "Riccardo Patrese");

    assert_eq!((h.qualifyings, h.qualifying), (2, Pair { player: 1, teammate: 1 }));
    assert_eq!((h.races, h.race), (2, Pair { player: 1, teammate: 1 }));
    assert_eq!(h.points, Pair { player: 15, teammate: 9 });
    assert_eq!(h.average_finish, Pair { player: Some(1.5), teammate: Some(2.5) });
    assert_eq!(h.retirements, Pair::default());
}

#[test]
fn test_a_session_the_teammate_missed_counts_for_neither() {
    let facts = season(vec![
        race(vec![row("me", 1, false, 9), row("Nelson Piquet", 2, false, 6)]),
        race(vec![row("me", 3, false, 4), row("Riccardo Patrese", 2, false, 6)]),
    ]);
    let h = with(&head_to_head(&facts), "Riccardo Patrese");
    assert_eq!(h.races, 1);
    assert_eq!(h.race, Pair { player: 0, teammate: 1 });
    assert_eq!(h.points, Pair { player: 4, teammate: 6 });
}

#[test]
fn test_alternate_drivers_of_one_seat_each_count_only_their_own_sessions() {
    let facts = season(vec![
        race(vec![row("me", 1, false, 9), row("Elio de Angelis", 2, false, 6)]),
        race(vec![row("me", 1, false, 9), row("Riccardo Patrese", 2, false, 6)]),
        race(vec![row("me", 3, false, 4), row("Riccardo Patrese", 2, false, 6)]),
    ]);
    let all = head_to_head(&facts);
    assert_eq!(with(&all, "Elio de Angelis").races, 1);
    assert_eq!(with(&all, "Riccardo Patrese").race, Pair { player: 1, teammate: 1 });
}

#[test]
fn test_a_stand_in_is_nobodys_teammate() {
    let facts = season(vec![race(vec![row("me", 2, false, 6), row("Tino Brambilla", 1, false, 9)])]);
    for h in head_to_head(&facts) {
        assert_eq!(h.races, 0, "{h:?}");
    }
}

#[test]
fn test_a_finish_beats_a_retirement_and_two_retirements_beat_nobody() {
    let facts = season(vec![
        // Classified 9th still beats a car that retired while 3rd.
        race(vec![row("Riccardo Patrese", 3, true, 0), row("me", 9, false, 0)]),
        race(vec![row("me", 4, true, 0), row("Riccardo Patrese", 6, true, 0)]),
    ]);
    let h = with(&head_to_head(&facts), "Riccardo Patrese");
    assert_eq!(h.races, 2);
    assert_eq!(h.race, Pair { player: 1, teammate: 0 });
    assert_eq!(h.retirements, Pair { player: 1, teammate: 2 });
    assert_eq!(h.average_finish, Pair { player: Some(9.0), teammate: None });
}
