//! [`SeasonFacts`]: what happened in one season, in the shape every story rule reads.
//!
//! This is the boundary between the story and the rest of the app. The service builds it once per
//! request — reading the roster through the season's own file, so a signed season uses its copy —
//! and every story function takes it by reference. Story rules therefore never see a roster file,
//! a config or the career store, and a test builds one by hand with no XML and no temp folder.
//!
//! What it settles once, so no rule has to settle it again:
//! - **which sessions count**: practice is dropped, the same rule the grid checks and the rating
//!   follow — nothing is derived from it.
//! - **the order**: rounds as the season lists them, and within a round qualifying before the race.
//! - **who drove for whom**: each result is matched to its roster entry here, by
//!   [`name_key`](crate::roster::custom_ai::name_key) — the same match seat inference uses, since
//!   AMS2's telemetry spelling does not always match the file's (`Allen` against `Allan Berg`).
//!
//! Three fields are the service's to fill after [`SeasonFacts::new`], because they come from
//! outside the season: [`SeasonFacts::history`] (the player's earlier seasons, via
//! [`PlayerHistory::before`]), [`SeasonFacts::expected`] (the rating's expected positions) and
//! [`SeasonFacts::objective`] (the contract's target).

use std::collections::HashMap;

use crate::career::data_store::{
    race_points, Championship, RecordedSession, SESSION_QUALIFY, SESSION_RACE,
};
use crate::roster::custom_ai::name_key;

/// One driver the season's roster fields.
#[derive(Clone, Debug, PartialEq)]
pub struct RosterDriver {
    /// The `<name>` the roster gives them.
    pub name: String,
    pub team: String,
    /// A per-track substitute rather than the driver who holds the seat.
    pub stand_in: bool,
}

/// The two session types a story is told from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionKind {
    Qualifying,
    Race,
}

impl SessionKind {
    /// The kind of a recorded `session_type`; `None` for practice and anything unknown.
    fn of(session_type: u32) -> Option<Self> {
        match session_type {
            SESSION_QUALIFY => Some(Self::Qualifying),
            SESSION_RACE => Some(Self::Race),
            _ => None,
        }
    }
}

/// One driver's result in one session.
#[derive(Clone, Debug, PartialEq)]
pub struct DriverResult {
    /// The name as telemetry recorded it.
    pub name: String,
    pub position: u32,
    pub dnf: bool,
    pub is_player: bool,
    /// What the result scored. Always 0 in qualifying.
    pub points: i32,
    /// The roster entry this result was matched to — `None` for the player, for stock AI and for
    /// anyone in a career with no roster.
    pub roster: Option<RosterDriver>,
}

impl DriverResult {
    /// Whether this result finished ahead of `other` in the same race: a classified finish beats
    /// a retirement, two finishes compare places, and two retirements beat nobody (`None`). The
    /// one statement of the rule every head-to-head counts by.
    pub fn beats(&self, other: &DriverResult) -> Option<bool> {
        match (self.dnf, other.dnf) {
            (false, true) => Some(true),
            (true, false) => Some(false),
            (false, false) if self.position != other.position => Some(self.position < other.position),
            _ => None,
        }
    }

    /// The team this result was raced for: the player's declared team, or the roster's.
    pub fn team<'a>(&'a self, player_team: Option<&'a str>) -> Option<&'a str> {
        if self.is_player {
            player_team
        } else {
            self.roster.as_ref().map(|r| r.team.as_str())
        }
    }
}

/// One qualifying or race session.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionFacts {
    pub id: String,
    pub kind: SessionKind,
    /// The track as a sentence names it: no layout, underscores as spaces ("Azure Circuit", not
    /// "Azure_Circuit – Azure_Circuit_2021"). The tables keep the full name; a headline reads it.
    pub track: String,
    /// When it was recorded — what orders seasons against each other.
    pub recorded_at: u64,
    pub results: Vec<DriverResult>,
}

impl SessionFacts {
    /// The player's row, if the player took part.
    pub fn player(&self) -> Option<&DriverResult> {
        self.results.iter().find(|r| r.is_player)
    }

    /// The row of the roster driver named `roster_name`, if they took part.
    pub fn driver(&self, roster_name: &str) -> Option<&DriverResult> {
        self.results
            .iter()
            .find(|r| r.roster.as_ref().is_some_and(|d| d.name == roster_name))
    }
}

/// One round of the season. A round that holds only practice, or nothing yet, is kept with no
/// sessions so that round numbers match what the player sees.
#[derive(Clone, Debug, PartialEq)]
pub struct RoundFacts {
    /// 1-based, as the Manage and Career tabs number rounds.
    pub number: usize,
    /// Qualifying before the race; sessions of one kind in the order they were recorded.
    pub sessions: Vec<SessionFacts>,
}

impl RoundFacts {
    /// The round's race — the last one, when a weekend ran more than one, since that is the
    /// main event.
    pub fn race(&self) -> Option<&SessionFacts> {
        self.sessions.iter().rev().find(|s| s.kind == SessionKind::Race)
    }

    /// The round's qualifying — the last one before the race.
    pub fn qualifying(&self) -> Option<&SessionFacts> {
        self.sessions.iter().rev().find(|s| s.kind == SessionKind::Qualifying)
    }

    /// Where the round was raced, from whichever session it has.
    pub fn track(&self) -> Option<&str> {
        self.race().or(self.sessions.first()).map(|s| s.track.as_str())
    }
}

/// What the player had done before a season began, so a season can tell a *maiden* win from
/// another one. Counted over the player's own rows only.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerHistory {
    pub starts: u32,
    pub wins: u32,
    pub podiums: u32,
    pub poles: u32,
    /// Whether any race has scored points.
    pub scored: bool,
    /// The team the player drove for in the latest season they started a race in.
    pub last_team: Option<String>,
}

impl PlayerHistory {
    /// Adds one session's player row, if the player took part.
    pub fn record(&mut self, s: &SessionFacts) {
        let Some(me) = s.player() else {
            return;
        };
        match s.kind {
            SessionKind::Qualifying => self.poles += u32::from(me.position == 1),
            SessionKind::Race => {
                self.starts += 1;
                if !me.dnf {
                    self.wins += u32::from(me.position == 1);
                    self.podiums += u32::from((1..=3).contains(&me.position));
                }
                self.scored |= me.points > 0;
            }
        }
    }

    /// The history `season` begins with: every season in `others` whose first session was
    /// recorded before `season`'s, taken in order. A season with no sessions has no start, and so
    /// neither counts nor is counted against.
    pub fn before(season: &SeasonFacts, others: &[SeasonFacts]) -> Self {
        let mut history = Self::default();
        let Some(start) = season.first_recorded() else {
            return history;
        };
        let mut earlier: Vec<(u64, &SeasonFacts)> = others
            .iter()
            .filter(|o| o.champ_id != season.champ_id)
            .filter_map(|o| Some((o.first_recorded()?, o)))
            .filter(|(at, _)| *at < start)
            .collect();
        earlier.sort_by_key(|(at, o)| (*at, o.champ_id.clone()));
        for (_, o) in earlier {
            let starts = history.starts;
            o.sessions().for_each(|s| history.record(s));
            if history.starts > starts && o.player_team.is_some() {
                history.last_team = o.player_team.clone();
            }
        }
        history
    }
}

/// Everything a story rule may know about one season.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeasonFacts {
    pub champ_id: String,
    pub player_team: Option<String>,
    /// The roster driver whose car the player took, if the season has a car fixed. They are not
    /// the player's teammate: the player is in their seat.
    pub replaced: Option<String>,
    /// Every driver the roster fields, in file order. Empty without a roster.
    pub roster: Vec<RosterDriver>,
    pub rounds: Vec<RoundFacts>,
    /// Each team's expected finishing position on this grid, from the rating — what makes a
    /// result a surprise. Empty without a roster, or when the season cannot be rated.
    pub expected: HashMap<String, f32>,
    /// What the player had done before this season. See [`PlayerHistory::before`].
    pub history: PlayerHistory,
    /// Points for positions 1, 2, 3, … — what a race left to run is worth.
    pub points_system: Vec<i32>,
    /// How many races the season is meant to run; `None` when it never said.
    pub planned_rounds: Option<u32>,
    /// The championship position the player's contract asks for, read off the contract.
    pub objective: Option<u32>,
}

impl SeasonFacts {
    /// The facts of `champ`, its sessions looked up in `sessions` and its drivers matched against
    /// `roster`.
    pub fn new(champ: &Championship, sessions: &[RecordedSession], roster: Vec<RosterDriver>) -> Self {
        let by_key = roster_by_key(&roster);
        let rounds = champ
            .rounds
            .iter()
            .enumerate()
            .map(|(i, round)| {
                let mut recorded: Vec<(SessionKind, &RecordedSession)> = round
                    .session_ids
                    .iter()
                    .filter_map(|id| sessions.iter().find(|s| &s.id == id))
                    .filter_map(|s| Some((SessionKind::of(s.session_type)?, s)))
                    .collect();
                recorded.sort_by_key(|(kind, s)| (*kind == SessionKind::Race, s.recorded_at));
                RoundFacts {
                    number: i + 1,
                    sessions: recorded
                        .into_iter()
                        .map(|(kind, s)| session_facts(kind, s, &champ.points_system, &by_key))
                        .collect(),
                }
            })
            .collect();
        Self {
            champ_id: champ.id.clone(),
            player_team: champ.player_team.clone().filter(|t| !t.is_empty()),
            replaced: champ.player_seat.as_ref().map(|s| s.replaces.clone()),
            roster,
            rounds,
            expected: HashMap::new(),
            history: PlayerHistory::default(),
            points_system: champ.points_system.clone(),
            planned_rounds: champ.planned_rounds.filter(|n| *n > 0),
            objective: None,
        }
    }

    /// Every counted session of the season, in order.
    pub fn sessions(&self) -> impl Iterator<Item = &SessionFacts> {
        self.rounds.iter().flat_map(|r| r.sessions.iter())
    }

    /// When the season's first counted session was recorded; `None` before it has one.
    pub fn first_recorded(&self) -> Option<u64> {
        self.sessions().map(|s| s.recorded_at).min()
    }
}

/// The roster keyed for matching telemetry names. A driver who holds a seat wins over a
/// stand-in of the same name: the regular entry is who they are for the season.
fn roster_by_key(roster: &[RosterDriver]) -> HashMap<String, &RosterDriver> {
    let mut by_key = HashMap::new();
    for d in roster.iter().filter(|d| !d.stand_in) {
        by_key.entry(name_key(&d.name)).or_insert(d);
    }
    for d in roster.iter().filter(|d| d.stand_in) {
        by_key.entry(name_key(&d.name)).or_insert(d);
    }
    by_key
}

fn session_facts(
    kind: SessionKind,
    s: &RecordedSession,
    points_system: &[i32],
    by_key: &HashMap<String, &RosterDriver>,
) -> SessionFacts {
    SessionFacts {
        id: s.id.clone(),
        kind,
        track: s.track.replace('_', " "),
        recorded_at: s.recorded_at,
        results: s
            .results
            .iter()
            .map(|r| DriverResult {
                name: r.name.clone(),
                position: r.race_position,
                dnf: r.dnf,
                is_player: r.is_player,
                points: match kind {
                    SessionKind::Race => race_points(points_system, r.race_position, r.dnf),
                    SessionKind::Qualifying => 0,
                },
                roster: if r.is_player {
                    None
                } else {
                    by_key.get(&name_key(&r.name)).map(|d| (*d).clone())
                },
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "../../tests/career/story/facts.rs"]
mod tests;
