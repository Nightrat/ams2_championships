use super::*;
use crate::career::story::facts::SessionFacts;
use crate::career::story::test_support::{ai, driver, me, race, season_for};

/// A factor that adds `amount` every race.
struct Fixed(f32);

impl MoodFactor for Fixed {
    fn name(&self) -> &'static str {
        "fixed"
    }
    fn delta(&self, _: &MoodContext) -> Option<MoodDelta> {
        Some(MoodDelta { reason: "mood.f.retired", amount: self.0 })
    }
}

/// An Osella season with Berg as teammate, one race per round.
fn osella(races: Vec<SessionFacts>) -> SeasonFacts {
    season_for(
        Some("Osella"),
        vec![driver("Allen Berg", "Osella", false)],
        races.into_iter().map(|r| vec![r]).collect(),
    )
}

fn plain(mine: u32) -> SessionFacts {
    race("r", vec![me(mine, false), ai("Allen Berg", "Osella", 9, false)])
}

fn moods(rounds: &[RoundMood]) -> Vec<f32> {
    rounds.iter().map(|m| (m.mood * 10.0).round() / 10.0).collect()
}

/// A context for one race, to ask a single factor about it.
fn ask(factor: &dyn MoodFactor, facts: &SeasonFacts, verdict: Option<Verdict>) -> Option<MoodDelta> {
    let before = SeasonSoFar::start(facts);
    let after = SeasonSoFar::whole(facts);
    let race = facts.rounds[0].race().unwrap();
    factor.delta(&MoodContext { facts, race, before: &before, after: &after, verdict })
}

#[test]
fn test_the_weekend_target_moves_the_mood_its_way() {
    let facts = osella(vec![plain(5)]);
    assert!(ask(&WeekendTarget, &facts, Some(Verdict::Met)).unwrap().amount > 0.0);
    assert!(ask(&WeekendTarget, &facts, Some(Verdict::Missed)).unwrap().amount < 0.0);
    assert_eq!(ask(&WeekendTarget, &facts, Some(Verdict::Void)), None);
    assert_eq!(ask(&WeekendTarget, &facts, None), None);
}

/// A 20-car race with the player at `mine` and the rest of the field around them.
fn full(mine: u32) -> SessionFacts {
    let mut rows = vec![me(mine, false)];
    rows.extend((1..=20).filter(|p| *p != mine).map(|p| ai(&format!("Driver {p}"), "Field", p, false)));
    race("r", rows)
}

#[test]
fn test_beating_the_car_pleases_by_degrees_and_trailing_it_does_not() {
    let mut facts = osella(vec![full(5)]);
    facts.expected.insert("Osella".into(), 13.0);
    let up = ask(&CarPerformance, &facts, None).unwrap();
    assert_eq!((up.reason, up.amount), ("mood.f.beat_car", 2.0));

    let mut behind = osella(vec![full(19)]);
    behind.expected.insert("Osella".into(), 3.0);
    let down = ask(&CarPerformance, &behind, None).unwrap();
    assert_eq!((down.reason, down.amount), ("mood.f.below_car", -4.0), "capped");

    let out = osella(vec![race("r", vec![me(1, true)])]);
    assert_eq!(ask(&CarPerformance, &out, None), None, "a retirement is not a finish");
}

#[test]
fn test_a_car_expected_behind_the_whole_grid_is_held_to_the_last_place() {
    // Expected 26th on a 2-car grid: finishing 1st gains one place, not 25.
    let mut facts = osella(vec![plain(1)]);
    facts.expected.insert("Osella".into(), 26.0);
    assert_eq!(ask(&CarPerformance, &facts, None), None, "a quarter point is too little to mention");
}

#[test]
fn test_the_teammate_duel_moves_the_mood_its_way() {
    assert!(ask(&TeammateDuel, &osella(vec![plain(5)]), None).unwrap().amount > 0.0);
    assert!(ask(&TeammateDuel, &osella(vec![plain(12)]), None).unwrap().amount < 0.0);
    let alone = osella(vec![race("r", vec![me(5, false)])]);
    assert_eq!(ask(&TeammateDuel, &alone, None), None, "no teammate started");
}

#[test]
fn test_the_objective_weighs_more_the_further_off_it_is() {
    let mut facts = osella(vec![race(
        "r",
        vec![ai("Prost", "McLaren", 1, false), ai("Senna", "Lotus", 2, false), me(6, false)],
    )]);
    facts.objective = Some(3);
    let on = {
        facts.objective = Some(3);
        ask(&ObjectivePace, &facts, None).unwrap().amount
    };
    // 3rd is the player; asking for 1st puts them 8 points off, under a race's worth (9).
    facts.objective = Some(1);
    let near = ask(&ObjectivePace, &facts, None).unwrap().amount;
    assert!(on > 0.0 && near < 0.0, "{on} {near}");
    facts.objective = None;
    assert_eq!(ask(&ObjectivePace, &facts, None), None, "no contract, no objective");
}

#[test]
fn test_a_retirement_costs_a_little() {
    let out = osella(vec![race("r", vec![me(4, true)])]);
    let d = ask(&Retirement, &out, None).unwrap();
    assert!(d.amount < 0.0 && d.amount > -5.0);
    assert_eq!(ask(&Retirement, &osella(vec![plain(4)]), None), None);
}

#[test]
fn test_the_mood_stays_between_0_and_100() {
    let facts = osella(vec![plain(5); 10]);
    assert!(replay_with(&facts, &[&Fixed(40.0)]).iter().all(|m| m.mood <= 100.0));
    assert!(replay_with(&facts, &[&Fixed(-40.0)]).iter().all(|m| m.mood >= 0.0));
}

#[test]
fn test_the_mood_eases_back_toward_neutral() {
    let facts = osella(vec![plain(5); 3]);
    // Nothing moves it: it stays where every career starts.
    assert_eq!(moods(&replay_with(&facts, &[])), [60.0, 60.0, 60.0]);
    // Pushed up every race, it settles where the push and the drift balance, short of 100.
    let pushed = replay_with(&osella(vec![plain(5); 40]), &[&Fixed(3.0)]);
    let last = pushed.last().unwrap().mood;
    assert!((last - 90.0).abs() < 0.5, "settles at START + 3 / REVERSION, got {last}");
}

#[test]
fn test_a_race_the_player_missed_leaves_the_mood_alone() {
    let missed = race("r", vec![ai("Allen Berg", "Osella", 1, false)]);
    let rounds = replay_with(&osella(vec![plain(5), missed, plain(5)]), &[&Fixed(1.0)]);
    assert_eq!(rounds.iter().map(|m| m.round).collect::<Vec<_>>(), [1, 3]);
}

#[test]
fn test_no_team_no_mood_and_the_same_career_always_gives_the_same_figure() {
    let facts = osella(vec![plain(5), plain(12), plain(3)]);
    assert_eq!(replay(&facts), replay(&facts));
    let mut free = facts.clone();
    free.player_team = None;
    assert!(replay(&free).is_empty());
}

#[test]
fn test_bands() {
    assert_eq!(Band::of(80.0), Band::Delighted);
    assert_eq!(Band::of(60.0), Band::Content);
    assert_eq!(Band::of(40.0), Band::Concerned);
    assert_eq!(Band::of(10.0), Band::LosingPatience);
}
