//! Which team of a new season continues which team of the last one.
//!
//! A renewal is recognised by the team's name inside its class (see
//! [`crate::career::contracts::Standing`]), and rosters for consecutive seasons rename the teams
//! that changed engine: between the prepared 1983 and 1984 `F-Retro_Gen3` rosters, Spirit-Honda
//! becomes Spirit-Hart, Williams-Ford becomes Williams-Honda, McLaren-Ford becomes McLaren-TAG.
//! Without a link the driver who delivered for Spirit in 1983 is a stranger to Spirit in 1984.
//!
//! The app **proposes** links by constructor — the part of the name before the first `-` — and
//! links only where that is unambiguous on both sides. Anything it cannot settle (1983 runs a
//! Lotus-Ford *and* a Lotus-Renault) is left for the player, whose choices are **stored** on the
//! new season ([`Championship::team_lineage`](crate::career::data_store::Championship)). The
//! proposal itself is worked out again every time; only the choices are kept.

use serde::{Deserialize, Serialize};

/// One team of the previous season and the team of this season that continues it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub from: String,
    /// `None`: the team has no successor on this grid — a choice the player can make.
    pub to: Option<String>,
}

/// The constructor a team name belongs to: the part before the first `-`, or the whole name.
/// "Spirit-Hart" and "Spirit-Honda" are both "Spirit"; "Ferrari" is "Ferrari".
pub fn constructor(team: &str) -> &str {
    team.split('-').next().unwrap_or(team).trim()
}

/// Links proposed from the names alone. A team still on the grid under the same name links to
/// itself — which is how a renewal was recognised before links existed, so nothing that renewed
/// then stops renewing. Otherwise a previous team links to a current one when each is the only
/// team of that constructor on its own grid. One link per previous team, in its order;
/// unmatched ones carry `to: None`.
pub fn propose(previous: &[String], current: &[String]) -> Vec<Link> {
    let count = |teams: &[String], c: &str| teams.iter().filter(|t| constructor(t).eq_ignore_ascii_case(c)).count();
    previous
        .iter()
        .map(|from| {
            if current.contains(from) {
                return Link { from: from.clone(), to: Some(from.clone()) };
            }
            let c = constructor(from);
            let to = (count(previous, c) == 1 && count(current, c) == 1)
                .then(|| current.iter().find(|t| constructor(t).eq_ignore_ascii_case(c)).cloned())
                .flatten();
            Link { from: from.clone(), to }
        })
        .collect()
}

/// How a link was settled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The player chose it.
    Chosen,
    /// The constructors matched, and the player has not changed it.
    Proposed,
    /// Nothing matched and the player has not chosen: the team has no successor for now.
    Unmatched,
}

/// The link in force for each previous team: the player's choice where there is one, the
/// proposal otherwise.
pub fn resolve(previous: &[String], current: &[String], chosen: &[Link]) -> Vec<(Link, Source)> {
    propose(previous, current)
        .into_iter()
        .map(|proposed| match chosen.iter().find(|c| c.from == proposed.from) {
            Some(c) => (c.clone(), Source::Chosen),
            None if proposed.to.is_some() => (proposed, Source::Proposed),
            None => (proposed, Source::Unmatched),
        })
        .collect()
}

/// The team of this season that continues `team`, or `None` when it has none.
pub fn successor(previous: &[String], current: &[String], chosen: &[Link], team: &str) -> Option<String> {
    resolve(previous, current, chosen)
        .into_iter()
        .find(|(l, _)| l.from == team)
        .and_then(|(l, _)| l.to)
}

#[cfg(test)]
#[path = "../tests/career/lineage.rs"]
mod tests;
