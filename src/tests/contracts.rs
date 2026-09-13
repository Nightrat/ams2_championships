use super::*;
use crate::driver_rating::{TeamEligibility, Tier};

fn elig(team: &str, tier: Tier, required: f32, expected_position: f32) -> TeamEligibility {
    TeamEligibility {
        team: team.into(),
        tier,
        required,
        expected_position,
        incumbent_skill: None,
    }
}

/// Fastest car first — the order `team_eligibility` returns and `offers_for` reads as rank.
fn grid() -> Vec<TeamEligibility> {
    vec![
        elig("Williams", Tier::Locked, 90.0, 1.5),
        elig("Brabham", Tier::OfferPossible, 62.0, 3.5),
        elig("Osella", Tier::Available, 20.0, 5.5),
    ]
}

/// `n` teams the driver has already earned, every one asking for exactly `REQ` so the salary
/// curve can be read without leverage moving it.
const REQ: f32 = 50.0;

fn open_grid(n: usize) -> Vec<TeamEligibility> {
    (0..n)
        .map(|i| {
            elig(
                &format!("Team{i}"),
                Tier::Available,
                REQ,
                1.5 + 2.0 * i as f32,
            )
        })
        .collect()
}

/// A rookie's view of a five-car grid: everything but the slowest car is out of reach, which is
/// the only shape in which a pay-driver seat exists at all. With the default one-third share the
/// back two sell, so Osella is the pay seat and AGS is earned.
fn skint_grid() -> Vec<TeamEligibility> {
    vec![
        elig("Williams", Tier::Locked, 90.0, 1.5),
        elig("Brabham", Tier::Locked, 80.0, 3.5),
        elig("Zakspeed", Tier::Locked, 65.0, 5.5),
        elig("Osella", Tier::Locked, 60.0, 7.5),
        elig("AGS", Tier::Available, 20.0, 9.5),
    ]
}

/// The class every hand-built grid in these tests belongs to.
const CLASS: &str = "F-Classic_Gen1";
/// A different series entirely — a 1960s grid, twenty years away from [`CLASS`].
const OTHER_CLASS: &str = "F-Vintage_Gen1";

fn params() -> OfferParams {
    OfferParams::default()
}

fn offer<'a>(offers: &'a [Offer], team: &str) -> &'a Offer {
    offers
        .iter()
        .find(|o| o.team == team)
        .unwrap_or_else(|| panic!("no offer from {team}"))
}

// ── Which teams offer at all ─────────────────────────────────────────────────

#[test]
fn test_only_the_back_of_the_grid_sells_a_seat() {
    // The rule this whole mechanic turns on. Osella spent 1986 asking its drivers to bring
    // sponsorship because it was broke; Williams did not, and no cheque would have opened that
    // seat. A front-runner out of reach makes no offer at all.
    let offers = offers_for("c1", 30.0, &skint_grid(), &params());
    let teams: Vec<&str> = offers.iter().map(|o| o.team.as_str()).collect();
    assert_eq!(teams, vec!["Osella", "AGS"]);

    let osella = offer(&offers, "Osella");
    assert_eq!(osella.kind, OfferKind::Pay);
    assert!(osella.buy_in > 0, "a bought seat has a price");
    assert_eq!(offer(&offers, "AGS").kind, OfferKind::Paid);
}

#[test]
fn test_a_front_running_team_is_never_for_sale() {
    // Even against a huge shortfall and a huge budget, the quick cars stay shut.
    for rep in [0.0, 30.0, 60.0] {
        let offers = offers_for("c1", rep, &skint_grid(), &params());
        for team in ["Williams", "Brabham", "Zakspeed"] {
            assert!(
                !offers.iter().any(|o| o.team == team),
                "{team} must not be buyable at rating {rep}"
            );
        }
    }
}

#[test]
fn test_zero_buy_in_takes_locked_teams_off_the_table() {
    let p = OfferParams {
        buy_in_per_point: 0,
        ..params()
    };
    let offers = offers_for("c1", 30.0, &skint_grid(), &p);
    assert_eq!(
        offers.iter().map(|o| o.team.as_str()).collect::<Vec<_>>(),
        vec!["AGS"],
        "with pay-driver seats off, every team out of reach makes no offer"
    );
}

#[test]
fn test_a_zero_share_also_takes_every_seat_off_the_market() {
    let p = OfferParams {
        pay_driver_share: 0.0,
        ..params()
    };
    let offers = offers_for("c1", 30.0, &skint_grid(), &p);
    assert!(offers.iter().all(|o| o.kind != OfferKind::Pay));
}

#[test]
fn test_a_wider_share_puts_more_of_the_grid_up_for_sale() {
    let p = OfferParams {
        pay_driver_share: 1.0,
        ..params()
    };
    let offers = offers_for("c1", 30.0, &skint_grid(), &p);
    assert_eq!(offers.len(), 5, "the whole grid sells at a share of one");
    assert_eq!(offer(&offers, "Williams").kind, OfferKind::Pay);
}

#[test]
fn test_tier_decides_offer_kind() {
    let offers = offers_for("c1", 62.0, &grid(), &params());
    assert_eq!(offer(&offers, "Brabham").kind, OfferKind::Paid);
    assert_eq!(offer(&offers, "Osella").kind, OfferKind::Paid);
}

#[test]
fn test_rank_is_the_grid_position_not_the_offer_index() {
    // Williams is dropped, but Brabham is still the second-fastest car on the grid. Renumbering
    // the survivors would pay a midfield team like a front-runner.
    let offers = offers_for("c1", 62.0, &grid(), &params());
    assert_eq!(offer(&offers, "Brabham").rank, 1);
    assert_eq!(offer(&offers, "Osella").rank, 2);
}

#[test]
fn test_empty_grid_yields_no_offers() {
    assert!(offers_for("c1", 80.0, &[], &params()).is_empty());
}

// ── Money ────────────────────────────────────────────────────────────────────

#[test]
fn test_salary_falls_down_the_grid() {
    let offers = offers_for("c1", REQ, &open_grid(8), &params());
    assert_eq!(offers.len(), 8);
    for pair in offers.windows(2) {
        assert!(
            pair[0].salary > pair[1].salary,
            "{} ({}) must out-pay {} ({})",
            pair[0].team,
            pair[0].salary,
            pair[1].team,
            pair[1].salary
        );
    }
}

#[test]
fn test_the_ends_of_the_grid_sit_on_the_configured_rates() {
    // Reputation exactly on the bar, so nothing but jitter separates the offer from the rate.
    let p = params();
    let offers = offers_for("c1", REQ, &open_grid(6), &p);
    let within =
        |got: i64, want: i64| (got as f32 - want as f32).abs() <= SALARY_JITTER * want as f32 + 1.0;
    assert!(
        within(offers[0].salary, p.top_salary),
        "top salary {}",
        offers[0].salary
    );
    assert!(
        within(offers[5].salary, p.floor_salary),
        "floor salary {}",
        offers[5].salary
    );
}

#[test]
fn test_a_one_team_grid_pays_the_top_rate() {
    // There is nothing to rank a lone team against, so it must not land on the floor rate.
    let p = params();
    let offers = offers_for("c1", REQ, &open_grid(1), &p);
    assert!(offers[0].salary as f32 > 0.9 * p.top_salary as f32);
}

#[test]
fn test_leverage_pays_more() {
    // Standing clear of the bar is worth money. It used to buy a longer deal too; deals are
    // single-season now, so pay is all the leverage there is.
    let p = params();
    let g = open_grid(4);
    let on_the_bar = offers_for("c1", REQ, &g, &p);
    let well_clear = offers_for("c1", REQ + LEVERAGE_RANGE, &g, &p);
    assert!(well_clear[0].salary > on_the_bar[0].salary);
}

#[test]
fn test_leverage_is_capped() {
    let p = params();
    let g = open_grid(4);
    let clear = offers_for("c1", REQ + LEVERAGE_RANGE, &g, &p);
    let absurd = offers_for("c1", 100.0, &g, &p);
    assert_eq!(clear[0].salary, absurd[0].salary);
}

#[test]
fn test_clearing_the_bar_and_coming_within_reach_are_the_same_deal() {
    // There is no separate "trial" any more. At the same rating, a team whose bar has just been
    // cleared and one still just out of reach offer identical terms — what separates a good
    // driver from a marginal one is the leverage below, not a different class of contract.
    let p = params();
    let over = vec![elig("Brabham", Tier::Available, REQ, 3.5)];
    let under = vec![elig("Brabham", Tier::OfferPossible, REQ, 3.5)];

    let a = &offers_for("c1", REQ, &over, &p)[0];
    let b = &offers_for("c1", REQ, &under, &p)[0];
    assert_eq!(a, b);
    assert_eq!(a.kind, OfferKind::Paid);
}

#[test]
fn test_a_bought_seat_never_earns_leverage() {
    let p = params();
    // A rating above the bar is leverage on an earned seat; on a bought one it means nothing,
    // because a seat is only ever for sale to someone who has not cleared the bar.
    let g = vec![
        elig("Fast", Tier::Available, 10.0, 1.5),
        elig("Osella", Tier::Locked, 90.0, 5.5),
    ];
    let a = offers_for("c1", REQ, &g, &p);
    let b = offers_for("c1", REQ + LEVERAGE_RANGE, &g, &p);
    assert_eq!(offer(&a, "Osella").salary, offer(&b, "Osella").salary);
    assert_eq!(offer(&a, "Osella").kind, OfferKind::Pay);
}

// ── Objectives ───────────────────────────────────────────────────────────────

#[test]
fn test_objective_comes_from_what_the_car_should_do() {
    let p = params();
    // A ten-car field, so nothing here is close enough to the back to be vacuous.
    let g: Vec<TeamEligibility> = vec![
        elig("Front", Tier::Available, REQ, 1.5),
        elig("Mid", Tier::OfferPossible, REQ, 5.5),
    ];
    let mut g = g;
    g.push(elig("Rear", Tier::Available, REQ, 9.5));
    let offers = offers_for("c1", REQ, &g, &p);

    // An earned seat: what the car should manage, plus the slack a fair signing carries. Both
    // of these are earned, whether the bar was cleared outright or only come within reach of.
    assert_eq!(
        offer(&offers, "Front").objective,
        Some(2 + p.objective_slack)
    );
    assert_eq!(offer(&offers, "Mid").objective, Some(6 + p.objective_slack));
}

#[test]
fn test_a_backmarker_asks_for_nothing() {
    // The slowest car's target plus slack is past the back of the grid, so every finish would
    // clear it. A team with nothing to ask for does not ask.
    let offers = offers_for("c1", 62.0, &grid(), &params());
    assert_eq!(offer(&offers, "Osella").objective, None);
}

#[test]
fn test_zero_slack_asks_for_exactly_the_car() {
    let p = OfferParams {
        objective_slack: 0,
        ..params()
    };
    let g = vec![
        elig("Front", Tier::Available, REQ, 1.5),
        elig("Rear", Tier::Available, REQ, 9.5),
    ];
    assert_eq!(offers_for("c1", REQ, &g, &p)[0].objective, Some(2));
}

// ── Determinism ──────────────────────────────────────────────────────────────

#[test]
fn test_offers_are_stable_across_regeneration() {
    // Nothing is stored, so the same career must produce the same offers every time it is asked.
    let g = grid();
    assert_eq!(
        offers_for("c1", 62.0, &g, &params()),
        offers_for("c1", 62.0, &g, &params())
    );
}

#[test]
fn test_a_different_season_reads_differently() {
    let g = open_grid(4);
    let a = offers_for("1774647213912", REQ, &g, &params());
    let b = offers_for("1777758943816", REQ, &g, &params());

    assert!(
        a.iter().zip(&b).any(|(x, y)| x.salary != y.salary),
        "two seasons must not offer identical money"
    );
    // Only the money moves: what a team will have, for how long, and what it asks for are
    // decisions about the driver, not a roll of the dice.
    for (x, y) in a.iter().zip(&b) {
        assert_eq!((x.kind, x.objective), (y.kind, y.objective));
    }
}

#[test]
fn test_hash_separates_its_parts() {
    assert_ne!(hash64(&["ab", "c"]), hash64(&["a", "bc"]));
}

#[test]
fn test_hash_is_pinned() {
    // Salaries hang off this. Changing the hash silently re-rolls every team's terms in every
    // existing career, so it is pinned rather than left to whatever the implementation becomes.
    assert_eq!(hash64(&["c1", "Brabham"]), 0x4e86_46be_3ed8_0686);
}

// ── Signing ──────────────────────────────────────────────────────────────────

#[test]
fn test_from_offer_freezes_the_terms() {
    let offers = offers_for("c1", 62.0, &grid(), &params());
    let o = offer(&offers, "Osella");
    let c = Contract::from_offer("c1", o, 1_700_000_000);

    assert_eq!(c.champ_id, "c1");
    assert_eq!(c.team, "Osella");
    assert_eq!(c.signed_at, 1_700_000_000);
    assert_eq!((c.salary, c.objective), (o.salary, o.objective));
    assert_eq!(c.bought_for, 0, "the seat was earned, not bought");
}

#[test]
fn test_terms_survive_the_rating_moving_underneath_them() {
    // The point of persisting a contract: regenerating offers after the rating changes gives a
    // different deal, but the one already signed is history and must not follow it.
    let g = open_grid(4);
    let signed = Contract::from_offer("c1", &offers_for("c1", REQ, &g, &params())[0], 1);
    let now = &offers_for("c1", REQ + LEVERAGE_RANGE, &g, &params())[0];
    assert_ne!(signed.salary, now.salary);
}

#[test]
fn test_for_championship_finds_the_right_contract() {
    let a = Contract {
        champ_id: "c1".into(),
        team: "Osella".into(),
        signed_at: 1,
        salary: 100,
        objective: None,
        bought_for: 0,
    };
    let b = Contract {
        champ_id: "c2".into(),
        team: "Brabham".into(),
        ..a.clone()
    };
    let all = vec![a, b];
    assert_eq!(
        for_championship(&all, "c2").map(|c| c.team.as_str()),
        Some("Brabham")
    );
    assert_eq!(for_championship(&all, "c9"), None);
}

// ── Persistence ──────────────────────────────────────────────────────────────

#[test]
fn test_a_save_written_before_contracts_still_loads() {
    let json = r#"{"sessions":[],"championships":[]}"#;
    let data: crate::data_store::CareerData = serde_json::from_str(json).unwrap();
    assert!(data.contracts.is_empty());
}

#[test]
fn test_a_contract_round_trips() {
    let c = Contract {
        champ_id: "c1".into(),
        team: "Brabham".into(),
        signed_at: 1_700_000_000,
        salary: 750_000,
        objective: Some(4),
        bought_for: 0,
    };
    let back: Contract = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(back, c);
}

#[test]
fn test_a_hand_written_contract_may_omit_the_defaulted_fields() {
    // config.json and the save file are both hand-edited often enough that a partial record has
    // to load rather than take the whole career down with it.
    let c: Contract =
        serde_json::from_str(r#"{"champ_id":"c1","team":"Brabham","salary":1}"#).unwrap();
    assert_eq!((c.signed_at, c.objective, c.bought_for), (0, None, 0));
}

#[test]
fn test_a_contract_written_when_deals_ran_for_years_still_loads() {
    // Deals are single-season now. A save from when they were not still carries `seasons`, and
    // serde must ignore it rather than refuse the whole career — it never meant anything
    // mechanically even while it was written.
    let c: Contract = serde_json::from_str(
        r#"{"champ_id":"c1","team":"Williams","signed_at":7,"seasons":3,"salary":500,"objective":4,"bought_for":0}"#,
    )
    .unwrap();
    assert_eq!(c.team, "Williams");
    assert_eq!((c.salary, c.objective), (500, Some(4)));
}

// ── Against the reference career ─────────────────────────────────────────────
//
// The same fixture the rating module snapshots, so offers are exercised against a real 72-session
// career and the real shipped rosters rather than only hand-built grids. Expected values are a
// snapshot of current behaviour — changing the economy is *supposed* to move them.

fn reference_career() -> crate::data_store::CareerData {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tests/fixtures/career_reference.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture missing")).unwrap()
}

const AI_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/custom_ai_files_with_perf_scalars"
);

/// The reference driver's rating, and the F-Classic_Gen1 grid it opens.
fn reference_eligibility() -> (f32, Vec<TeamEligibility>) {
    use crate::driver_rating::{
        assigned_sessions, compute_reputation_global, team_eligibility, RatingContext,
    };

    let dir = std::path::Path::new(AI_DIR);
    let contexts: Vec<RatingContext> = crate::custom_ai::class_performance(dir)
        .into_iter()
        .map(|perf| {
            let path = dir.join(format!("{}.xml", perf.class));
            let pace = perf
                .cars
                .iter()
                .map(|c| (c.team.clone(), c.pace_delta_pct))
                .collect();
            RatingContext::new(&perf.class, crate::custom_ai::parse_seats(&path), &pace)
        })
        .collect();

    let data = reference_career();
    let rated = assigned_sessions(&data.championships, &data.sessions);
    let rep = compute_reputation_global(Some("Nightrat"), &rated, &contexts, None).value;

    let own = contexts
        .iter()
        .find(|c| c.class == "F-Classic_Gen1")
        .expect("fixture class");
    let path = dir.join("F-Classic_Gen1.xml");
    let skills = crate::custom_ai::parse_team_skills(&path);
    (rep, team_eligibility(rep, &own.expected, &skills))
}

#[test]
fn test_reference_career_offers_snapshot() {
    let (rep, eligibility) = reference_eligibility();
    // Pinned by the rating module's own snapshot; repeated here so a failure here says which of
    // the two moved.
    assert!((rep - 57.16).abs() < 0.5, "rating {rep}");

    let offers = offers_for("1777758943816", rep, &eligibility, &params());
    // Five of fourteen 1986 seats are available in any sense: four on merit, plus the one team
    // behind them that is broke enough to sell. The front nine are simply shut.
    assert_eq!(
        offers.iter().map(|o| o.team.as_str()).collect::<Vec<_>>(),
        vec!["Lola", "Minardi", "Zakspeed", "Osella", "AGS"]
    );
    let earned = offers.iter().filter(|o| o.kind != OfferKind::Pay);
    assert_eq!(
        earned.clone().count(),
        4,
        "a mid-career rating must not open the whole 1986 grid"
    );
    assert!(
        offers.iter().any(|o| o.kind == OfferKind::Paid),
        "and must open something"
    );
    // Nothing this career has earned carries a price, and everything it has not does.
    for o in &offers {
        assert_eq!(
            o.buy_in > 0,
            o.kind == OfferKind::Pay,
            "{} price and kind disagree",
            o.team
        );
    }

    // Every offer is internally coherent, whatever the economy is tuned to.
    let p = params();
    for o in &offers {
        assert!(o.salary > 0, "{} pays nothing", o.team);
        assert!(
            o.salary <= (p.top_salary as f32 * 1.5) as i64,
            "{} overpays",
            o.team
        );
        assert!(o.rank < eligibility.len());
    }

    // Ranks come out ascending because eligibility is ordered by car pace and offers preserve it.
    let ranks: Vec<usize> = offers.iter().map(|o| o.rank).collect();
    assert!(ranks.windows(2).all(|w| w[0] < w[1]), "ranks {ranks:?}");

    // What the rating earns is a contiguous run from the slowest car up — nothing further
    // forward may come within reach while a slower car is still out of it.
    let merit: Vec<&str> = earned.map(|o| o.team.as_str()).collect();
    assert_eq!(merit, vec!["Minardi", "Zakspeed", "Osella", "AGS"]);
    assert_eq!(offers.last().unwrap().rank, eligibility.len() - 1);

    // Minardi, the seat this career actually took in 1987, is the furthest forward of them, and
    // the team pays for it like any other earned seat — there is no separate trial tier.
    let minardi = offer(&offers, "Minardi");
    assert_eq!(minardi.kind, OfferKind::Paid);
    assert!(!minardi.renewal, "nothing was held going into this season");
    assert_eq!(minardi.buy_in, 0);

    // The front of the grid is not for sale at any price. This is the fix for a model that had
    // it backwards: a Williams seat used to carry a price tag, when in reality a front-running
    // team has no budget hole to plug and loses more by fielding a slow driver than a cheque
    // covers. Pay drivers lived at the back — Osella, AGS and Coloni in this very era.
    for shut in [
        "Williams", "Brabham", "Lotus", "McLaren", "Ferrari", "Tyrrell",
    ] {
        assert!(
            !offers.iter().any(|o| o.team == shut),
            "{shut} must not be buyable"
        );
    }

    // Lola is the one seat money opens: the slowest car this rating cannot quite reach.
    let lola = offer(&offers, "Lola");
    assert_eq!(lola.kind, OfferKind::Pay);
    assert_eq!(lola.rank, 9);
    // Sponsorship is the shortfall alone, so it stays in the range a season's prize money can
    // eventually cover rather than several times the best salary on the grid.
    assert_eq!(
        lola.buy_in,
        ((lola.required - rep) * params().buy_in_per_point as f32) as i64
    );
    assert!(lola.buy_in < params().top_salary, "{}", lola.buy_in);
}

#[test]
fn test_reference_career_offers_are_stable() {
    let (rep, eligibility) = reference_eligibility();
    assert_eq!(
        offers_for("1777758943816", rep, &eligibility, &params()),
        offers_for("1777758943816", rep, &eligibility, &params())
    );
}

// ── Finances ─────────────────────────────────────────────────────────────────

use crate::data_store::{Round, SessionResult};

fn prize_params() -> PrizeParams {
    PrizeParams::default()
}

/// A race whose finishing order is `order`, with `player` carrying the recorder's own flag.
fn race(id: &str, order: &[&str], player: Option<&str>) -> RecordedSession {
    RecordedSession {
        id: id.into(),
        recorded_at: 0,
        track: "Monza".into(),
        track_variation: String::new(),
        car_name: String::new(),
        car_class: "F-Classic_Gen1".into(),
        session_type: 5,
        results: order
            .iter()
            .enumerate()
            .map(|(i, n)| SessionResult {
                name: (*n).into(),
                car_name: String::new(),
                car_class: "F-Classic_Gen1".into(),
                race_position: i as u32 + 1,
                laps_completed: 10,
                fastest_lap: 90.0,
                last_lap: 90.0,
                dnf: false,
                is_player: player == Some(*n),
            })
            .collect(),
        lap_chart: vec![],
    }
}

fn season(id: &str, status: ChampionshipStatus, session_ids: &[&str]) -> Championship {
    Championship {
        id: id.into(),
        name: format!("Season {id}"),
        status,
        points_system: vec![25, 18, 15, 12, 10, 8],
        manufacturer_scoring: false,
        rounds: session_ids
            .iter()
            .map(|s| Round {
                session_ids: vec![(*s).into()],
            })
            .collect(),
        session_ids: vec![],
        custom_ai_file: Some("F-Classic_Gen1.xml".into()),
        player_team: Some("Osella".into()),
    }
}

fn contract(champ_id: &str, salary: i64, objective: Option<u32>) -> Contract {
    Contract {
        champ_id: champ_id.into(),
        team: "Osella".into(),
        signed_at: 1,
        salary,
        objective,
        bought_for: 0,
    }
}

/// One finished season the player won outright, from a four-car field.
fn won_season() -> (Vec<Contract>, Vec<Championship>, Vec<RecordedSession>) {
    let s = race(
        "s1",
        &["Nightrat", "Piquet", "Mansell", "Berg"],
        Some("Nightrat"),
    );
    (
        vec![contract("c1", 500_000, Some(3))],
        vec![season("c1", ChampionshipStatus::Final, &["s1"])],
        vec![s],
    )
}

#[test]
fn test_prize_is_the_champion_rate_for_a_win() {
    let p = prize_params();
    assert_eq!(prize(1, 20, &p), p.champion_prize);
}

#[test]
fn test_the_last_classified_driver_earns_the_floor() {
    let p = prize_params();
    assert_eq!(prize(20, 20, &p), p.floor_prize);
}

#[test]
fn test_prize_scales_with_the_size_of_the_field() {
    // Fifth of six is nearly last; fifth of twenty-six is a good season. A flat table by
    // position alone would pay them the same.
    let p = prize_params();
    assert!(prize(5, 26, &p) > prize(5, 6, &p));
}

#[test]
fn test_prize_falls_with_position() {
    let p = prize_params();
    let pay: Vec<i64> = (1..=10).map(|pos| prize(pos, 10, &p)).collect();
    assert!(pay.windows(2).all(|w| w[0] > w[1]), "{pay:?}");
}

#[test]
fn test_prize_for_an_unplaced_driver_is_nothing() {
    assert_eq!(prize(0, 10, &prize_params()), 0);
}

#[test]
fn test_a_one_driver_field_is_paid_as_a_win() {
    let p = prize_params();
    assert_eq!(prize(1, 1, &p), p.champion_prize);
}

#[test]
fn test_a_completed_season_pays_salary_and_prize() {
    let (c, ch, s) = won_season();
    let f = finances(&c, &ch, &s, None, 0, &prize_params());

    assert_eq!(f.seasons.len(), 1);
    let led = &f.seasons[0];
    assert!(led.complete);
    assert_eq!(led.position, Some(1));
    assert_eq!(led.field, 4);
    assert_eq!(led.salary, 500_000);
    assert_eq!(led.prize, prize_params().champion_prize);
    assert_eq!(f.earned, led.salary + led.prize);
    assert_eq!(f.spent, 0);
    assert_eq!(f.balance, f.earned);
}

#[test]
fn test_an_unfinished_season_pays_nothing_yet() {
    // Rounds are added as they are raced, so a career never declares how long a season is meant
    // to be. There is nothing to pro-rate against, so nothing is credited until it is over.
    let (c, mut ch, s) = won_season();
    ch[0].status = ChampionshipStatus::Active;
    let f = finances(&c, &ch, &s, None, 0, &prize_params());

    let led = &f.seasons[0];
    assert!(!led.complete);
    assert_eq!((led.salary, led.prize), (0, 0));
    // The standings are still reported — the season just has not paid out.
    assert_eq!(led.position, Some(1));
    assert_eq!(f.balance, 0);
}

#[test]
fn test_a_season_without_a_contract_is_not_in_the_ledger() {
    let (_, ch, s) = won_season();
    let f = finances(&[], &ch, &s, None, 0, &prize_params());
    assert!(f.seasons.is_empty());
    assert_eq!(f.balance, 0);
}

#[test]
fn test_an_orphaned_contract_keeps_its_spending() {
    // The championship was deleted; the money paid to take the seat was still paid.
    let mut c = contract("gone", 500_000, None);
    c.bought_for = 900_000;
    let f = finances(&[c], &[], &[], None, 0, &prize_params());

    assert_eq!(f.seasons.len(), 1);
    assert_eq!(f.seasons[0].name, "", "no championship to name it after");
    assert_eq!(f.seasons[0].salary, 0, "a season never raced pays no wage");
    assert_eq!(f.spent, 900_000);
    assert_eq!(f.balance, -900_000);
}

#[test]
fn test_balance_may_go_negative() {
    // Reassigning sessions can move standings that were already paid out, so spending can end
    // up ahead of earnings. Debt is the honest answer; a clamped balance would hide it.
    let (mut c, ch, s) = won_season();
    c[0].bought_for = 50_000_000;
    let f = finances(&c, &ch, &s, None, 0, &prize_params());
    assert!(f.balance < 0, "balance {}", f.balance);
    assert_eq!(f.balance, f.earned - f.spent);
}

#[test]
fn test_objective_is_decided_against_the_standings() {
    let (c, ch, s) = won_season();
    // Won the title against a target of third.
    assert_eq!(
        finances(&c, &ch, &s, None, 0, &prize_params()).seasons[0].objective_met,
        Some(true)
    );

    let missed = vec![contract("c1", 500_000, Some(1))];
    let back = vec![race(
        "s1",
        &["Piquet", "Mansell", "Berg", "Nightrat"],
        Some("Nightrat"),
    )];
    let f = finances(&missed, &ch, &back, None, 0, &prize_params());
    assert_eq!(f.seasons[0].position, Some(4));
    assert_eq!(f.seasons[0].objective_met, Some(false));
}

#[test]
fn test_an_objective_is_undecided_until_the_season_ends() {
    let (c, mut ch, s) = won_season();
    ch[0].status = ChampionshipStatus::Progress;
    assert_eq!(
        finances(&c, &ch, &s, None, 0, &prize_params()).seasons[0].objective_met,
        None
    );
}

#[test]
fn test_a_season_with_no_target_is_never_failed() {
    let c = vec![contract("c1", 500_000, None)];
    let (_, ch, s) = won_season();
    assert_eq!(
        finances(&c, &ch, &s, None, 0, &prize_params()).seasons[0].objective_met,
        None
    );
}

#[test]
fn test_the_player_is_found_by_the_recorders_own_flag() {
    let (c, ch, s) = won_season();
    // No name supplied: the `is_player` flag alone has to identify the driver.
    assert_eq!(
        finances(&c, &ch, &s, None, 0, &prize_params()).seasons[0].position,
        Some(1)
    );
}

#[test]
fn test_a_named_driver_overrides_the_flag() {
    let (c, ch, s) = won_season();
    let f = finances(&c, &ch, &s, Some("Berg"), 0, &prize_params());
    assert_eq!(f.seasons[0].position, Some(4));
}

#[test]
fn test_an_unidentifiable_player_still_draws_a_salary() {
    // A wrong name would be worse than a missing one, so an unidentified driver simply wins no
    // prize money — the wage and the spending are still real.
    let (c, ch, _) = won_season();
    let anonymous = vec![race("s1", &["Piquet", "Mansell"], None)];
    let f = finances(&c, &ch, &anonymous, None, 0, &prize_params());

    assert_eq!(f.seasons[0].position, None);
    assert_eq!(f.seasons[0].prize, 0);
    assert_eq!(f.seasons[0].salary, 500_000);
}

#[test]
fn test_seasons_follow_championship_order() {
    let s = race("s1", &["Nightrat", "Berg"], Some("Nightrat"));
    let champs = vec![
        season("c1", ChampionshipStatus::Final, &["s1"]),
        season("c2", ChampionshipStatus::Final, &[]),
    ];
    // Deliberately supplied out of order: the ledger follows the save's championship order, not
    // the order contracts happen to sit in.
    let c = vec![contract("c2", 1, None), contract("c1", 1, None)];
    let f = finances(&c, &champs, &[s], None, 0, &prize_params());
    let ids: Vec<&str> = f.seasons.iter().map(|l| l.champ_id.as_str()).collect();
    assert_eq!(ids, vec!["c1", "c2"]);
}

#[test]
fn test_reference_career_finances() {
    // The real career, with a contract synthesised for each of its championships, so the ledger
    // is exercised against genuine standings rather than a two-car grid.
    let data = reference_career();
    let contracts: Vec<Contract> = data
        .championships
        .iter()
        .map(|c| contract(&c.id, 400_000, Some(5)))
        .collect();
    let f = finances(
        &contracts,
        &data.championships,
        &data.sessions,
        None,
        0,
        &prize_params(),
    );

    assert_eq!(f.seasons.len(), data.championships.len());
    assert_eq!(f.spent, 0, "nothing was bought");
    assert_eq!(f.balance, f.earned);

    // Exactly one of the fixture's five championships is `Final`, and it is the only one that
    // has paid anything — the other four are raced but unfinished.
    let paid: Vec<&SeasonLedger> = f.seasons.iter().filter(|l| l.complete).collect();
    assert_eq!(paid.len(), 1, "completed seasons");
    let won = paid[0];
    assert_eq!(won.name, "Nightrat F1 1986");
    assert_eq!(won.position, Some(1), "the player won it");
    assert_eq!(won.field, 29);
    assert_eq!(won.prize, prize_params().champion_prize);
    assert_eq!(
        won.objective_met,
        Some(true),
        "a target of 5th, won outright"
    );
    assert_eq!(f.earned, won.salary + won.prize);

    // Standings still resolve for the unfinished seasons — they simply have not paid out yet.
    for led in f.seasons.iter().filter(|l| !l.complete && l.field > 0) {
        assert!(led.position.is_some(), "{} has no placing", led.name);
        assert_eq!((led.salary, led.prize), (0, 0), "{} paid early", led.name);
    }
}

#[test]
fn test_reference_career_stops_paying_if_a_season_reopens() {
    // The inverse of the rule: reopening the one completed season takes its payout back, because
    // nothing is banked — every figure is re-derived from the championship as it now stands.
    let data = reference_career();
    let contracts: Vec<Contract> = data
        .championships
        .iter()
        .map(|c| contract(&c.id, 400_000, Some(5)))
        .collect();
    let mut champs = data.championships.clone();
    champs[0].status = ChampionshipStatus::Progress;

    let f = finances(
        &contracts,
        &champs,
        &data.sessions,
        None,
        0,
        &prize_params(),
    );
    assert_eq!(f.earned, 0, "a reopened season is unpaid again");
    assert_eq!(f.seasons[0].position, Some(1), "the result is still there");
}

// ── Buying a seat ────────────────────────────────────────────────────────────

#[test]
fn test_only_a_bought_seat_carries_a_price() {
    for o in offers_for("c1", 62.0, &grid(), &params()) {
        assert_eq!(
            o.buy_in > 0,
            o.kind == OfferKind::Pay,
            "{} price and kind disagree",
            o.team
        );
    }
}

#[test]
fn test_buy_in_rises_with_the_shortfall() {
    let p = params();
    // Fastest first, as eligibility always is, with a seat the driver already has on merit — so
    // the last-resort discount never fires and the price on the back car is the honest one.
    let g = vec![
        elig("Fast", Tier::Available, 10.0, 1.5),
        elig("Mid", Tier::Available, 10.0, 3.5),
        elig("Osella", Tier::Locked, 90.0, 5.5),
    ];
    let close = offer(&offers_for("c1", 80.0, &g, &p), "Osella").buy_in;
    let far = offer(&offers_for("c1", 40.0, &g, &p), "Osella").buy_in;
    assert!(far > close, "{far} should cost more than {close}");
}

#[test]
fn test_the_price_is_the_shortfall_and_nothing_else() {
    // Two selling teams the same distance out of reach cost the same. The price plugs a budget
    // hole, and every team that sells is at the back already — there is no car quality left to
    // price in. An earlier version added a season's wage, which made the *quickest* car the
    // dearest to buy into; backwards, since those seats are not for sale at any price.
    let p = params();
    // Six teams, so the default one-third share puts the back *two* on the market.
    let g = vec![
        elig("T0", Tier::Available, 10.0, 1.5),
        elig("T1", Tier::Available, 10.0, 3.5),
        elig("T2", Tier::Available, 10.0, 5.5),
        elig("T3", Tier::Available, 10.0, 7.5),
        elig("SlowA", Tier::Locked, 80.0, 9.5),
        elig("SlowB", Tier::Locked, 80.0, 11.5),
    ];
    let offers = offers_for("c1", 50.0, &g, &p);
    assert_eq!(offer(&offers, "SlowA").kind, OfferKind::Pay);
    assert_eq!(offer(&offers, "SlowB").kind, OfferKind::Pay);
    assert_eq!(
        offer(&offers, "SlowA").buy_in,
        offer(&offers, "SlowB").buy_in
    );
    // And it is exactly the shortfall, at the configured rate.
    assert_eq!(
        offer(&offers, "SlowA").buy_in,
        (30.0 * p.buy_in_per_point as f32) as i64
    );
}

#[test]
fn test_a_bought_seat_has_no_slack_in_its_target() {
    let p = params();
    // The pay seat sits one place off the back, so its target is not vacuous.
    let g = vec![
        elig("T0", Tier::Available, 10.0, 1.5),
        elig("T1", Tier::Available, 10.0, 3.5),
        elig("T2", Tier::Available, 10.0, 5.5),
        elig("T3", Tier::Available, 10.0, 7.5),
        elig("Osella", Tier::Locked, 90.0, 9.5),
        elig("AGS", Tier::Available, 10.0, 11.5),
    ];
    let o = offers_for("c1", 50.0, &g, &p);
    let o = offer(&o, "Osella");
    assert_eq!(o.kind, OfferKind::Pay);
    // Round(9.5) = 10, and no slack: a bought seat is told to deliver what the car can do.
    assert_eq!(o.objective, Some(10));
}

#[test]
fn test_a_bought_seat_is_still_paid() {
    // The team is being paid to have the driver; it does not also stop paying them. Without a
    // wage the buy-in could never be recovered and one purchase would end a career.
    let g = vec![elig("Williams", Tier::Locked, 90.0, 1.5)];
    assert!(offers_for("c1", 50.0, &g, &params())[0].salary > 0);
}

// ── Standing ─────────────────────────────────────────────────────────────────

/// A ledger of completed seasons at the given teams, oldest first, each meeting its target
/// unless named in `missed`.
fn ledger_of(teams: &[&str], missed: &[usize]) -> Finances {
    let entries: Vec<(&str, &str)> = teams.iter().map(|t| (*t, CLASS)).collect();
    ledger_of_in(&entries, missed)
}

/// Completed seasons as `(team, class)` pairs, oldest first. A team name only identifies a team
/// within its own class, so the pairing is what an incumbency is really made of.
fn ledger_of_in(entries: &[(&str, &str)], missed: &[usize]) -> Finances {
    let seasons = entries
        .iter()
        .enumerate()
        .map(|(i, (team, class))| SeasonLedger {
            champ_id: format!("c{i}"),
            name: format!("Season {i}"),
            team: (*team).into(),
            class: (*class).into(),
            salary: 0,
            prize: 0,
            bought_for: 0,
            position: Some(1),
            field: 10,
            objective: Some(5),
            objective_met: Some(!missed.contains(&i)),
            complete: true,
        })
        .collect();
    Finances {
        seasons,
        ..Default::default()
    }
}

#[test]
fn test_standing_is_empty_without_a_completed_season() {
    assert_eq!(standing(&Finances::default()), Standing::default());
}

#[test]
fn test_standing_ignores_a_season_still_being_raced() {
    // The deal a renewal follows has not run its course yet, so it makes nobody an incumbent.
    let mut led = ledger_of(&["Osella"], &[]);
    led.seasons[0].complete = false;
    assert_eq!(standing(&led).incumbent, None);
}

#[test]
fn test_standing_reads_the_most_recent_completed_season() {
    let s = standing(&ledger_of(&["Osella", "Minardi"], &[]));
    assert_eq!(s.incumbent.as_deref(), Some("Minardi"));
    assert_eq!(s.delivered, Some(true));
    assert_eq!(s.tenure, 1);
}

#[test]
fn test_standing_counts_consecutive_seasons_at_one_team() {
    let s = standing(&ledger_of(&["Osella", "Osella", "Osella"], &[]));
    assert_eq!(s.tenure, 3);
}

#[test]
fn test_standing_tenure_breaks_when_the_driver_moves_away_and_back() {
    // A driver who left and came back is a returning signing, not a servant of long standing.
    let s = standing(&ledger_of(&["Osella", "Minardi", "Osella"], &[]));
    assert_eq!(s.incumbent.as_deref(), Some("Osella"));
    assert_eq!(s.tenure, 1);
}

#[test]
fn test_standing_carries_a_missed_target() {
    let s = standing(&ledger_of(&["Osella"], &[0]));
    assert_eq!(s.delivered, Some(false));
}

// ── Renewals ─────────────────────────────────────────────────────────────────

fn served(team: &str, delivered: Option<bool>, tenure: u32) -> Standing {
    served_in(team, CLASS, delivered, tenure)
}

fn served_in(team: &str, class: &str, delivered: Option<bool>, tenure: u32) -> Standing {
    Standing {
        incumbent: Some(team.into()),
        class: class.into(),
        delivered,
        tenure,
    }
}

#[test]
fn test_delivering_earns_a_renewal_whatever_the_rating_says() {
    // Williams is locked on merit; the seat is kept anyway. Re-earning it every winter would
    // make the objective decorative.
    let offers = offers_for_with(
        "c1",
        CLASS,
        40.0,
        0,
        &grid(),
        &served("Williams", Some(true), 1),
        &params(),
    );
    let w = offer(&offers, "Williams");
    assert_eq!(w.kind, OfferKind::Paid);
    assert_eq!(w.buy_in, 0, "a seat held is not a seat bought");
}

#[test]
fn test_a_renewal_pays_more_than_the_open_market() {
    let p = params();
    let g = vec![elig("Osella", Tier::Available, REQ, 5.5)];
    let open = offers_for("c1", REQ, &g, &p)[0].salary;
    let renewed = offers_for_with(
        "c1",
        CLASS,
        REQ,
        0,
        &g,
        &served("Osella", Some(true), 1),
        &p,
    )[0]
    .salary;
    assert!(renewed > open, "renewal {renewed} against market {open}");
}

#[test]
fn test_loyalty_grows_with_tenure_and_then_stops() {
    let p = params();
    let g = vec![elig("Osella", Tier::Available, REQ, 5.5)];
    let pay = |t: u32| {
        offers_for_with(
            "c1",
            CLASS,
            REQ,
            0,
            &g,
            &served("Osella", Some(true), t),
            &p,
        )[0]
        .salary
    };
    assert!(pay(1) < pay(2) && pay(2) < pay(3));
    assert_eq!(pay(3), pay(9), "loyalty is capped, not compounding forever");
}

#[test]
fn test_missing_the_target_drops_the_driver_back_to_merit() {
    let p = params();
    let offers = offers_for_with(
        "c1",
        CLASS,
        40.0,
        0,
        &grid(),
        &served("Williams", Some(false), 3),
        &p,
    );
    // No renewal, and Williams is a front-runner that does not sell — so the seat is simply
    // gone. Failing to deliver at a top team really does cost you the drive.
    assert!(!offers.iter().any(|o| o.team == "Williams"));

    // At the back it is softer: the team that dropped you will still take your sponsorship.
    let back = offers_for_with(
        "c1",
        CLASS,
        30.0,
        0,
        &skint_grid(),
        &served("Osella", Some(false), 2),
        &p,
    );
    assert_eq!(offer(&back, "Osella").kind, OfferKind::Pay);
}

#[test]
fn test_a_dropped_driver_keeps_a_seat_they_had_earned_anyway() {
    // Being dropped is not a punishment on top of the rating — it just stops the seat being
    // free. A team the driver clears on merit still offers.
    let p = params();
    let g = vec![elig("Osella", Tier::Available, REQ, 5.5)];
    let o = offers_for_with(
        "c1",
        CLASS,
        REQ,
        0,
        &g,
        &served("Osella", Some(false), 2),
        &p,
    );
    assert_eq!(o[0].kind, OfferKind::Paid);
}

#[test]
fn test_a_deal_that_set_no_target_cannot_be_failed() {
    // A backmarker team asks for nothing, so it has no grounds to drop anyone.
    let p = params();
    let offers = offers_for_with(
        "c1",
        CLASS,
        40.0,
        0,
        &grid(),
        &served("Williams", None, 1),
        &p,
    );
    assert_eq!(offer(&offers, "Williams").kind, OfferKind::Paid);
}

#[test]
fn test_a_renewal_without_a_target_earns_no_loyalty_rise() {
    // Keeping the seat is not the same as having delivered in it; only a met target pays more.
    let p = params();
    let g = vec![elig("Osella", Tier::Available, REQ, 5.5)];
    let plain = offers_for("c1", REQ, &g, &p)[0].salary;
    let kept = offers_for_with("c1", CLASS, REQ, 0, &g, &served("Osella", None, 3), &p)[0].salary;
    assert_eq!(kept, plain);
}

#[test]
fn test_a_renewal_applies_only_to_the_incumbent() {
    let p = params();
    let offers = offers_for_with(
        "c1",
        CLASS,
        62.0,
        0,
        &grid(),
        &served("Williams", Some(true), 2),
        &p,
    );
    assert_eq!(offer(&offers, "Brabham").kind, OfferKind::Paid);
    assert_eq!(offer(&offers, "Osella").kind, OfferKind::Paid);
}

#[test]
fn test_offers_for_is_offers_for_with_on_an_empty_standing() {
    let g = grid();
    assert_eq!(
        offers_for("c1", 62.0, &g, &params()),
        offers_for_with("c1", CLASS, 62.0, 0, &g, &Standing::default(), &params())
    );
}

#[test]
fn test_reference_career_renews_the_seat_it_delivered_in() {
    // End to end on real data: the fixture's completed 1986 season was won from a Brabham, and
    // a season won is a season delivered — so Brabham re-signs, though the rating alone is
    // nowhere near the bar its 1986 car asks for.
    let data = reference_career();
    let contracts: Vec<Contract> = data
        .championships
        .iter()
        .map(|c| contract(&c.id, 400_000, Some(5)))
        .collect();
    let ledger = finances(
        &contracts,
        &data.championships,
        &data.sessions,
        None,
        0,
        &prize_params(),
    );
    let mut s = standing(&ledger);
    // The synthesised contracts all name Osella; the real 1986 seat was a Brabham.
    s.incumbent = Some("Brabham".into());

    let (rep, eligibility) = reference_eligibility();
    let offers = offers_for_with("1777758943816", CLASS, rep, 0, &eligibility, &s, &params());
    assert_eq!(
        s.delivered,
        Some(true),
        "the fixture won its one full season"
    );

    let brabham = offer(&offers, "Brabham");
    assert_eq!(brabham.kind, OfferKind::Paid);
    assert!(
        brabham.required > rep,
        "and not because the rating reached it"
    );
    assert_eq!(brabham.buy_in, 0);
}

// ── An incumbency does not follow a driver between series ────────────────────

#[test]
fn test_a_seat_is_only_held_inside_its_own_series() {
    // The bug this rule exists for. "Ferrari" appears in seven of the eight shipped rosters,
    // spanning thirty years, so matching on the team name alone would renew a 1967 drive into a
    // 1990 car — and a renewal is granted regardless of rating, so it would hand over a seat the
    // career never came close to earning.
    let p = params();
    let g = vec![
        elig("Ferrari", Tier::Locked, 95.0, 1.5),
        elig("Minardi", Tier::Available, 20.0, 5.5),
    ];
    let won_elsewhere = served_in("Ferrari", OTHER_CLASS, Some(true), 3);

    let offers = offers_for_with("c1", CLASS, 40.0, 0, &g, &won_elsewhere, &p);
    assert!(
        !offers.iter().any(|o| o.team == "Ferrari"),
        "a Ferrari drive in another series must not open this one: {offers:?}"
    );

    // Same standing, same grid, in the series it was actually earned in: the seat is held.
    let home = offers_for_with("c1", OTHER_CLASS, 40.0, 0, &g, &won_elsewhere, &p);
    assert_eq!(offer(&home, "Ferrari").kind, OfferKind::Paid);
}

#[test]
fn test_an_unknown_class_holds_no_seat_either_way() {
    // A championship with no roster cannot be rated and so has no offers; a contract whose
    // championship was deleted has no class left. Neither may match the other by being blank.
    let p = params();
    let g = vec![elig("Osella", Tier::Locked, 90.0, 5.5)];
    let nowhere = served_in("Osella", "", Some(true), 2);
    assert!(offers_for_with("c1", "", 40.0, 0, &g, &nowhere, &p)
        .iter()
        .all(|o| o.kind != OfferKind::Paid));
    assert!(offers_for_with("c1", CLASS, 40.0, 0, &g, &nowhere, &p)
        .iter()
        .all(|o| o.kind != OfferKind::Paid));
}

#[test]
fn test_standing_carries_the_class_of_the_last_completed_season() {
    let s = standing(&ledger_of_in(&[("Osella", CLASS)], &[]));
    assert_eq!(s.incumbent.as_deref(), Some("Osella"));
    assert_eq!(s.class, CLASS);
}

#[test]
fn test_tenure_breaks_when_the_driver_changes_series() {
    // Two seasons at "Ferrari" that are not two seasons anywhere: a 1967 Ferrari and a 1990 one
    // share nothing but a name, so the second is a fresh signing, not a second year served.
    let s = standing(&ledger_of_in(
        &[("Ferrari", OTHER_CLASS), ("Ferrari", CLASS)],
        &[],
    ));
    assert_eq!(s.class, CLASS);
    assert_eq!(s.tenure, 1, "not a servant of long standing");

    // Consecutive seasons in one series still count.
    let stayed = standing(&ledger_of_in(
        &[("Ferrari", CLASS), ("Ferrari", CLASS)],
        &[],
    ));
    assert_eq!(stayed.tenure, 2);
}

#[test]
fn test_changing_series_is_a_fresh_start_not_a_punishment() {
    // Leaving a series costs the renewal, not the rating. Everything the driver has earned on
    // merit is still on the table in the new one.
    let p = params();
    let g = grid();
    let elsewhere = served_in("Williams", OTHER_CLASS, Some(true), 3);
    assert_eq!(
        offers_for_with("c1", CLASS, 62.0, 0, &g, &elsewhere, &p),
        offers_for("c1", 62.0, &g, &p),
        "a standing from another series must read exactly like no standing at all"
    );
}

// ── A career starts with something in the bank ───────────────────────────────

#[test]
fn test_a_starting_balance_is_spendable_before_anything_is_earned() {
    // The point of it: a driver nobody rates yet has no salary and no prize money, so without a
    // founding balance a pay-driver seat — the one route in that does not need a rating — could
    // never be taken by the only people who need it.
    let f = finances(&[], &[], &[], None, 1_000_000, &prize_params());
    assert_eq!(f.starting, 1_000_000);
    assert_eq!(f.balance, 1_000_000);
    assert_eq!((f.earned, f.spent), (0, 0));
    assert!(f.seasons.is_empty());
}

#[test]
fn test_the_starting_balance_is_not_earnings() {
    // It is capital, not income: a career that has raced nothing has earned nothing, however
    // much it began with. Keeping the two apart is what lets the ledger say where money came
    // from rather than only how much there is.
    let (c, ch, s) = won_season();
    let f = finances(&c, &ch, &s, None, 500_000, &prize_params());
    assert_eq!(f.starting, 500_000);
    assert_eq!(f.earned, c[0].salary + prize_params().champion_prize);
    assert_eq!(f.balance, f.starting + f.earned - f.spent);
}

#[test]
fn test_spending_comes_out_of_what_the_career_started_with() {
    let (mut c, ch, s) = won_season();
    c[0].bought_for = 300_000;
    let f = finances(&c, &ch, &s, None, 1_000_000, &prize_params());
    assert_eq!(f.spent, 300_000);
    assert_eq!(f.balance, 1_000_000 + f.earned - 300_000);
}

#[test]
fn test_a_career_that_started_with_nothing_is_unchanged() {
    // Every save written before this defaults to zero, which is exactly what it had.
    let (c, ch, s) = won_season();
    let f = finances(&c, &ch, &s, None, 0, &prize_params());
    assert_eq!(f.starting, 0);
    assert_eq!(f.balance, f.earned - f.spent);
}

#[test]
fn test_a_new_career_can_buy_into_two_pay_seats_on_every_shipped_grid() {
    // The promise `config::default_starting_balance` is set to keep: a driver who has raced
    // nothing has a *choice* of way in, not one take-it-or-leave-it seat. Re-measured here
    // rather than asserted as a number, so retuning the economy fails loudly instead of quietly
    // breaking the promise.
    use crate::driver_rating::{expected_positions, team_eligibility, RatingParams};

    let balance = crate::config::Config::default().starting_balance;
    let rating = RatingParams::default().starting_rating;
    let dir = std::path::Path::new(AI_DIR);

    // Two grids cannot offer a choice at any price: their back rows are reachable on merit, so
    // there is nothing there to buy. Named rather than skipped silently — if a roster edit gives
    // them pay seats, this list is what has to be revisited.
    let no_choice = ["F-Vintage_Gen2", "F-Classic_Gen3"];
    let mut checked = 0;

    for perf in crate::custom_ai::class_performance(dir) {
        let path = dir.join(format!("{}.xml", perf.class));
        let pace: std::collections::HashMap<String, f32> = perf
            .cars
            .iter()
            .map(|c| (c.team.clone(), c.pace_delta_pct))
            .collect();
        let seats = crate::custom_ai::parse_seats(&path);
        let elig = team_eligibility(
            rating,
            &expected_positions(&pace, &seats),
            &crate::custom_ai::parse_team_skills(&path),
        );

        // Offers generated *at* the balance, so the last-resort discount only fires if the
        // career genuinely cannot reach anything — reading prices at a balance of zero would
        // show a discounted seat rather than what it really costs.
        let offers = offers_for_with(
            "c",
            "",
            rating,
            balance,
            &elig,
            &Standing::default(),
            &params(),
        );
        let mut costs: Vec<i64> = offers
            .iter()
            .filter(|o| o.kind == OfferKind::Pay)
            .map(|o| o.buy_in)
            .collect();
        costs.sort();

        if no_choice.contains(&perf.class.as_str()) {
            assert!(
                costs.len() < 2,
                "{} now offers {} pay seats — it is no longer an exception",
                perf.class,
                costs.len()
            );
            continue;
        }

        assert!(costs.len() >= 2, "{} should offer a choice", perf.class);
        assert!(
            costs[1] <= balance,
            "{}: the second-cheapest seat costs {} but a new career has {}",
            perf.class,
            costs[1],
            balance
        );
        checked += 1;
    }
    assert_eq!(checked, 6, "every shipped grid but the two exceptions");
}

#[test]
fn test_the_starting_balance_buys_a_choice_not_the_grid() {
    // The other half of the promise. A balance that reached every seat for sale would make the
    // rating irrelevant in the first season, so on at least one shipped roster it must fall
    // short of the dearest seats.
    use crate::driver_rating::{expected_positions, team_eligibility, RatingParams};

    let balance = crate::config::Config::default().starting_balance;
    let rating = RatingParams::default().starting_rating;
    let dir = std::path::Path::new(AI_DIR);
    let mut grids_with_seats_out_of_reach = 0;

    for perf in crate::custom_ai::class_performance(dir) {
        let path = dir.join(format!("{}.xml", perf.class));
        let pace: std::collections::HashMap<String, f32> = perf
            .cars
            .iter()
            .map(|c| (c.team.clone(), c.pace_delta_pct))
            .collect();
        let elig = team_eligibility(
            rating,
            &expected_positions(&pace, &crate::custom_ai::parse_seats(&path)),
            &crate::custom_ai::parse_team_skills(&path),
        );
        let offers = offers_for_with(
            "c",
            "",
            rating,
            balance,
            &elig,
            &Standing::default(),
            &params(),
        );
        if offers
            .iter()
            .any(|o| o.kind == OfferKind::Pay && o.buy_in > balance)
        {
            grids_with_seats_out_of_reach += 1;
        }
    }
    assert!(
        grids_with_seats_out_of_reach >= 4,
        "a founding balance must leave plenty still to be earned, not open the market"
    );
}

// ── Never locked out, but never given a seat either ─────────────────────────

/// A grid an unproven driver has earned nothing on — which, on most shipped rosters, is exactly
/// what a first season looks like now that eligibility has no floor of its own.
fn nothing_earned() -> Vec<TeamEligibility> {
    vec![
        elig("Williams", Tier::Locked, 95.0, 1.5),
        elig("Brabham", Tier::Locked, 85.0, 3.5),
        elig("Osella", Tier::Locked, 70.0, 5.5),
        elig("AGS", Tier::Locked, 65.0, 7.5),
    ]
}

fn at_balance(rating: f32, balance: i64, g: &[TeamEligibility]) -> Vec<Offer> {
    offers_for_with(
        "c1",
        CLASS,
        rating,
        balance,
        g,
        &Standing::default(),
        &params(),
    )
}

#[test]
fn test_a_penniless_career_is_never_locked_out() {
    // Every seat asks more than the driver has earned, and the sponsorship on all of them is
    // beyond a balance of nothing. Something still has to be takeable, or the career cannot
    // begin — and a career that cannot begin can never earn its way out.
    let offers = at_balance(20.0, 0, &nothing_earned());
    let takeable: Vec<&Offer> = offers
        .iter()
        .filter(|o| o.kind == OfferKind::Paid || o.buy_in <= 0)
        .collect();
    assert_eq!(takeable.len(), 1, "exactly one way in, not a free-for-all");
    assert_eq!(
        takeable[0].kind,
        OfferKind::Pay,
        "it is still a bought seat"
    );
}

#[test]
fn test_the_last_resort_asks_for_everything_the_career_has() {
    // The point of pricing it at the balance rather than at zero: the weakest team still costs
    // something. A driver who scrapes in arrives with nothing left.
    for balance in [0, 250_000, 900_000] {
        let offers = at_balance(20.0, balance, &nothing_earned());
        let cheapest = offers
            .iter()
            .filter(|o| o.kind == OfferKind::Pay)
            .min_by_key(|o| o.buy_in)
            .expect("a seat for sale");
        assert_eq!(cheapest.buy_in, balance, "priced at exactly what there is");
    }
}

#[test]
fn test_the_discount_goes_to_the_cheapest_seat() {
    // Not to the best car the driver fancies: the way in is the one nobody else wanted.
    let g = nothing_earned();
    let offers = at_balance(20.0, 0, &g);
    let discounted: Vec<&str> = offers
        .iter()
        .filter(|o| o.kind == OfferKind::Pay && o.buy_in == 0)
        .map(|o| o.team.as_str())
        .collect();
    // AGS asks least of the four, so its seat is the cheapest to buy.
    assert_eq!(discounted, vec!["AGS"]);
}

#[test]
fn test_no_discount_while_anything_is_already_within_reach() {
    let g = nothing_earned();
    // Rich enough for the cheapest seat at its real price: nothing is marked down.
    let rich = at_balance(20.0, 50_000_000, &g);
    let poor = at_balance(20.0, 0, &g);
    let real_cheapest = rich
        .iter()
        .filter(|o| o.kind == OfferKind::Pay)
        .map(|o| o.buy_in)
        .min()
        .unwrap();
    assert!(real_cheapest > 0, "the honest price survives a full wallet");
    assert_ne!(
        real_cheapest,
        poor.iter()
            .filter(|o| o.kind == OfferKind::Pay)
            .map(|o| o.buy_in)
            .min()
            .unwrap()
    );
}

#[test]
fn test_an_earned_seat_stops_the_last_resort_firing() {
    // A driver who has earned something needs no way in inventing for them.
    let mut g = nothing_earned();
    g.push(elig("Coloni", Tier::Available, 10.0, 9.5));
    let offers = at_balance(20.0, 0, &g);
    assert!(
        offers
            .iter()
            .all(|o| o.kind != OfferKind::Pay || o.buy_in > 0),
        "nothing should be marked down: {offers:?}"
    );
}

#[test]
fn test_a_way_in_exists_even_with_pay_driver_seats_switched_off() {
    // `buy_in_per_point: 0` means no team sells, so there is nothing to discount. The least
    // demanding team takes the driver anyway — the alternative is a career that cannot start.
    let p = OfferParams {
        buy_in_per_point: 0,
        ..params()
    };
    let g = nothing_earned();
    let offers = offers_for_with("c1", CLASS, 20.0, 0, &g, &Standing::default(), &p);
    assert_eq!(offers.len(), 1, "{offers:?}");
    assert_eq!(offers[0].team, "AGS", "the least demanding team");
    assert_eq!(offers[0].kind, OfferKind::Paid);
    assert_eq!(offers[0].buy_in, 0);
}

#[test]
fn test_an_empty_grid_still_yields_nothing() {
    // No teams, no seat to invent. The guarantee is about a career reaching a grid, not about
    // conjuring one.
    assert!(at_balance(20.0, 1_000_000, &[]).is_empty());
}
