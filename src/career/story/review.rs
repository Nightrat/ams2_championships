//! The review of a finished season: what it came to, gathered from every other part of the story.
//!
//! Nothing new is worked out here. The champion and the player's place come from the tally, the
//! objective from the title maths, the teammate and rival from their modules, the mood arc from
//! the replay, and the top headlines from the feed — the review only picks out what a season is
//! remembered for.

use serde::Serialize;

use super::facts::SeasonFacts;
use super::headlines::{self, Headline};
use super::mood::{self, Band};
use super::rival::Rival;
use super::tally::SeasonSoFar;
use super::teammate::{self, HeadToHead};
use super::title::{self, ObjectiveStatus};

/// Headlines a review keeps: the season's biggest moments, not its whole feed.
const REVIEW_HEADLINES: usize = 5;

/// One weekend the review singles out.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Weekend {
    pub round: usize,
    pub track: String,
    pub position: u32,
    pub dnf: bool,
}

/// One driver's final place.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Placing {
    pub name: String,
    pub player: bool,
    pub position: u32,
    pub points: i32,
}

/// How the team's mood went over the season.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MoodArc {
    pub end: f32,
    pub band: Band,
    pub high: f32,
    pub low: f32,
}

/// What a finished season came to.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SeasonReview {
    pub champion: Option<Placing>,
    /// The player's final place; `None` when they never raced.
    pub player: Option<Placing>,
    /// Drivers in the final standings.
    pub field: usize,
    pub objective: Option<ObjectiveStatus>,
    pub best: Option<Weekend>,
    pub worst: Option<Weekend>,
    pub teammates: Vec<HeadToHead>,
    pub rival: Option<Rival>,
    pub mood: Option<MoodArc>,
    /// The biggest headlines, in the order they happened.
    pub headlines: Vec<Headline>,
}

/// The review of `facts`, as the season stands.
pub fn review(facts: &SeasonFacts) -> SeasonReview {
    let so_far = SeasonSoFar::whole(facts);
    let table = so_far.table();
    let placing = |i: usize| {
        table.get(i).map(|s| Placing {
            name: s.name.clone(),
            player: s.player,
            position: i as u32 + 1,
            points: s.points,
        })
    };
    let weekends: Vec<Weekend> = facts
        .rounds
        .iter()
        .filter_map(|r| {
            let race = r.race()?;
            let me = race.player()?;
            Some(Weekend {
                round: r.number,
                track: race.track.clone(),
                position: me.position,
                dnf: me.dnf,
            })
        })
        .collect();
    // Best is the highest classified finish, the first time it came; worst is the first
    // retirement, or failing one the lowest finish.
    let best = weekends.iter().filter(|w| !w.dnf).min_by_key(|w| (w.position, w.round)).cloned();
    let worst = weekends
        .iter()
        .find(|w| w.dnf)
        .or_else(|| weekends.iter().filter(|w| !w.dnf).max_by_key(|w| (w.position, std::cmp::Reverse(w.round))))
        .cloned();
    let moods = mood::replay(facts);
    let mut top = headlines::for_season(facts);
    top.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.round.cmp(&b.round)));
    top.truncate(REVIEW_HEADLINES);
    top.sort_by_key(|h| h.round);
    SeasonReview {
        champion: placing(0),
        player: table.iter().position(|s| s.player).and_then(placing),
        field: table.len(),
        objective: title::objective(facts, &so_far),
        best,
        worst,
        teammates: teammate::head_to_head(facts),
        rival: so_far.rivalry.current(),
        mood: moods.last().map(|last| MoodArc {
            end: last.mood,
            band: last.band,
            high: moods.iter().map(|m| m.mood).fold(f32::MIN, f32::max),
            low: moods.iter().map(|m| m.mood).fold(f32::MAX, f32::min),
        }),
        headlines: top,
    }
}

#[cfg(test)]
#[path = "../../tests/career/story/review.rs"]
mod tests;
