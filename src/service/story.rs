//! A season's story, as Career → Paddock shows it.
//!
//! This is where the story's inputs come from — the season's roster file (its signed copy when it
//! has one), the expected positions the rating gives its teams, the player's earlier seasons, the
//! career's mode — and where its results become views. The rules themselves live in
//! [`crate::career::story`] and never see a file or the config.

use std::collections::HashMap;

use serde::Serialize;

use super::{champ_roster_seats, not_found, App, AppResult};
use crate::career::contracts::TeamMood;
use crate::career::data_store::{CareerData, Championship};
use crate::career::story::{
    facts::{PlayerHistory, RosterDriver, SeasonFacts},
    biography,
    headlines::{self, points, Headline},
    mood::{self, Band},
    review,
    rival::Rival,
    tally::SeasonSoFar,
    targets::{self, Verdict},
    teammate::{self, HeadToHead},
    templates,
    title::{self, TitleFight, TitleStatus},
};
use crate::format::ordinal;

/// Everything the story says about one season. Each part carries its own reason for being empty,
/// so a part with nothing to show says why instead of rendering a blank card.
#[derive(Serialize, Debug)]
pub struct SeasonStory {
    pub title: TitleView,
    pub review: ReviewView,
    pub mood: MoodView,
    pub targets: TargetsView,
    pub interest: InterestView,
    pub rival: RivalView,
    pub teammates: TeammatesView,
    pub news: NewsView,
}

/// How the player's team sees them.
#[derive(Serialize, Debug, Default)]
pub struct MoodView {
    /// 0–100, rounded; `None` before the first race with the team.
    pub mood: Option<u32>,
    pub band: Option<Band>,
    /// "Concerned".
    pub label: Option<String>,
    /// "Osella wants to see more from you."
    pub says: Option<String>,
    /// "Down 9 after Monza: missed the weekend target and finished behind your teammate."
    pub why: Option<String>,
    /// The mood after each race, oldest first — the card draws it as bars.
    pub trend: Vec<MoodPoint>,
    /// Said while the team has been losing patience for several races running: it is
    /// considering its options. A warning only — nobody is sacked.
    pub warning: Option<String>,
    /// Why there is no mood, when there is none.
    pub reason: Option<String>,
}

/// The mood after one race.
#[derive(Serialize, Debug)]
pub struct MoodPoint {
    pub round: usize,
    pub mood: u32,
    pub band: Band,
}

/// The review of a finished season.
#[derive(Serialize, Debug, Default)]
pub struct ReviewView {
    /// What the season came to, a sentence each.
    pub lines: Vec<String>,
    /// Its biggest headlines, in order.
    pub headlines: Vec<Headline>,
    /// Why there is no review, when there is none.
    pub reason: Option<String>,
}

/// The player's career, season by season.
#[derive(Serialize, Debug, Default)]
pub struct CareerStoryView {
    /// In the order they were raced.
    pub seasons: Vec<CareerSeasonView>,
    /// The firsts, in the order they came.
    pub milestones: Vec<String>,
    /// The most recently raced finished season — what the Paddock reviews between seasons.
    pub latest_finished: Option<String>,
    /// Why there is no career to tell, when there is none.
    pub reason: Option<String>,
}

/// One season of the career.
#[derive(Serialize, Debug)]
pub struct CareerSeasonView {
    pub id: String,
    pub name: String,
    pub finished: bool,
    pub champion: bool,
    /// "Spirit-Honda · 8th of 24 · 14 points · 1 win".
    pub line: String,
}

/// Which teams on this season's grid would talk to the player about a seat today.
///
/// Mid-season the app cannot see next season's grid — the next roster is not even in the game's
/// folder until it replaces this one — so this is **this** grid, and says so. Real offers for
/// next season are made on next season's roster, when that season is created.
#[derive(Serialize, Debug, Default)]
pub struct InterestView {
    /// Teams that would pay to have the player, quickest car first; the player's own team left
    /// out. A team that would only sell a seat is not interested in the driver, only the money.
    pub teams: Vec<InterestedTeam>,
    /// What the list is and is not.
    pub note: Option<String>,
    /// Why there is no list yet, when there is none.
    pub reason: Option<String>,
}

/// One team that would talk.
#[derive(Serialize, Debug)]
pub struct InterestedTeam {
    pub team: String,
    /// Place in the grid's pace order, 1 = quickest car.
    pub pace: usize,
    /// Quicker than the car the player drives now — a step up.
    pub quicker: bool,
}

/// The share of the calendar run before other teams start looking: two thirds, the point a
/// season's shape is clear and next year's seats start being talked about.
const INTEREST_FROM: (u32, u32) = (2, 3);

/// The team's weekend targets for one season.
#[derive(Serialize, Debug, Default)]
pub struct TargetsView {
    /// "The team's target for round 9: finish 12th or better." Also shown on the Live tab.
    pub next: Option<String>,
    /// Every raced round's target with its verdict, newest first.
    pub judged: Vec<JudgedView>,
    /// "Targets met: 5 of 8." Void targets are not counted either way.
    pub record: Option<String>,
    /// Why there are no targets, when there are none.
    pub reason: Option<String>,
}

/// One raced round's target, judged.
#[derive(Serialize, Debug)]
pub struct JudgedView {
    pub round: usize,
    pub track: String,
    pub target: String,
    pub verdict: Verdict,
}

/// The player's rival for one season.
#[derive(Serialize, Debug, Default)]
pub struct RivalView {
    pub rival: Option<Rival>,
    /// Points the player is ahead of the rival in the championship; negative when behind.
    pub points_ahead: i32,
    /// The rivalry in a sentence.
    pub summary: Option<String>,
    /// Why there is no rival yet, when there is none.
    pub reason: Option<String>,
}

/// The title fight and the contract's objective for one season.
#[derive(Serialize, Debug, Default)]
pub struct TitleView {
    /// The standings with each driver's chances; `None` without a calendar.
    pub fight: Option<TitleFight>,
    /// "8 of 12 races run — 14 points behind Nelson Piquet": where the player stands, in a line.
    /// Also what the Live tab shows for the season being raced.
    pub summary: Option<String>,
    /// What would settle the title at the next round, when it can be settled there.
    pub clinch: Option<String>,
    /// Said once the planned races are all run but the season is still open.
    pub past_plan: Option<String>,
    /// How the player stands against the contract's target position.
    pub objective: Option<String>,
    /// Why there is no title fight, when there is none.
    pub reason: Option<String>,
}

/// The teammate head-to-head for one season.
#[derive(Serialize, Debug, Default)]
pub struct TeammatesView {
    /// The player's team, when the season has one.
    pub team: Option<String>,
    /// One per teammate, in roster order.
    pub head_to_head: Vec<HeadToHead>,
    /// The one-line version, heading the card; `None` until a session has been shared.
    pub summary: Option<String>,
    /// Why there is nothing to compare, when there is not.
    pub reason: Option<String>,
}

/// The Paddock news feed for one season, newest round first.
#[derive(Serialize, Debug, Default)]
pub struct NewsView {
    /// Only rounds that made news.
    pub rounds: Vec<NewsRound>,
    /// Why there is no news, when there is none.
    pub reason: Option<String>,
}

/// One round's headlines.
#[derive(Serialize, Debug)]
pub struct NewsRound {
    pub round: usize,
    pub track: String,
    pub headlines: Vec<Headline>,
}

impl App {
    /// The story of championship `id` as it stands.
    pub fn season_story(&self, id: &str) -> AppResult<SeasonStory> {
        let data = self.data();
        let champ = data
            .championships
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| not_found("not found"))?;
        // The rating is the dearest thing here; worked out once, it gives both the expected
        // positions the story reads and the market the interest card shows.
        let rated = if data.mode.uses_roster() {
            self.champ_eligibility(&self.active_path(), champ, &data)
        } else {
            None
        };
        let facts = self.season_facts_rated(&data, champ, rated.as_ref().map(|(_, e)| e.as_slice()));
        let finished = champ.status == crate::career::data_store::ChampionshipStatus::Final;
        Ok(SeasonStory {
            review: review_view(&facts, finished),
            title: title_view(&facts, finished),
            mood: mood_view(&facts, data.mode.uses_roster()),
            targets: targets_view(&facts, data.mode.uses_roster()),
            interest: self.interest_view(&data, champ, &facts, rated.as_ref()),
            rival: rival_view(&facts),
            teammates: teammates_view(&facts, data.mode.uses_roster()),
            news: news_view(&facts),
        })
    }

    /// Everything the story rules may know about `champ`.
    fn season_facts(&self, data: &CareerData, champ: &Championship) -> SeasonFacts {
        let rated = if data.mode.uses_roster() {
            self.champ_eligibility(&self.active_path(), champ, data)
        } else {
            None
        };
        self.season_facts_rated(data, champ, rated.as_ref().map(|(_, e)| e.as_slice()))
    }

    /// [`Self::season_facts`] with the season's team eligibility already worked out.
    fn season_facts_rated(
        &self,
        data: &CareerData,
        champ: &Championship,
        eligibility: Option<&[crate::career::driver_rating::TeamEligibility]>,
    ) -> SeasonFacts {
        // A view only: a stale read of the path changes nothing.
        let career = self.active_path();
        let uses_roster = data.mode.uses_roster();
        let roster = if uses_roster {
            champ_roster_seats(&self.config_path, &career, champ)
                .into_iter()
                .map(|s| RosterDriver {
                    name: s.driver,
                    team: s.team,
                    stand_in: s.stand_in,
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut facts = SeasonFacts::new(champ, &data.sessions, roster);
        // The other seasons only lend the player's earlier results, so they need no roster.
        let others: Vec<SeasonFacts> = data
            .championships
            .iter()
            .filter(|c| c.id != champ.id)
            .map(|c| SeasonFacts::new(c, &data.sessions, Vec::new()))
            .collect();
        facts.history = PlayerHistory::before(&facts, &others);
        facts.expected = eligibility
            .unwrap_or_default()
            .iter()
            .map(|t| (t.team.clone(), t.expected_position))
            .collect();
        if data.mode.uses_contracts() {
            facts.objective = crate::career::contracts::for_championship(&data.contracts, &champ.id)
                .and_then(|c| c.objective);
        }
        facts
    }
}

/// `finished` is whether the season is `Final` — it only decides whether the player is told to
/// finish it.
fn title_view(facts: &SeasonFacts, finished: bool) -> TitleView {
    let so_far = SeasonSoFar::whole(facts);
    let objective = title::objective(facts, &so_far).map(|o| {
        let pos = o.position.map(ordinal).unwrap_or_default();
        let target = ordinal(o.target);
        let key = match (&o.position, o.on_target, &o.holder) {
            (Some(_), true, _) => "objective.on",
            (Some(_), false, Some(_)) => "objective.off",
            (None, _, Some(_)) => "objective.unplaced",
            _ => "objective.open",
        };
        let values = [
            ("target", target),
            ("pos", pos),
            ("gap", points(o.gap)),
            ("holder", o.holder.unwrap_or_default()),
        ];
        templates::render(key, &values, 0)
    });
    let Some(fight) = title::fight(facts, &so_far) else {
        return TitleView {
            objective,
            reason: Some(
                "This season has no planned number of races, so there is no telling what is still \
                 to race for. Set Races on the Manage tab."
                    .to_string(),
            ),
            ..Default::default()
        };
    };
    TitleView {
        summary: Some(summary_line(&fight)),
        clinch: fight.clinch_next.as_ref().map(|c| {
            let conditions: Vec<String> = c
                .against
                .iter()
                .map(|m| {
                    let (key, gap) = match m.by {
                        by if by > 0 => ("title.cond.outscore", by),
                        0 => ("title.cond.match", 0),
                        by => ("title.cond.within", -by),
                    };
                    templates::render(key, &[("rival", m.rival.clone()), ("gap", points(gap))], 0)
                })
                .collect();
            templates::render(
                "title.clinch_next",
                &[("driver", c.leader.clone()), ("conditions", and_list(&conditions))],
                0,
            )
        }),
        past_plan: (!finished && fight.races_left == 0 && fight.leader().is_some())
            .then(|| templates::render("title.past_plan", &[("planned", fight.planned.to_string())], 0)),
        objective,
        fight: Some(fight),
        reason: None,
    }
}

/// Where the player stands, in one line: the calendar so far, then their place in the fight.
fn summary_line(f: &TitleFight) -> String {
    let progress = templates::render(
        "title.progress",
        &[("count", f.races_run.to_string()), ("planned", f.planned.to_string())],
        0,
    );
    let leader = f.leader().map(|l| l.name.clone()).unwrap_or_default();
    let standing = match (f.player(), f.leader()) {
        (_, None) => templates::render("title.no_races", &[], 0),
        (Some(me), _) if me.status == TitleStatus::Clinched => templates::render("title.champion", &[], 0),
        (Some(me), _) if me.status == TitleStatus::Eliminated => templates::render("title.eliminated", &[], 0),
        (Some(me), _) if me.position == 1 => match f.table.get(1) {
            Some(second) if second.points == me.points => {
                templates::render("title.leading_countback", &[("rival", second.name.clone())], 0)
            }
            Some(second) => templates::render("title.leading", &[("gap", points(me.points - second.points))], 0),
            None => templates::render("title.leading", &[("gap", points(me.points))], 0),
        },
        (Some(me), _) => templates::render("title.behind", &[("gap", points(me.behind)), ("leader", leader)], 0),
        (None, _) => templates::render("title.leader", &[("leader", leader)], 0),
    };
    format!("{progress} \u{2014} {standing}")
}

impl App {
    /// The player's career, season by season, with its firsts.
    pub fn career_story(&self) -> CareerStoryView {
        let data = self.data();
        // Only the player's results and team are read, so no roster and no rating are needed.
        let seasons: Vec<(SeasonFacts, bool)> = data
            .championships
            .iter()
            .map(|c| {
                (
                    SeasonFacts::new(c, &data.sessions, Vec::new()),
                    c.status == crate::career::data_store::ChampionshipStatus::Final,
                )
            })
            .collect();
        let bio = biography::biography(&seasons);
        if bio.seasons.is_empty() {
            return CareerStoryView {
                reason: Some("No season has been raced yet.".into()),
                ..Default::default()
            };
        }
        let name_of = |id: &str| {
            data.championships
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.name.clone())
                .unwrap_or_default()
        };
        let finished_ids: Vec<&str> = seasons.iter().filter(|(_, f)| *f).map(|(s, _)| s.champ_id.as_str()).collect();
        CareerStoryView {
            latest_finished: bio
                .seasons
                .iter()
                .rev()
                .find(|s| finished_ids.contains(&s.champ_id.as_str()))
                .map(|s| s.champ_id.clone()),
            milestones: bio
                .milestones
                .iter()
                .map(|m| {
                    templates::render(
                        &format!("career.{}", m.kind),
                        &[
                            ("track", m.track.clone()),
                            ("round", m.round.to_string()),
                            ("season", name_of(&m.champ_id)),
                        ],
                        0,
                    )
                })
                .collect(),
            seasons: bio
                .seasons
                .iter()
                .map(|s| CareerSeasonView {
                    name: name_of(&s.champ_id),
                    finished: finished_ids.contains(&s.champ_id.as_str()),
                    champion: s.champion,
                    line: career_line(s),
                    id: s.champ_id.clone(),
                })
                .collect(),
            reason: None,
        }
    }

    /// The interest card: from two thirds of the calendar, the teams on this grid whose offer the
    /// player would get today, worked out by the same `offers_for_with` the Manage tab uses.
    fn interest_view(
        &self,
        data: &CareerData,
        champ: &Championship,
        facts: &SeasonFacts,
        rated: Option<&(
            crate::career::driver_rating::Reputation,
            Vec<crate::career::driver_rating::TeamEligibility>,
        )>,
    ) -> InterestView {
        let reason = |r: String| InterestView { reason: Some(r), ..Default::default() };
        if !data.mode.uses_contracts() {
            return reason("Only a singleplayer career has a market for seats.".into());
        }
        let Some(planned) = facts.planned_rounds else {
            return reason("Teams look at drivers for next season once a season's shape is clear: set Races on the Manage tab.".into());
        };
        let run = SeasonSoFar::whole(facts).rounds_raced;
        let from = (planned * INTEREST_FROM.0).div_ceil(INTEREST_FROM.1);
        if run < from {
            let left = from - run;
            return reason(format!(
                "Other teams start looking at drivers for next season two thirds of the way through: {left} more race{}.",
                if left == 1 { "" } else { "s" }
            ));
        }
        let Some((rep, eligibility)) = rated else {
            return reason("This season has no roster to read a market from.".into());
        };
        let cfg = self.config();
        let (ledger, _) = super::career_standing(&cfg, data);
        let own = facts.player_team.as_deref();
        let own_pace = own.and_then(|t| eligibility.iter().position(|e| e.team == t));
        let offers = crate::career::contracts::offers_for_with(
            &champ.id,
            &super::champ_class(champ),
            rep.value,
            ledger.balance,
            eligibility,
            // As a newcomer: interest is what other teams think of the driver, not a renewal.
            &crate::career::contracts::Standing::default(),
            &cfg.offer_params(),
        );
        let teams: Vec<InterestedTeam> = offers
            .into_iter()
            .filter(|o| o.kind == crate::career::contracts::OfferKind::Paid && Some(o.team.as_str()) != own)
            .map(|o| InterestedTeam {
                quicker: own_pace.is_some_and(|p| o.rank < p),
                pace: o.rank + 1,
                team: o.team,
            })
            .collect();
        InterestView {
            note: Some(
                "Teams on this season's grid that would talk to you today. Next season's offers are \
                 made on next season's roster, when you create that season."
                    .into(),
            ),
            reason: teams.is_empty().then(|| "No other team on this grid would pay to have you yet.".to_string()),
            teams,
        }
    }

    /// How the team felt about the player when `champ` ended — the story's mood, handed to the
    /// contracts as a plain value. `None` when the season had no mood to work out (no team, no
    /// roster, or no race the player started).
    pub(super) fn season_end_mood(&self, data: &CareerData, champ: &Championship) -> Option<TeamMood> {
        if !data.mode.uses_roster() {
            return None;
        }
        let facts = self.season_facts(data, champ);
        mood::replay(&facts).last().map(|m| team_mood(m.band))
    }
}

/// One season of the career in a line: the team, the place, the points, the wins.
fn career_line(s: &biography::SeasonLine) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(team) = &s.team {
        parts.push(team.clone());
    }
    if s.champion {
        parts.push(templates::render("career.champion", &[], 0));
    } else if let Some(pos) = s.position {
        parts.push(templates::render(
            "career.place",
            &[("pos", ordinal(pos)), ("field", s.field.to_string())],
            0,
        ));
    }
    parts.push(points(s.points));
    match s.wins {
        0 => {}
        1 => parts.push(templates::render("career.win", &[], 0)),
        n => parts.push(templates::render("career.wins", &[("count", n.to_string())], 0)),
    }
    templates::render("career.line", &[("parts", parts.join(" \u{b7} "))], 0)
}

/// The review of a finished season, a sentence for each thing it came to.
fn review_view(facts: &SeasonFacts, finished: bool) -> ReviewView {
    if !finished {
        return ReviewView {
            reason: Some(templates::render("review.pending", &[], 0)),
            ..Default::default()
        };
    }
    let r = review::review(facts);
    let mut lines = Vec::new();
    if let Some(c) = &r.champion {
        lines.push(if c.player {
            templates::render("review.champion.player", &[("points", points(c.points))], 0)
        } else {
            templates::render("review.champion", &[("driver", c.name.clone()), ("points", points(c.points))], 0)
        });
    }
    if let Some(me) = r.player.as_ref().filter(|p| p.position > 1) {
        lines.push(templates::render(
            "review.position",
            &[
                ("pos", ordinal(me.position)),
                ("field", r.field.to_string()),
                ("points", points(me.points)),
            ],
            0,
        ));
    }
    if let Some(o) = r.objective.as_ref().filter(|o| o.position.is_some()) {
        let key = if o.on_target { "review.objective.met" } else { "review.objective.missed" };
        lines.push(templates::render(key, &[("target", ordinal(o.target))], 0));
    }
    if let Some(b) = &r.best {
        lines.push(templates::render(
            "review.best",
            &[("pos", ordinal(b.position)), ("track", b.track.clone()), ("round", b.round.to_string())],
            0,
        ));
    }
    if let Some(w) = r.worst.as_ref().filter(|w| Some(w.round) != r.best.as_ref().map(|b| b.round)) {
        let result = if w.dnf { "a retirement".to_string() } else { ordinal(w.position) };
        lines.push(templates::render(
            "review.worst",
            &[("result", result), ("track", w.track.clone()), ("round", w.round.to_string())],
            0,
        ));
    }
    if let Some(s) = summary(&r.teammates) {
        lines.push(format!("{s}."));
    }
    if let Some(rv) = &r.rival {
        lines.push(templates::render(
            "review.rival",
            &[
                ("driver", rv.name.clone()),
                ("won_times", times(rv.duel.player)),
                ("lost_times", times(rv.duel.rival)),
            ],
            0,
        ));
    }
    if let (Some(m), Some(team)) = (&r.mood, &facts.player_team) {
        let band = templates::render(&format!("mood.band.{}", m.band.key()), &[], 0).to_lowercase();
        lines.push(templates::render(
            "review.mood",
            &[
                ("team", team.clone()),
                ("band", band),
                ("end", (m.end.round() as u32).to_string()),
                ("high", (m.high.round() as u32).to_string()),
                ("low", (m.low.round() as u32).to_string()),
            ],
            0,
        ));
    }
    ReviewView {
        lines,
        headlines: r.headlines,
        reason: None,
    }
}

/// The story's band as the contracts know it.
fn team_mood(band: Band) -> TeamMood {
    match band {
        Band::Delighted => TeamMood::Delighted,
        Band::Content => TeamMood::Content,
        Band::Concerned => TeamMood::Concerned,
        Band::LosingPatience => TeamMood::LosingPatience,
    }
}

fn mood_view(facts: &SeasonFacts, uses_roster: bool) -> MoodView {
    let reason = |r: &str| MoodView {
        reason: Some(r.to_string()),
        ..Default::default()
    };
    if !uses_roster {
        return reason("A multiplayer career has no team to please.");
    }
    let Some(team) = facts.player_team.clone() else {
        return reason("No team yet. Once you drive for one, you will see how they rate you.");
    };
    if facts.roster.is_empty() {
        return reason("This season has no roster, so there is no team to read.");
    }
    let rounds = mood::replay(facts);
    let Some(last) = rounds.last() else {
        return reason(&format!("{team} is waiting to see you race."));
    };
    let band = last.band;
    let track = facts
        .rounds
        .iter()
        .find(|r| r.number == last.round)
        .and_then(|r| r.track())
        .unwrap_or_default()
        .to_string();
    let reasons: Vec<String> = last
        .deltas
        .iter()
        .map(|d| templates::render(d.reason, &[], 0))
        .collect();
    let change = last.mood.round() as i32 - last.was.round() as i32;
    let why = if reasons.is_empty() {
        templates::render("mood.why.quiet", &[("track", track)], 0)
    } else {
        let key = match change {
            c if c > 0 => "mood.why.up",
            c if c < 0 => "mood.why.down",
            _ => "mood.why.flat",
        };
        templates::render(
            key,
            &[("change", change.abs().to_string()), ("track", track), ("reasons", and_list(&reasons))],
            0,
        )
    };
    MoodView {
        mood: Some(last.mood.round() as u32),
        band: Some(band),
        label: Some(templates::render(&format!("mood.band.{}", band.key()), &[], 0)),
        says: Some(templates::render(&format!("mood.says.{}", band.key()), &[("team", team.clone())], 0)),
        why: Some(why),
        trend: rounds
            .iter()
            .map(|m| MoodPoint {
                round: m.round,
                mood: m.mood.round() as u32,
                band: m.band,
            })
            .collect(),
        warning: (mood::patience_run(&rounds) >= mood::WARNING_RACES)
            .then(|| templates::render("mood.warning", &[("team", team)], 0)),
        reason: None,
    }
}

fn targets_view(facts: &SeasonFacts, uses_roster: bool) -> TargetsView {
    let reason = |r: &str| TargetsView {
        reason: Some(r.to_string()),
        ..Default::default()
    };
    if !uses_roster {
        return reason("A multiplayer career has no team to set targets.");
    }
    if facts.roster.is_empty() {
        return reason("This season has no roster, so the team has nothing to set targets from.");
    }
    if facts.player_team.is_none() {
        return reason("No team yet. Once you drive for one, it sets a target for every race.");
    }
    let season = targets::season(facts);
    let track = |round: usize| {
        facts
            .rounds
            .iter()
            .find(|r| r.number == round)
            .and_then(|r| r.track())
            .unwrap_or_default()
            .to_string()
    };
    let met = season.judged.iter().filter(|j| j.verdict == Verdict::Met).count();
    let judged = season.judged.iter().filter(|j| j.verdict != Verdict::Void).count();
    TargetsView {
        next: season.next.map(|t| {
            templates::render(
                "target.next",
                &[("round", t.round.to_string()), ("target", t.text)],
                0,
            )
        }),
        record: (judged > 0).then(|| {
            templates::render(
                "target.record",
                &[("met", met.to_string()), ("judged", judged.to_string())],
                0,
            )
        }),
        judged: season
            .judged
            .into_iter()
            .rev()
            .map(|j| JudgedView {
                round: j.target.round,
                track: track(j.target.round),
                target: j.target.text,
                verdict: j.verdict,
            })
            .collect(),
        reason: None,
    }
}

fn rival_view(facts: &SeasonFacts) -> RivalView {
    let so_far = SeasonSoFar::whole(facts);
    let Some(rival) = so_far.rivalry.current() else {
        return RivalView {
            reason: Some(
                "No rival yet. One emerges after a couple of races, from whoever you keep finishing \
                 closest to."
                    .to_string(),
            ),
            ..Default::default()
        };
    };
    let points_of = |player: bool| {
        so_far
            .table()
            .iter()
            .find(|s| if player { s.player } else { !s.player && s.name == rival.name })
            .map_or(0, |s| s.points)
    };
    let ahead = points_of(true) - points_of(false);
    let standing = match ahead {
        0 => templates::render("rival.standing.level", &[], 0),
        a if a > 0 => templates::render("rival.standing.ahead", &[("gap", points(a))], 0),
        a => templates::render("rival.standing.behind", &[("gap", points(-a))], 0),
    };
    let key = match rival.duel.player.cmp(&rival.duel.rival) {
        std::cmp::Ordering::Greater => "rival.summary",
        std::cmp::Ordering::Less => "rival.summary.behind",
        std::cmp::Ordering::Equal => "rival.summary.level",
    };
    let summary = templates::render(
        key,
        &[
            ("since", rival.since_round.to_string()),
            ("won_times", times(rival.duel.player)),
            ("lost_times", times(rival.duel.rival)),
            ("standing", standing),
        ],
        0,
    );
    RivalView {
        rival: Some(rival),
        points_ahead: ahead,
        summary: Some(summary),
        reason: None,
    }
}

/// "once", "twice", "7 times".
fn times(n: u32) -> String {
    match n {
        1 => "once".to_string(),
        2 => "twice".to_string(),
        n => format!("{n} times"),
    }
}

/// "a", "a and b", "a, b and c".
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn teammates_view(facts: &SeasonFacts, uses_roster: bool) -> TeammatesView {
    let reason = |r: &str| TeammatesView {
        team: facts.player_team.clone(),
        reason: Some(r.to_string()),
        ..Default::default()
    };
    if !uses_roster {
        return reason("A multiplayer career has no teams, so there is no teammate to measure against.");
    }
    if facts.roster.is_empty() {
        return reason("This season has no roster to read its teams from.");
    }
    let Some(team) = facts.player_team.as_deref() else {
        return reason("No team yet. Your teammate is whoever drives the other car of the team you join.");
    };
    let head_to_head = teammate::head_to_head(facts);
    if head_to_head.is_empty() {
        return reason(&format!("{team} runs no other car this season."));
    }
    TeammatesView {
        team: Some(team.to_string()),
        summary: summary(&head_to_head),
        head_to_head,
        reason: None,
    }
}

/// "Against Riccardo Patrese: races 7–3, qualifying 6–4" — one clause per teammate the player has
/// shared a session with.
fn summary(all: &[HeadToHead]) -> Option<String> {
    let score = |key: &str, player: u32, other: u32| {
        templates::render(key, &[("player", player.to_string()), ("other", other.to_string())], 0)
    };
    let clauses: Vec<String> = all
        .iter()
        .filter(|h| h.races + h.qualifyings > 0)
        .map(|h| {
            let mut parts = Vec::new();
            if h.races > 0 {
                parts.push(score("teammate.races", h.race.player, h.race.teammate));
            }
            if h.qualifyings > 0 {
                parts.push(score("teammate.qualifying", h.qualifying.player, h.qualifying.teammate));
            }
            templates::render(
                "teammate.clause",
                &[("teammate", h.teammate.clone()), ("parts", parts.join(", "))],
                0,
            )
        })
        .collect();
    (!clauses.is_empty()).then(|| clauses.join(". "))
}

fn news_view(facts: &SeasonFacts) -> NewsView {
    let mut by_round: HashMap<usize, Vec<Headline>> = HashMap::new();
    for h in headlines::for_season(facts) {
        by_round.entry(h.round).or_default().push(h);
    }
    let rounds: Vec<NewsRound> = facts
        .rounds
        .iter()
        .rev()
        .filter_map(|r| {
            Some(NewsRound {
                round: r.number,
                track: r.track().unwrap_or_default().to_string(),
                headlines: by_round.remove(&r.number)?,
            })
        })
        .collect();
    let reason = rounds
        .is_empty()
        .then(|| "Nothing to report yet. The news fills in as rounds are raced.".to_string());
    NewsView { rounds, reason }
}
