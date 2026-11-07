//! Headlines from the title fight: the moment a championship is won, and the moment the player
//! can no longer win it. Both compare the fight before the round with the fight after it, so each
//! fires once, on the round it happened.

use super::{HeadlineDraft, HeadlineRule, RoundContext};
use crate::career::story::title::{fight, TitleStatus};

/// Someone — the player or not — clinching the championship.
pub struct TitleClinched;

impl HeadlineRule for TitleClinched {
    fn kind(&self) -> &'static str {
        "title_clinched"
    }
    fn priority(&self) -> u8 {
        98
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let was = fight(cx.facts, cx.before)
            .and_then(|f| f.leader().map(|l| (l.name.clone(), l.status)));
        let now = fight(cx.facts, cx.after)?;
        let champion = now.leader().filter(|l| l.status == TitleStatus::Clinched)?;
        if was == Some((champion.name.clone(), TitleStatus::Clinched)) {
            return None;
        }
        let s = cx.round.race()?;
        Some(HeadlineDraft {
            session_id: s.id.clone(),
            subject: champion.name.clone(),
            player: champion.player,
            key: "title.clinched",
            values: vec![("driver", champion.name.clone()), ("track", s.track.clone())],
        })
    }
}

/// The player's title chances ending.
pub struct PlayerOutOfTheFight;

impl HeadlineRule for PlayerOutOfTheFight {
    fn kind(&self) -> &'static str {
        "player_out_of_title_fight"
    }
    fn priority(&self) -> u8 {
        55
    }
    fn check(&self, cx: &RoundContext) -> Option<HeadlineDraft> {
        let was = fight(cx.facts, cx.before).and_then(|f| f.player().map(|p| p.status));
        if was == Some(TitleStatus::Eliminated) {
            return None;
        }
        let now = fight(cx.facts, cx.after)?;
        let me = now.player().filter(|p| p.status == TitleStatus::Eliminated)?;
        let s = cx.round.race()?;
        Some(HeadlineDraft {
            session_id: s.id.clone(),
            subject: me.name.clone(),
            player: true,
            key: "title.out",
            values: vec![("driver", me.name.clone()), ("track", s.track.clone())],
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/career/story/headlines/title.rs"]
mod tests;
