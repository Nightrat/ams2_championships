//! Hand-built [`SeasonFacts`] for the story tests — no roster file and no career, which is the
//! point of the facts boundary.

use std::collections::HashMap;

use crate::career::data_store::race_points;
use crate::career::story::facts::{
    DriverResult, PlayerHistory, RosterDriver, RoundFacts, SeasonFacts, SessionFacts, SessionKind,
};

/// The points table every hand-built race scores on.
pub const POINTS: &[i32] = &[9, 6, 4, 3, 2, 1];

/// A roster driver.
pub fn driver(name: &str, team: &str, stand_in: bool) -> RosterDriver {
    RosterDriver {
        name: name.into(),
        team: team.into(),
        stand_in,
    }
}

/// The player's row. Points are filled in by [`race`].
pub fn me(position: u32, dnf: bool) -> DriverResult {
    DriverResult {
        name: "Nightrat".into(),
        position,
        dnf,
        is_player: true,
        points: 0,
        roster: None,
    }
}

/// An AI row driving for `team`. Points are filled in by [`race`].
pub fn ai(name: &str, team: &str, position: u32, dnf: bool) -> DriverResult {
    DriverResult {
        name: name.into(),
        position,
        dnf,
        is_player: false,
        points: 0,
        roster: Some(driver(name, team, false)),
    }
}

fn session(id: &str, kind: SessionKind, track: &str, results: Vec<DriverResult>) -> SessionFacts {
    SessionFacts {
        id: id.into(),
        kind,
        track: track.into(),
        recorded_at: 0,
        results,
    }
}

/// A race at Monza, every row scored on [`POINTS`].
pub fn race(id: &str, rows: Vec<DriverResult>) -> SessionFacts {
    race_at(id, "Monza", rows)
}

/// [`race`] at another track.
pub fn race_at(id: &str, track: &str, rows: Vec<DriverResult>) -> SessionFacts {
    let rows = rows
        .into_iter()
        .map(|r| DriverResult {
            points: race_points(POINTS, r.position, r.dnf),
            ..r
        })
        .collect();
    session(id, SessionKind::Race, track, rows)
}

/// A qualifying session at Monza.
pub fn quali(id: &str, rows: Vec<DriverResult>) -> SessionFacts {
    session(id, SessionKind::Qualifying, "Monza", rows)
}

/// A season for `team` with one round per entry of `rounds`, recorded in order.
pub fn season_for(team: Option<&str>, roster: Vec<RosterDriver>, rounds: Vec<Vec<SessionFacts>>) -> SeasonFacts {
    let mut at = 0;
    SeasonFacts {
        champ_id: "c1".into(),
        player_team: team.map(String::from),
        replaced: None,
        roster,
        rounds: rounds
            .into_iter()
            .enumerate()
            .map(|(i, sessions)| RoundFacts {
                number: i + 1,
                sessions: sessions
                    .into_iter()
                    .map(|s| {
                        at += 1;
                        SessionFacts { recorded_at: at, ..s }
                    })
                    .collect(),
            })
            .collect(),
        expected: HashMap::new(),
        history: PlayerHistory::default(),
        points_system: POINTS.to_vec(),
        planned_rounds: None,
        objective: None,
    }
}

/// An Osella season, one round per race.
pub fn season(races: Vec<SessionFacts>) -> SeasonFacts {
    season_for(Some("Osella"), vec![], races.into_iter().map(|r| vec![r]).collect())
}
