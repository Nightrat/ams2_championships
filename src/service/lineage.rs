//! Which team of a new season continues which team of the last one — proposed by constructor,
//! confirmed or corrected by the player, and read by the renewal. See [`crate::career::lineage`].

use serde::{Deserialize, Serialize};

use super::{bad, champ_class, champ_roster_file, conflict, not_found, App, AppResult};
use crate::career::contracts::{self, Finances, Standing};
use crate::career::data_store::{CareerData, Championship};
use crate::career::lineage::{self, Link, Source};

/// The lineage of one season, for the Manage tab to show and correct.
#[derive(Serialize, Debug, Default)]
pub struct LineageView {
    /// The completed season this one carries teams over from; `None` when there is none.
    pub previous: Option<PreviousSeason>,
    /// The player's team in that season — the link the renewal reads.
    pub your_team: Option<String>,
    /// One per team of the previous season, with the link in force and how it was settled.
    pub links: Vec<LinkView>,
    /// This season's teams, for choosing a successor.
    pub teams: Vec<String>,
    /// Signed or raced: the offers were built on these links, so they no longer change.
    pub locked: bool,
    /// Why there is nothing to link, when there is not.
    pub reason: Option<String>,
}

/// The season teams are carried over from.
#[derive(Serialize, Debug)]
pub struct PreviousSeason {
    pub id: String,
    pub name: String,
}

/// One previous team and its successor.
#[derive(Serialize, Debug)]
pub struct LinkView {
    pub from: String,
    pub to: Option<String>,
    pub source: Source,
}

/// The player's links, replacing any stored before.
#[derive(Deserialize, Debug, Default)]
pub struct LineagePatch {
    pub links: Vec<Link>,
}

/// How the last completed season's team carries into this one.
pub(super) struct Carried {
    /// The standing with its team named as this season's grid names it — what the offers read.
    pub standing: Standing,
    /// The previous team and its successor (`None`: nobody continues it), when this season has a
    /// previous one in its class.
    pub link: Option<(String, Option<String>)>,
}

/// The completed contracted season `champ` carries teams over from: the last completed one, when
/// it is another season in the same class. A series change is a fresh start, as renewals have
/// always treated it.
pub(super) fn previous_season<'a>(ledger: &Finances, data: &'a CareerData, champ: &Championship) -> Option<&'a Championship> {
    let last = ledger.seasons.iter().rev().find(|s| s.complete)?;
    let class = champ_class(champ);
    if last.champ_id == champ.id || class.is_empty() || last.class != class {
        return None;
    }
    data.championships.iter().find(|c| c.id == last.champ_id)
}

/// The team names a season's roster fields — its own copy once signed.
pub(super) fn roster_teams(app: &App, champ: &Championship) -> Vec<String> {
    champ_roster_file(&app.config_path, &app.active_path(), champ)
        .map(|f| crate::roster::custom_ai::list_teams(&f))
        .unwrap_or_default()
}

/// The standing the offers for `champ` are built on: the previous team translated through the
/// lineage, so a team renamed between rosters is still the one being renewed.
pub(super) fn carried(app: &App, data: &CareerData, ledger: &Finances, standing: &Standing, champ: &Championship) -> Carried {
    let (Some(prev), Some(team)) = (previous_season(ledger, data, champ), standing.incumbent.as_deref()) else {
        return Carried { standing: standing.clone(), link: None };
    };
    let successor = lineage::successor(&roster_teams(app, prev), &roster_teams(app, champ), &champ.team_lineage, team);
    let mut translated = standing.clone();
    // No successor means no renewal: the incumbency is dropped rather than left to match a team
    // of the same name the player said does not continue theirs.
    translated.incumbent = successor.clone();
    // How the team felt when that season ended decides the edge cases of a renewal and adds to
    // its pay — see `contracts::renews`.
    translated.mood = app.season_end_mood(data, prev);
    Carried {
        standing: translated,
        link: Some((team.to_string(), successor)),
    }
}

impl App {
    /// The lineage of championship `id`: what is proposed, what the player chose, what is open.
    pub fn lineage(&self, id: &str) -> AppResult<LineageView> {
        let data = self.data();
        let champ = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?;
        if !data.mode.uses_contracts() {
            return Ok(LineageView {
                reason: Some("Only a singleplayer career carries teams over: there are no contracts to renew.".into()),
                ..Default::default()
            });
        }
        let (ledger, standing) = super::career_standing(&self.config(), &data);
        let Some(prev) = previous_season(&ledger, &data, champ) else {
            return Ok(LineageView {
                reason: Some(
                    "No finished season in this series before this one, so there is no team to carry over."
                        .into(),
                ),
                ..Default::default()
            });
        };
        let previous = roster_teams(self, prev);
        let teams = roster_teams(self, champ);
        Ok(LineageView {
            links: lineage::resolve(&previous, &teams, &champ.team_lineage)
                .into_iter()
                .map(|(l, source)| LinkView { from: l.from, to: l.to, source })
                .collect(),
            previous: Some(PreviousSeason { id: prev.id.clone(), name: prev.name.clone() }),
            your_team: standing.incumbent,
            teams,
            locked: locked(&data, champ),
            reason: None,
        })
    }

    /// Stores the player's links for championship `id`, replacing any stored before.
    pub fn set_lineage(&self, id: &str, patch: LineagePatch) -> AppResult<LineageView> {
        {
            let mut data = self.store.write().unwrap();
            let champ = data
                .championships
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| not_found("not found"))?;
            if !data.mode.uses_contracts() {
                return Err(conflict("Only a singleplayer career carries teams over."));
            }
            if locked(&data, champ) {
                return Err(conflict(
                    "This season is signed, under way or finished: its offers were made on these links, so they no longer change.",
                ));
            }
            let (ledger, _) = super::career_standing(&self.config(), &data);
            let prev = previous_season(&ledger, &data, champ)
                .ok_or_else(|| conflict("No finished season in this series to carry teams over from."))?;
            let previous = roster_teams(self, prev);
            let teams = roster_teams(self, champ);
            for l in &patch.links {
                if !previous.contains(&l.from) {
                    return Err(bad(format!("{} was not a team last season.", l.from)));
                }
                if let Some(to) = &l.to {
                    if !teams.contains(to) {
                        return Err(bad(format!("{to} is not a team on this grid.")));
                    }
                }
            }
            let champ = data.championships.iter_mut().find(|c| c.id == id).expect("found above");
            champ.team_lineage = patch.links;
        }
        self.persist()?;
        self.lineage(id)
    }
}

/// Signed, raced or finished: the same point at which a season stops taking offers.
fn locked(data: &CareerData, champ: &Championship) -> bool {
    champ.status == crate::career::data_store::ChampionshipStatus::Final
        || contracts::for_championship(&data.contracts, &champ.id).is_some()
        || champ.rounds.iter().any(|r| !r.session_ids.is_empty())
}
