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
    assert!(cfg.data_file.is_none());
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
fn test_load_or_create_rewrites_file_with_all_fields() {
    let path = tmp_path();
    // Write a minimal config — on load it should be rewritten with all fields present
    fs::write(&path, r#"{"port":9000}"#).unwrap();
    load_or_create(&path);
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
    assert!(cfg.data_file.is_none());
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
fn test_load_or_create_data_file_some() {
    let path = tmp_path();
    fs::write(&path, r#"{"data_file":"/some/path/career.json"}"#).unwrap();
    let cfg = load_or_create(&path);
    assert_eq!(cfg.data_file, Some("/some/path/career.json".into()));
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
    // load_or_create rewrites the file with every field filled in; the enum must survive that.
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
