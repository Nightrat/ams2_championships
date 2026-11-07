//! The Paddock news feed: a season's results told back as headlines, round by round.
//!
//! **Each kind of headline is one [`HeadlineRule`] in [`RULES`].** The walker goes through the
//! rounds in order, keeping a running tally ([`SeasonSoFar`]) of what has happened, and asks every
//! rule whether this round is news. It never knows which rules exist — adding a headline is one
//! struct and one line in `RULES`, and nothing here changes.
//!
//! A rule says *that* something is news and hands over the facts; [`templates`](super::templates)
//! says how it is worded. Worked out from results on every request and never stored, so
//! reassigning a session rewrites the feed, which is intended: the feed is the season as it
//! now stands.

use std::collections::BTreeMap;

use serde::Serialize;

use super::facts::{DriverResult, RoundFacts, SeasonFacts};
use super::tally::SeasonSoFar;
use super::templates;

mod field;
mod mood;
mod player;
mod rival;
mod title;

/// Most headlines one round may carry. A thirty-car race can make a lot of news, and a feed that
/// lists all of it buries what mattered; the highest-priority ones are kept.
pub const MAX_PER_ROUND: usize = 4;

/// Most headlines one driver may get in one round, so a weekend's winner does not take every
/// slot with their pole, their win and their streak.
pub const MAX_PER_SUBJECT: usize = 2;

/// Every headline the feed can carry. Order is the tie-break between equal priorities.
pub const RULES: &[&dyn HeadlineRule] = &[
    &title::TitleClinched,
    &player::Win,
    &player::LeadTaken,
    &player::Podium,
    &player::FirstPoints,
    &player::Pole,
    &player::Debut,
    &title::PlayerOutOfTheFight,
    &rival::NewRival,
    &mood::MoodShift,
    &mood::ConsideringOptions,
    &player::PodiumStreak,
    &player::BestResult,
    &rival::RivalDuelTurns,
    &player::PointsStreak,
    &player::FirstRetirement,
    &field::LeadChange,
    &field::FirstWinOfSeason,
    &field::WinnerNotLeader,
    &field::Upset,
];

/// One kind of news.
pub trait HeadlineRule: Sync {
    /// A stable name for the kind, sent to the page so it can style a headline.
    fn kind(&self) -> &'static str;
    /// How much it matters, 0–100. Decides which headlines survive [`MAX_PER_ROUND`].
    fn priority(&self) -> u8;
    /// This round's headline of this kind, if the round is news.
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft>;
}

/// What a rule sees: the season, the round, and the tally before and after it.
pub struct RoundContext<'a> {
    pub facts: &'a SeasonFacts,
    pub round: &'a RoundFacts,
    /// Everything up to the previous round.
    pub before: &'a SeasonSoFar,
    /// Everything up to and including this round.
    pub after: &'a SeasonSoFar,
}

/// A headline before it is worded: what it is about and the values its sentence needs.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadlineDraft {
    /// The session it is about.
    pub session_id: String,
    /// Who it is about.
    pub subject: String,
    pub player: bool,
    /// The template to word it with.
    pub key: &'static str,
    pub values: Vec<(&'static str, String)>,
}

/// One headline in the feed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Headline {
    pub round: usize,
    pub session_id: String,
    pub kind: &'static str,
    pub priority: u8,
    pub subject: String,
    /// About the player rather than the field.
    pub player: bool,
    /// The template it was worded with — which variant of the news it is ("win.maiden").
    pub key: &'static str,
    pub text: String,
}

/// The season's headlines, round by round.
pub fn for_season(facts: &SeasonFacts) -> Vec<Headline> {
    for_season_with(facts, RULES)
}

/// [`for_season`] over any list of rules — what the tests drive one rule at a time.
pub fn for_season_with(facts: &SeasonFacts, rules: &[&dyn HeadlineRule]) -> Vec<Headline> {
    let mut before = SeasonSoFar::start(facts);
    let mut feed = Vec::new();
    for round in &facts.rounds {
        let mut after = before.clone();
        after.record(round);
        let cx = RoundContext { facts, round, before: &before, after: &after };
        let mut found: Vec<(u8, usize, Headline)> = rules
            .iter()
            .enumerate()
            .filter_map(|(order, rule)| {
                let draft = rule.check(&cx)?;
                Some((rule.priority(), order, word(facts, round, *rule, draft)))
            })
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut per_subject: BTreeMap<String, usize> = BTreeMap::new();
        feed.extend(
            found
                .into_iter()
                .map(|(_, _, h)| h)
                .filter(|h| {
                    let n = per_subject.entry(h.subject.clone()).or_default();
                    *n += 1;
                    *n <= MAX_PER_SUBJECT
                })
                .take(MAX_PER_ROUND),
        );
        before = after;
    }
    feed
}

fn word(facts: &SeasonFacts, round: &RoundFacts, rule: &dyn HeadlineRule, d: HeadlineDraft) -> Headline {
    let seed = templates::seed(&[&facts.champ_id, &d.session_id, d.key, &d.subject]);
    Headline {
        round: round.number,
        text: templates::render(d.key, &d.values, seed),
        session_id: d.session_id,
        kind: rule.kind(),
        priority: rule.priority(),
        subject: d.subject,
        player: d.player,
        key: d.key,
    }
}

/// "1 point", "12 points".
pub(crate) fn points(n: i32) -> String {
    if n == 1 {
        "1 point".to_string()
    } else {
        format!("{n} points")
    }
}

/// A classified place, or "retired".
fn result_of(r: &DriverResult) -> String {
    if r.dnf {
        "retired".to_string()
    } else {
        crate::format::ordinal(r.position)
    }
}

#[cfg(test)]
#[path = "../../tests/career/story/headlines.rs"]
mod tests;
