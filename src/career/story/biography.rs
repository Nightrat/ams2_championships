//! The player's career as a story: a line per season and the milestones along the way.
//!
//! Seasons are put in the order they were raced — by their first recorded session — since a
//! championship carries no date of its own. A season with nothing recorded has not begun, and is
//! left out. Like everything in the story it is recounted from results each time.

use serde::Serialize;

use super::facts::{SeasonFacts, SessionKind};
use super::tally::SeasonSoFar;

/// One season of the career.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SeasonLine {
    pub champ_id: String,
    pub team: Option<String>,
    /// The player's final place; `None` when they never raced in it.
    pub position: Option<u32>,
    pub field: usize,
    pub points: i32,
    pub wins: u32,
    pub podiums: u32,
    /// Finished, and first.
    pub champion: bool,
}

/// A first in the career.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Milestone {
    /// Which first: `first_start`, `first_points`, `first_podium`, `first_win`, `first_pole`,
    /// `first_title`.
    pub kind: &'static str,
    pub champ_id: String,
    pub round: usize,
    pub track: String,
}

/// The whole career.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Biography {
    pub seasons: Vec<SeasonLine>,
    pub milestones: Vec<Milestone>,
}

/// The career told from `seasons`, each paired with whether it is finished (a title is only won
/// once a season is over).
pub fn biography(seasons: &[(SeasonFacts, bool)]) -> Biography {
    let mut ordered: Vec<(u64, &SeasonFacts, bool)> = seasons
        .iter()
        .filter_map(|(f, done)| Some((f.first_recorded()?, f, *done)))
        .collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.champ_id.cmp(&b.1.champ_id)));
    let mut bio = Biography::default();
    for (_, facts, finished) in ordered {
        let so_far = SeasonSoFar::whole(facts);
        let position = so_far.player_position();
        let points = so_far.table().iter().find(|s| s.player).map_or(0, |s| s.points);
        let mut wins = 0;
        let mut podiums = 0;
        for round in &facts.rounds {
            for s in &round.sessions {
                let Some(me) = s.player() else { continue };
                let mut first = |kind: &'static str| {
                    if !bio.milestones.iter().any(|m| m.kind == kind) {
                        bio.milestones.push(Milestone {
                            kind,
                            champ_id: facts.champ_id.clone(),
                            round: round.number,
                            track: s.track.clone(),
                        });
                    }
                };
                match s.kind {
                    SessionKind::Qualifying if me.position == 1 => first("first_pole"),
                    SessionKind::Qualifying => {}
                    SessionKind::Race => {
                        first("first_start");
                        if me.points > 0 {
                            first("first_points");
                        }
                        if !me.dnf && me.position <= 3 {
                            podiums += 1;
                            first("first_podium");
                        }
                        if !me.dnf && me.position == 1 {
                            wins += 1;
                            first("first_win");
                        }
                    }
                }
            }
        }
        let champion = finished && position == Some(1);
        if champion && !bio.milestones.iter().any(|m| m.kind == "first_title") {
            let last = facts.rounds.iter().rev().find_map(|r| Some((r.number, r.race()?)));
            bio.milestones.push(Milestone {
                kind: "first_title",
                champ_id: facts.champ_id.clone(),
                round: last.map_or(0, |l| l.0),
                track: last.map(|l| l.1.track.clone()).unwrap_or_default(),
            });
        }
        bio.seasons.push(SeasonLine {
            champ_id: facts.champ_id.clone(),
            team: facts.player_team.clone(),
            position,
            field: so_far.table().len(),
            points,
            wins,
            podiums,
            champion,
        });
    }
    bio
}

#[cfg(test)]
#[path = "../../tests/career/story/biography.rs"]
mod tests;
