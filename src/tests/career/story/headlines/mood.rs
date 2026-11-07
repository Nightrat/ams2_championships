use super::*;
use crate::career::story::facts::{SeasonFacts, SessionFacts};
use crate::career::story::headlines::for_season_with;
use crate::career::story::test_support::{ai, driver, me, race, season_for};

/// A 20-car race with the player 15th and the teammate Berg 3rd: whichever target the team set
/// — the top 7 for a car expected 5th, or beating Berg — it is missed, and Berg is ahead.
fn bad_day() -> SessionFacts {
    let mut rows = vec![me(15, false), ai("Allen Berg", "Osella", 3, false)];
    rows.extend((1..=20).filter(|p| *p != 15 && *p != 3).map(|p| ai(&format!("Driver {p}"), "Field", p, false)));
    race("r", rows)
}

fn season(races: usize) -> SeasonFacts {
    let mut roster = vec![driver("Allen Berg", "Osella", false)];
    roster.extend((1..=19).map(|p| driver(&format!("Driver {p}"), "Field", false)));
    let mut facts = season_for(Some("Osella"), roster, (0..races).map(|_| vec![bad_day()]).collect());
    facts.expected.insert("Osella".into(), 5.0);
    facts
}

#[test]
fn test_the_mood_crossing_into_another_band_is_news_once() {
    // -12.5 a race (target, teammate, ten places behind the car): 60 → 47.5 (concerned) →
    // 36.3 → 26.1 (losing patience).
    let feed = for_season_with(&season(4), &[&MoodShift]);
    let got: Vec<(usize, &str)> = feed.iter().map(|h| (h.round, h.key)).collect();
    assert_eq!(got, [(1, "mood.down.concerned"), (3, "mood.down.losing_patience")]);
    assert!(feed[0].text.contains("Osella"), "{}", feed[0].text);
    assert!(feed.iter().all(|h| h.player));
}

#[test]
fn test_the_team_considers_its_options_once_after_three_races_losing_patience() {
    // Losing patience from race 3 on (see above), so the third such race is race 5.
    let feed = for_season_with(&season(7), &[&ConsideringOptions]);
    let got: Vec<(usize, &str)> = feed.iter().map(|h| (h.round, h.key)).collect();
    assert_eq!(got, [(5, "mood.considering")]);
}
