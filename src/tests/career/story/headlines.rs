//! The walker, driven with fake rules — what it keeps, in what order — and the tally it hands
//! them. Each real rule has its own file under `headlines/`.

use super::*;
use crate::career::story::test_support::{ai, me, race, season};

/// A rule that makes news about `subject` every round.
struct Always {
    kind: &'static str,
    priority: u8,
    subject: &'static str,
}

impl HeadlineRule for Always {
    fn kind(&self) -> &'static str {
        self.kind
    }
    fn priority(&self) -> u8 {
        self.priority
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        Some(HeadlineDraft {
            session_id: cx.round.race()?.id.clone(),
            subject: self.subject.into(),
            player: false,
            key: "win",
            values: vec![("driver", self.subject.into()), ("track", "Monza".into())],
        })
    }
}

fn one_round() -> SeasonFacts {
    season(vec![race("r1", vec![me(1, false)])])
}

fn kinds(feed: &[Headline]) -> Vec<&'static str> {
    feed.iter().map(|h| h.kind).collect()
}

#[test]
fn test_a_round_keeps_only_its_highest_priority_headlines() {
    let rules: Vec<Always> = (0..6)
        .map(|i| Always {
            kind: ["a", "b", "c", "d", "e", "f"][i],
            priority: [10, 60, 30, 90, 50, 20][i],
            subject: ["A", "B", "C", "D", "E", "F"][i],
        })
        .collect();
    let refs: Vec<&dyn HeadlineRule> = rules.iter().map(|r| r as &dyn HeadlineRule).collect();
    let feed = for_season_with(&one_round(), &refs);
    assert_eq!(feed.len(), MAX_PER_ROUND);
    assert_eq!(kinds(&feed), ["d", "b", "e", "c"]);
}

#[test]
fn test_one_driver_gets_at_most_two_headlines_a_round() {
    let rules = [
        Always { kind: "a", priority: 50, subject: "Senna" },
        Always { kind: "b", priority: 50, subject: "Senna" },
        Always { kind: "c", priority: 50, subject: "Senna" },
        Always { kind: "d", priority: 10, subject: "Prost" },
    ];
    let refs: Vec<&dyn HeadlineRule> = rules.iter().map(|r| r as &dyn HeadlineRule).collect();
    // Equal priorities keep the order the rules are listed in.
    assert_eq!(kinds(&for_season_with(&one_round(), &refs)), ["a", "b", "d"]);
}

#[test]
fn test_the_feed_is_the_same_every_time() {
    let facts = season(vec![
        race("r1", vec![me(3, false), ai("Senna", "Lotus", 1, false), ai("Prost", "McLaren", 2, false)]),
        race("r2", vec![me(1, false), ai("Senna", "Lotus", 2, false), ai("Prost", "McLaren", 3, false)]),
        race("r3", vec![me(5, true), ai("Senna", "Lotus", 2, false), ai("Prost", "McLaren", 1, false)]),
    ]);
    let feed = for_season(&facts);
    assert!(!feed.is_empty());
    assert_eq!(feed, for_season(&facts));
}

#[test]
fn test_every_real_rule_has_its_own_kind() {
    let mut all: Vec<&str> = RULES.iter().map(|r| r.kind()).collect();
    let n = all.len();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), n);
}

