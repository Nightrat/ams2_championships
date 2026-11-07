//! Points table [9, 6, 4, 3, 2, 1]: a race left is worth 9.

use super::*;
use crate::career::story::facts::{DriverResult, SeasonFacts, SessionFacts};
use crate::career::story::test_support::{ai, me, race, season};

fn senna(pos: u32) -> DriverResult {
    ai("Ayrton Senna", "Lotus", pos, false)
}
fn prost(pos: u32) -> DriverResult {
    ai("Alain Prost", "McLaren", pos, false)
}
fn prost_out() -> DriverResult {
    ai("Alain Prost", "McLaren", 9, true)
}

fn planned(races: Vec<SessionFacts>, planned: u32) -> SeasonFacts {
    let mut facts = season(races);
    facts.planned_rounds = Some(planned);
    facts
}

fn fight_of(facts: &SeasonFacts) -> TitleFight {
    fight(facts, &SeasonSoFar::whole(facts)).unwrap()
}

fn status_of(f: &TitleFight, name: &str) -> TitleStatus {
    f.table.iter().find(|c| c.name == name).unwrap().status
}

#[test]
fn test_no_calendar_no_title_fight() {
    let facts = season(vec![race("r1", vec![senna(1)])]);
    assert!(fight(&facts, &SeasonSoFar::whole(&facts)).is_none());
}

#[test]
fn test_a_rival_who_can_only_draw_level_loses_on_the_countback() {
    // Senna 18 with two wins; Prost 9 from a 2nd and a 4th. One race left: Prost can only reach
    // 18 with one win, and two wins beat one.
    let facts = planned(
        vec![race("r1", vec![senna(1), prost(2)]), race("r2", vec![senna(1), prost(4)])],
        3,
    );
    let f = fight_of(&facts);
    assert_eq!((f.races_run, f.races_left, f.points_left), (2, 1, 9));
    assert_eq!(status_of(&f, "Ayrton Senna"), TitleStatus::Clinched);
    assert_eq!(status_of(&f, "Alain Prost"), TitleStatus::Eliminated);
}

#[test]
fn test_a_tie_only_the_name_would_settle_keeps_both_in_the_fight() {
    // Senna 18 with two wins; Prost 9 with one. If Prost wins the last race and Senna scores
    // nothing, both have 18 points and two wins — no FIA rule separates them.
    let facts = planned(
        vec![
            race("r1", vec![senna(1), prost_out()]),
            race("r2", vec![senna(1), prost_out()]),
            race("r3", vec![prost(1), ai("Ayrton Senna", "Lotus", 2, true)]),
        ],
        4,
    );
    let f = fight_of(&facts);
    assert_eq!(status_of(&f, "Ayrton Senna"), TitleStatus::InContention);
    assert_eq!(status_of(&f, "Alain Prost"), TitleStatus::InContention);
}

#[test]
fn test_a_season_run_past_its_calendar_is_judged_on_the_standings() {
    let facts = planned(
        vec![race("r1", vec![senna(1), prost(2)]), race("r2", vec![prost(1), senna(3)]), race("r3", vec![senna(1), prost(2)])],
        2,
    );
    let f = fight_of(&facts);
    assert_eq!((f.races_run, f.races_left, f.points_left), (3, 0, 0));
    assert_eq!(status_of(&f, "Ayrton Senna"), TitleStatus::Clinched);
    assert!(f.clinch_next.is_none());
}

#[test]
fn test_the_leader_can_clinch_next_round_by_a_margin() {
    let facts = planned(vec![race("r1", vec![senna(1), prost(3), me(6, false)])], 3);
    let f = fight_of(&facts);
    let chance = f.clinch_next.expect("winning the next race with Prost scoring nothing settles it");
    assert_eq!(chance.leader, "Ayrton Senna");
    // 9 against 4 with a race to spare after the next: Senna must lead by more than 9.
    assert_eq!(chance.against[0], Margin { rival: "Alain Prost".into(), by: 5 });
    assert_eq!(f.table[0].behind, 0);
    assert_eq!(f.table[1].behind, 5);
}

#[test]
fn test_no_clinch_chance_while_too_much_is_left() {
    let facts = planned(vec![race("r1", vec![senna(1), prost(2)])], 5);
    assert!(fight_of(&facts).clinch_next.is_none());
}

#[test]
fn test_the_objective_is_read_against_the_standings() {
    let mut facts = season(vec![race("r1", vec![senna(1), prost(2), ai("Piquet", "Williams", 3, false), me(5, false)])]);
    facts.objective = Some(3);
    let so_far = SeasonSoFar::whole(&facts);
    let o = objective(&facts, &so_far).unwrap();
    assert_eq!(o.position, Some(4));
    assert!(!o.on_target);
    assert_eq!(o.holder.as_deref(), Some("Piquet"));
    assert_eq!(o.gap, 2);

    facts.objective = Some(4);
    let o = objective(&facts, &so_far).unwrap();
    assert!(o.on_target);
    assert_eq!((o.holder, o.gap), (None, 0));

    facts.objective = None;
    assert!(objective(&facts, &so_far).is_none());
}
