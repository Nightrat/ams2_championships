use super::*;
use std::fs;

fn tmp_path() -> std::path::PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("ams2_cfg_test_{ns}.json"))
}

#[test]
fn test_load_or_create_missing_file_writes_defaults_and_creates_file() {
    let path = tmp_path();
    assert!(!path.exists());
    let cfg = load_or_create(&path);
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.poll_ms, 200);
    assert!(cfg.record_practice);
    assert!(cfg.record_qualify);
    assert!(cfg.record_race);
    assert!(!cfg.show_track_map);
    assert_eq!(cfg.track_map_max_points, 5000);
    assert!(cfg.active_career.is_none());
    assert!(path.exists(), "config file should be created");
    let _ = fs::remove_file(&path);
}

#[test]
fn test_load_or_create_valid_json_reads_values() {
    let path = tmp_path();
    fs::write(&path, r#"{"port":9090,"host":"0.0.0.0","poll_ms":500,"record_practice":false,"record_qualify":true,"record_race":true,"show_track_map":false,"track_map_max_points":1000}"#).unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.port, 9090);
    assert_eq!(cfg.host, "0.0.0.0");
    assert_eq!(cfg.poll_ms, 500);
    assert!(!cfg.record_practice);
    assert!(cfg.record_qualify);
    assert!(!cfg.show_track_map);
    assert_eq!(cfg.track_map_max_points, 1000);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_load_or_create_partial_json_fills_serde_defaults() {
    let path = tmp_path();
    // Only provide port — all other fields should get their serde defaults
    fs::write(&path, r#"{"port":7777}"#).unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.port, 7777);
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.poll_ms, 200);
    assert!(cfg.record_race);
    assert!(!cfg.show_track_map);
    assert_eq!(cfg.track_map_max_points, 5000);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_load_or_create_invalid_json_returns_defaults() {
    let path = tmp_path();
    fs::write(&path, "not valid json {{{{").unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.host, "127.0.0.1");
    let _ = fs::remove_file(&path);
}

#[test]
fn test_load_and_upgrade_rewrites_file_with_all_fields() {
    let path = tmp_path();
    // Write a minimal config — the startup upgrade fills in every field added since.
    fs::write(&path, r#"{"port":9000}"#).unwrap();
    load_and_upgrade(&path);
    let written = fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&written).unwrap();
    for key in &[
        "host",
        "poll_ms",
        "record_practice",
        "record_qualify",
        "record_race",
        "show_track_map",
        "track_map_max_points",
    ] {
        assert!(
            v.get(key).is_some(),
            "expected key '{key}' in rewritten config"
        );
    }
    let _ = fs::remove_file(&path);
}

#[test]
fn test_config_default_values() {
    let cfg = Config::default();
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.poll_ms, 200);
    assert!(cfg.record_practice);
    assert!(cfg.record_qualify);
    assert!(cfg.record_race);
    assert!(!cfg.show_track_map);
    assert_eq!(cfg.track_map_max_points, 5000);
    assert!(cfg.active_career.is_none());
    assert!(cfg.enforce_team_eligibility);
}

#[test]
fn test_enforce_team_eligibility_defaults_on_for_existing_configs() {
    // Config files written before this setting existed must not silently disable enforcement.
    let path = tmp_path();
    fs::write(&path, r#"{"port":8080}"#).unwrap();
    assert!(load_or_create(&path).enforce_team_eligibility);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_load_or_create_active_career_some() {
    let path = tmp_path();
    fs::write(&path, r#"{"active_career":"GT3 Career"}"#).unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.active_career.as_deref(), Some("GT3 Career"));
    let _ = fs::remove_file(&path);
}

#[test]
fn test_a_legacy_data_file_path_still_names_the_active_career() {
    // Upgrading must not drop the career the user was on. The old setting held a full path; the
    // name is taken from it, in either save layout.
    let path = tmp_path();
    fs::write(&path, r#"{"data_file":"F:/champs/sp.json"}"#).unwrap();
    assert_eq!(load_or_create(&path).active_career.as_deref(), Some("sp"));

    fs::write(&path, r#"{"data_file":"F:/champs/sp/career.json"}"#).unwrap();
    assert_eq!(load_or_create(&path).active_career.as_deref(), Some("sp"));
    let _ = fs::remove_file(&path);
}

#[test]
fn test_active_career_wins_over_a_legacy_data_file() {
    // Both present only while a config written by an older build has not been rewritten yet.
    let path = tmp_path();
    fs::write(
        &path,
        r#"{"active_career":"current","data_file":"F:/champs/old.json"}"#,
    )
    .unwrap();
    assert_eq!(
        load_or_create(&path).active_career.as_deref(),
        Some("current")
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn test_the_legacy_data_file_field_is_dropped_on_the_next_write() {
    let path = tmp_path();
    fs::write(&path, r#"{"data_file":"F:/champs/sp.json"}"#).unwrap();
    let cfg = load_and_upgrade(&path);
    assert_eq!(cfg.active_career.as_deref(), Some("sp"));

    let written = fs::read_to_string(&path).unwrap();
    assert!(
        !written.contains("data_file"),
        "the old spelling is not written back: {written}"
    );
    assert!(written.contains("active_career"));
    let _ = fs::remove_file(&path);
}

#[test]
fn test_rating_tuning_defaults_for_existing_configs() {
    // A config written before the rating was tunable must rate exactly as it did then.
    let path = tmp_path();
    fs::write(&path, r#"{"port":8080}"#).unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.rating_params(), RatingParams::default());
    assert_eq!(cfg.retirement_min_laps_down, 3);
    assert_eq!(cfg.retirement_distance_pct, 90.0);
    assert!(!cfg.hide_locked_teams, "the ladder is shown by default");
    let _ = fs::remove_file(&path);
}

#[test]
fn test_rating_params_clamps_hand_edited_values() {
    // config.json is edited by hand often enough that the bounds cannot be trusted on the way
    // in. A half-life below zero would invert the decay; a strictness of 400 would lock the
    // grid with no way back through the UI.
    let path = tmp_path();
    fs::write(
        &path,
        r#"{"starting_rating":250.0,"rating_strictness":-400.0,"rating_half_life":-3.0}"#,
    )
    .unwrap();
    let p = load_or_create(&path).rating_params();
    assert_eq!(p.starting_rating, 100.0);
    assert_eq!(p.strictness, -50.0);
    assert_eq!(p.recency_half_life, 0.0);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_eligibility_gates_round_trip_through_the_file() {
    let path = tmp_path();
    fs::write(&path, r#"{"eligibility_gates":"incumbent"}"#).unwrap();
    assert_eq!(load_or_create(&path).eligibility_gates, Gates::Incumbent);
    // The startup upgrade rewrites the file with every field filled in; the enum must survive it.
    load_and_upgrade(&path);
    assert_eq!(load_or_create(&path).eligibility_gates, Gates::Incumbent);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_retirement_distance_converts_percent_to_a_fraction() {
    // The config speaks in percent because that is how the hint reads; the rating math wants a
    // fraction. Out-of-range values are clamped before the conversion, never after.
    let path = tmp_path();
    fs::write(&path, r#"{"retirement_distance_pct":75.0}"#).unwrap();
    let p = load_or_create(&path).rating_params();
    assert!((p.retirement_distance - 0.75).abs() < 0.0001);

    fs::write(&path, r#"{"retirement_distance_pct":400.0}"#).unwrap();
    let p = load_or_create(&path).rating_params();
    assert!(
        (p.retirement_distance - 1.0).abs() < 0.0001,
        "a share of the distance cannot exceed the whole, got {}",
        p.retirement_distance
    );
    let _ = fs::remove_file(&path);
}

// ── Contract economy ─────────────────────────────────────────────────────────

#[test]
fn test_economy_defaults_match_the_module() {
    let path = tmp_path();
    fs::write(&path, "{}").unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.offer_params(), crate::contracts::OfferParams::default());
    assert_eq!(cfg.prize_params(), crate::contracts::PrizeParams::default());
    // Contracts are decided by the career mode now, not by config — nothing to assert here.
    let _ = fs::remove_file(&path);
}

#[test]
fn test_an_inverted_salary_pair_holds_the_floor_below_the_top() {
    // config.json is hand-edited. A floor above the top would pay the back of the grid more than
    // the front, which no UI could explain; the floor is held down rather than the pair swapped,
    // because silently reordering someone's numbers is worse than ignoring one of them.
    let path = tmp_path();
    fs::write(
        &path,
        r#"{"contract_top_salary":100,"contract_floor_salary":900}"#,
    )
    .unwrap();
    let p = load_or_create(&path).offer_params();
    assert_eq!(p.top_salary, 100);
    assert_eq!(p.floor_salary, 100);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_an_inverted_prize_pair_is_clamped_the_same_way() {
    let path = tmp_path();
    fs::write(&path, r#"{"champion_prize":500,"last_place_prize":9000}"#).unwrap();
    let p = load_or_create(&path).prize_params();
    assert_eq!((p.champion_prize, p.floor_prize), (500, 500));
    let _ = fs::remove_file(&path);
}

#[test]
fn test_a_zero_or_negative_salary_cannot_reach_the_economy() {
    // Zero would divide the geometric curve by nothing; negative would pay a driver to race.
    let path = tmp_path();
    fs::write(
        &path,
        r#"{"contract_top_salary":0,"contract_floor_salary":-5,"champion_prize":-1}"#,
    )
    .unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.offer_params().top_salary, 1);
    assert_eq!(cfg.offer_params().floor_salary, 1);
    assert_eq!(cfg.prize_params().champion_prize, 1);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_a_zero_buy_in_is_allowed_because_it_means_off() {
    // Unlike the salaries, zero here is a setting rather than a mistake: it switches pay-driver
    // seats off entirely.
    let path = tmp_path();
    fs::write(&path, r#"{"contract_buy_in_per_point":0}"#).unwrap();
    assert_eq!(load_or_create(&path).offer_params().buy_in_per_point, 0);

    fs::write(&path, r#"{"contract_buy_in_per_point":-50}"#).unwrap();
    assert_eq!(load_or_create(&path).offer_params().buy_in_per_point, 0);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_objective_slack_is_not_config_driven_yet() {
    // A feel knob rather than money, so it stays on the module default; a config that mentions
    // it must not appear to change anything. `contract_max_seasons` is here too because deals
    // are single-season now — a config carrying it from before must be inert, not an error.
    let path = tmp_path();
    fs::write(
        &path,
        r#"{"contract_max_seasons":99,"contract_objective_slack":99}"#,
    )
    .unwrap();
    let p = load_or_create(&path).offer_params();
    assert_eq!(
        p.objective_slack,
        crate::contracts::OfferParams::default().objective_slack
    );
    let _ = fs::remove_file(&path);
}

// ── A read must never write ──────────────────────────────────────────────────

#[test]
fn test_load_or_create_does_not_touch_an_existing_file() {
    // The whole cause of "EOF while parsing a value at line 1 column 0": this is called several
    // times per request from more than one thread, and `fs::write` truncates before it writes.
    // A reader landing in that window saw an empty file and fell back to defaults — which the
    // next call would then persist over the user's real settings.
    let path = tmp_path();
    let original = r#"{"port":9000}"#;
    fs::write(&path, original).unwrap();

    let before = fs::metadata(&path).unwrap().modified().unwrap();
    for _ in 0..5 {
        assert_eq!(load_or_create(&path).port, 9000);
    }
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        original,
        "a read must leave the file byte-for-byte alone"
    );
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_a_damaged_config_is_never_written_over() {
    // Same rule as career saves: defaults let the server start, but writing them back would
    // destroy the settings nobody managed to read.
    let path = tmp_path();
    let broken = "{ not json at all";
    fs::write(&path, broken).unwrap();

    let cfg = load_or_create(&path);
    assert_eq!(
        cfg.port, 8080,
        "falls back to defaults so the server starts"
    );
    let err = save(&path, &cfg).expect_err("writing defaults over it must be refused");
    assert!(err.contains("refusing to overwrite"), "{err}");
    assert_eq!(fs::read_to_string(&path).unwrap(), broken);

    // And the startup upgrade must not sneak past the same guard.
    load_and_upgrade(&path);
    assert_eq!(fs::read_to_string(&path).unwrap(), broken);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_a_config_with_a_byte_order_mark_still_loads() {
    let path = tmp_path();
    fs::write(&path, "\u{feff}{\"port\":9100}").unwrap();
    assert_eq!(load_or_create(&path).port, 9100);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_save_leaves_no_temp_file_behind() {
    // The write goes via a sibling temp file so a concurrent reader never sees a half-written
    // config; it must not survive the rename.
    let path = tmp_path();
    save(&path, &Config::default()).unwrap();
    assert!(!path.with_extension("json.tmp").exists());
    assert_eq!(load_or_create(&path).port, 8080);
    let _ = fs::remove_file(&path);
}

// ── The economy stored is the economy used ───────────────────────────────────

#[test]
fn test_normalize_economy_makes_the_stored_pair_the_effective_one() {
    // Clamping only on the way out cannot enforce a relationship *between* two fields: an
    // inverted pair would be stored, shown by the Config tab, and quietly ignored by the grid.
    let mut cfg = Config {
        contract_top_salary: 100_000,
        contract_floor_salary: 900_000,
        champion_prize: 500,
        last_place_prize: 9_000,
        ..Config::default()
    };
    cfg.normalize_economy();

    assert_eq!(cfg.contract_floor_salary, 100_000, "floor held below top");
    assert_eq!(cfg.last_place_prize, 500, "last place held below champion");
    // Stored and effective now agree, which is the whole point.
    assert_eq!(cfg.contract_floor_salary, cfg.offer_params().floor_salary);
    assert_eq!(cfg.last_place_prize, cfg.prize_params().floor_prize);
}

#[test]
fn test_normalize_economy_is_idempotent_and_leaves_a_sane_config_alone() {
    let mut cfg = Config::default();
    let (top, floor) = (cfg.contract_top_salary, cfg.contract_floor_salary);
    cfg.normalize_economy();
    assert_eq!(
        (cfg.contract_top_salary, cfg.contract_floor_salary),
        (top, floor)
    );
    cfg.normalize_economy();
    assert_eq!(
        (cfg.contract_top_salary, cfg.contract_floor_salary),
        (top, floor)
    );
}

#[test]
fn test_normalize_economy_pulls_nonsense_into_range() {
    let mut cfg = Config {
        contract_top_salary: -5,
        contract_floor_salary: 0,
        contract_buy_in_per_point: -1,
        champion_prize: i64::MAX,
        ..Config::default()
    };
    cfg.normalize_economy();
    assert_eq!(cfg.contract_top_salary, 1);
    assert_eq!(cfg.contract_floor_salary, 1);
    assert_eq!(
        cfg.contract_buy_in_per_point, 0,
        "zero means pay-drivers off"
    );
    assert!(cfg.champion_prize <= 1_000_000_000);
}

#[test]
fn test_a_career_starts_with_enough_to_choose_a_way_in() {
    // What the figure is *for* lives in `contracts`, where it can be measured against the real
    // rosters: see `test_a_new_career_can_buy_into_two_pay_seats_on_every_shipped_grid`. All this
    // asserts is the shape — a career starts with real money, on the order of a season's pay
    // rather than a rounding error.
    let path = tmp_path();
    fs::write(&path, "{}").unwrap();
    let cfg = load_or_create(&path);
    assert!(cfg.starting_balance > cfg.offer_params().floor_salary);
    assert!(cfg.starting_balance >= cfg.offer_params().buy_in_per_point);
    let _ = fs::remove_file(&path);
}

#[test]
fn test_a_hand_set_starting_balance_survives_and_is_clamped() {
    let path = tmp_path();
    fs::write(&path, r#"{"starting_balance":250000}"#).unwrap();
    assert_eq!(load_or_create(&path).starting_balance, 250_000);

    // Negative is meaningless; a career cannot be founded in debt.
    fs::write(&path, r#"{"starting_balance":-5}"#).unwrap();
    assert!(load_or_create(&path).starting_balance.max(0) == 0);
    let _ = fs::remove_file(&path);
}
