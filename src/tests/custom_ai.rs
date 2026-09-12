use super::*;

const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!--Custom AI by jusk - F1 1967 Season
Some comment with a < in it just in case
-->
<custom_ai_drivers>
	<driver livery_name="Brabham-Repco #1 J. Brabham">
		<name>Jack Brabham</name>
		<country>AUS</country>
        <race_skill>0.93</race_skill>
	</driver>
	<driver livery_name="Brabham-Repco #1 J. Brabham" tracks="Kyalami_Historic">
        <qualifying_skill>0.98</qualifying_skill>
	</driver>
	<driver livery_name="Brabham-Repco #2 D. Hulme">
		<name>Denny Hulme</name>
		<country>NZL</country>
	</driver>
</custom_ai_drivers>
"#;

#[test]
fn test_parse_driver_teams_basic_mapping() {
    let map = parse_driver_teams_str(SAMPLE);
    // Only the team name is kept — car number and driver name are stripped.
    assert_eq!(map.get("Jack Brabham"), Some(&"Brabham-Repco".to_string()));
    assert_eq!(map.get("Denny Hulme"), Some(&"Brabham-Repco".to_string()));
}

#[test]
fn test_extract_team_name_strips_number_and_driver() {
    assert_eq!(
        extract_team_name("Brabham-Repco #1 J. Brabham"),
        "Brabham-Repco"
    );
}

#[test]
fn test_extract_team_name_strips_leading_year() {
    assert_eq!(extract_team_name("1986 AGS #31 - I. Capelli"), "AGS");
    assert_eq!(
        extract_team_name("1988 Eurobrun #32 - O. Larrauri"),
        "Eurobrun"
    );
}

#[test]
fn test_extract_team_name_no_hash_returns_trimmed_input() {
    assert_eq!(extract_team_name("  Some Team Only  "), "Some Team Only");
}

#[test]
fn test_extract_team_name_driver_before_number_with_dash_separator() {
    // F-Retro_Gen1.xml's convention: "Team - Driver #Num", unlike the more common
    // "Team #Num Driver" / "Team #Num - Driver" used elsewhere.
    assert_eq!(
        extract_team_name("Marlboro Team Texaco - E. Fittipaldi #5"),
        "Marlboro Team Texaco"
    );
    // Team name itself contains an unspaced hyphen — must not be mistaken for the separator.
    assert_eq!(
        extract_team_name("Dalton-Amon Int. - C. Amon #22"),
        "Dalton-Amon Int."
    );
}

#[test]
fn test_parse_driver_teams_ignores_track_override_blocks() {
    let map = parse_driver_teams_str(SAMPLE);
    // Only two distinct drivers have <name> — the track-specific override block must not add entries.
    assert_eq!(map.len(), 2);
}

#[test]
fn test_parse_driver_teams_empty_xml_returns_empty_map() {
    let map = parse_driver_teams_str("<custom_ai_drivers></custom_ai_drivers>");
    assert!(map.is_empty());
}

#[test]
fn test_parse_driver_teams_malformed_xml_does_not_panic() {
    let map = parse_driver_teams_str("<driver livery_name=\"Oops\"><name>Unclosed");
    // Neither </name> nor </driver> is closed: the name can't be read, so the block is skipped
    // rather than panicking on the truncated input.
    assert!(map.is_empty());
}

#[test]
fn test_parse_driver_teams_file_not_found_returns_empty() {
    let map = parse_driver_teams(std::path::Path::new("Z:/does/not/exist_nope.xml"));
    assert!(map.is_empty());
}

// ── Grid seats and player-team inference ─────────────────────────────────────

/// Mirrors the shape of the real 1986 F-Classic_Gen1 file: Brabham #8 carries two alternate
/// drivers, and Danner appears under two different teams (Osella and Arrows).
const ROSTER: &str = r#"<custom_ai_drivers>
    <driver livery_name="1986 Williams #5 - N. Mansell"><name>Nigel Mansell</name></driver>
    <driver livery_name="1986 Williams #6 - N. Piquet"><name>Nelson Piquet</name></driver>
    <driver livery_name="1986 Brabham #7 - R. Patrese"><name>Riccardo Patrese</name></driver>
    <driver livery_name="1986 Brabham #8 - D. Warwick"><name>Derek Warwick</name></driver>
    <driver livery_name="1986 Brabham #8 - E. De Angelis"><name>Elio De Angelis</name></driver>
    <driver livery_name="1986 McLaren #1 - A. Prost"><name>Alain Prost</name></driver>
    <driver livery_name="1986 Arrows #18 - T. Boutsen"><name>Thierry Boutsen</name></driver>
    <driver livery_name="1986 Arrows #17 - C. Danner"><name>Christian Danner</name></driver>
    <driver livery_name="1986 Osella #22 - C. Danner"><name>Christian Danner</name></driver>
    <driver livery_name="1986 Osella #21 - A. Berg"><name>Allan Berg</name></driver>
</custom_ai_drivers>"#;

const M1: &str = "Formula Classic Gen1 Model1";
const M2: &str = "Formula Classic Gen1 Model2";

fn grid<'a>(rows: &'a [(&'a str, &'a str, bool)]) -> Vec<GridEntry<'a>> {
    rows.iter()
        .map(|(name, car_name, is_player)| GridEntry {
            name,
            car_name,
            is_player: *is_player,
        })
        .collect()
}

/// A grid leaving only Brabham #7 free among the Model1 seats.
const FULL_GRID: &[(&str, &str, bool)] = &[
    ("Nightrat", M1, true),
    ("Nigel Mansell", M1, false),
    ("Nelson Piquet", M1, false),
    ("Derek Warwick", M1, false),
    ("Alain Prost", M2, false),
    ("Thierry Boutsen", M2, false),
    ("Christian Danner", M1, false),
    ("Allen Berg", M1, false),
];

#[test]
fn test_parse_seats_extracts_team_and_number() {
    let seats = parse_seats_str(ROSTER);
    assert_eq!(seats.len(), 10, "one entry per named driver");
    let patrese = seats
        .iter()
        .find(|s| s.driver == "Riccardo Patrese")
        .unwrap();
    assert_eq!(patrese.seat, "Brabham #7");
    assert_eq!(patrese.team, "Brabham");
}

#[test]
fn test_parse_seats_dedupes_to_fewer_seats_than_entries() {
    let seats = parse_seats_str(ROSTER);
    let distinct: std::collections::HashSet<&str> = seats.iter().map(|s| s.seat.as_str()).collect();
    // Brabham #8 has two drivers, so 10 entries collapse to 9 seats.
    assert_eq!(distinct.len(), 9);
}

#[test]
fn test_name_key_matches_across_spelling_variants() {
    // The real roster says "Allan Berg"; AMS2's telemetry reports "Allen Berg".
    assert_eq!(name_key("Allan Berg"), name_key("Allen Berg"));
    // Multi-word surnames key off the last word, matching the livery's "A. De Cesaris".
    assert_eq!(name_key("Andre De Cesaris"), "a|cesaris");
}

#[test]
fn test_name_key_ignores_stock_ai_marker() {
    assert_eq!(name_key("Aires Silva  (AI)"), name_key("Aires Silva"));
}

#[test]
fn test_infer_derives_single_empty_seat() {
    let seats = parse_seats_str(ROSTER);
    let g = grid(FULL_GRID);
    match infer_player_seat(&seats, &g) {
        PlayerSeat::Derived(seat) => assert_eq!(seat.seat, "Brabham #7"),
        other => panic!("expected Derived, got {other:?}"),
    }
}

#[test]
fn test_infer_resolves_dual_livery_driver_by_car_model() {
    let seats = parse_seats_str(ROSTER);
    // Danner on Model1 must be read as Osella #22, leaving Arrows #17 free rather than Osella.
    // If he were mis-seated at Arrows #17, Osella #22 would show as a free Model1 seat and the
    // result would be Candidates instead of Derived.
    assert!(matches!(
        infer_player_seat(&seats, &grid(FULL_GRID)),
        PlayerSeat::Derived(_)
    ));

    // On Model2 the same driver is the Arrows entry instead, so Osella #22 becomes free.
    let rows: Vec<(&str, &str, bool)> = FULL_GRID
        .iter()
        .map(|&(n, c, p)| {
            if n == "Christian Danner" {
                (n, M2, p)
            } else {
                (n, c, p)
            }
        })
        .collect();
    match infer_player_seat(&seats, &grid(&rows)) {
        PlayerSeat::Candidates(v) => {
            let names: Vec<&str> = v.iter().map(|s| s.seat.as_str()).collect();
            assert!(names.contains(&"Osella #22"), "got {names:?}");
        }
        other => panic!("expected Candidates, got {other:?}"),
    }
}

#[test]
fn test_infer_filters_candidates_by_player_car_model() {
    let seats = parse_seats_str(ROSTER);
    // Drop Warwick: both Brabham seats open up, but Arrows #17 stays excluded because Boutsen
    // pins Arrows to Model2 while the player drove Model1.
    let rows: Vec<(&str, &str, bool)> = FULL_GRID
        .iter()
        .copied()
        .filter(|&(n, _, _)| n != "Derek Warwick")
        .collect();
    match infer_player_seat(&seats, &grid(&rows)) {
        PlayerSeat::Candidates(v) => {
            let mut names: Vec<&str> = v.iter().map(|s| s.seat.as_str()).collect();
            names.sort();
            assert_eq!(names, vec!["Brabham #7", "Brabham #8"]);
        }
        other => panic!("expected Candidates, got {other:?}"),
    }
}

#[test]
fn test_infer_identifies_player_without_is_player_flag() {
    let seats = parse_seats_str(ROSTER);
    // Sessions recorded before the is_player flag existed have it false on every row; the
    // player is still identifiable as the only name absent from the roster, which keeps the
    // car-model filter working (without it, Arrows #17 would survive as a candidate).
    let rows: Vec<(&str, &str, bool)> = FULL_GRID.iter().map(|&(n, c, _)| (n, c, false)).collect();
    match infer_player_seat(&seats, &grid(&rows)) {
        PlayerSeat::Derived(seat) => assert_eq!(seat.seat, "Brabham #7"),
        other => panic!("expected Derived, got {other:?}"),
    }
}

#[test]
fn test_infer_reports_roster_not_detected_for_stock_ai() {
    let seats = parse_seats_str(ROSTER);
    let rows: &[(&str, &str, bool)] = &[
        ("Nightrat", M1, true),
        ("Aires Silva  (AI)", M1, false),
        ("Aldo Conti  (AI)", M1, false),
        ("Alex James  (AI)", M1, false),
    ];
    match infer_player_seat(&seats, &grid(rows)) {
        PlayerSeat::RosterNotDetected { matched, .. } => assert_eq!(matched, 0),
        other => panic!("expected RosterNotDetected, got {other:?}"),
    }
}

#[test]
fn test_infer_reports_no_empty_seat_when_grid_is_full() {
    let seats = parse_seats_str(
        r#"<custom_ai_drivers>
        <driver livery_name="1986 Williams #5 - N. Mansell"><name>Nigel Mansell</name></driver>
    </custom_ai_drivers>"#,
    );
    let rows: &[(&str, &str, bool)] = &[("Nightrat", M1, true), ("Nigel Mansell", M1, false)];
    assert_eq!(
        infer_player_seat(&seats, &grid(rows)),
        PlayerSeat::NoEmptySeat
    );
}

#[test]
fn test_check_player_team_accepts_declared_team_and_seat() {
    let seats = parse_seats_str(ROSTER);
    let g = grid(FULL_GRID);
    assert!(matches!(
        check_player_team(&seats, &g, "Brabham"),
        TeamCheck::Passed(_)
    ));
    // The car number may be included, and case is ignored.
    assert!(matches!(
        check_player_team(&seats, &g, "brabham #7"),
        TeamCheck::Passed(_)
    ));
}

#[test]
fn test_check_player_team_rejects_contradicted_team() {
    let seats = parse_seats_str(ROSTER);
    let g = grid(FULL_GRID);
    // Williams is refuted by both its drivers being on the grid, McLaren by the car model.
    match check_player_team(&seats, &g, "Williams") {
        TeamCheck::Failed(reason) => assert!(reason.contains("Brabham #7"), "got {reason}"),
        other => panic!("expected Failed, got {other:?}"),
    }
    assert!(matches!(
        check_player_team(&seats, &g, "McLaren"),
        TeamCheck::Failed(_)
    ));
}

#[test]
fn test_check_player_team_skips_when_unverifiable() {
    let seats = parse_seats_str(ROSTER);
    let g = grid(FULL_GRID);
    // No declared team, and no roster, are both "accept without checking".
    assert!(matches!(
        check_player_team(&seats, &g, "   "),
        TeamCheck::Skipped(_)
    ));
    assert!(matches!(
        check_player_team(&[], &g, "Brabham"),
        TeamCheck::Skipped(_)
    ));
    let stock: &[(&str, &str, bool)] = &[("Nightrat", M1, true), ("Aires Silva  (AI)", M1, false)];
    assert!(matches!(
        check_player_team(&seats, &grid(stock), "Brabham"),
        TeamCheck::Skipped(_)
    ));
}

#[test]
fn test_list_files_filters_xml_and_sorts() {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_custom_ai_test_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("b_drivers.xml"), "<x/>").unwrap();
    std::fs::write(dir.join("a_drivers.xml"), "<x/>").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignore me").unwrap();

    let files = list_files(&dir);
    assert_eq!(files, vec!["a_drivers.xml", "b_drivers.xml"]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_list_files_missing_dir_returns_empty() {
    let files = list_files(std::path::Path::new("Z:/definitely/missing/dir"));
    assert!(files.is_empty());
}

const SAMPLE_WITH_SCALARS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<custom_ai_drivers>
    <driver livery_name="Williams #5 N. Mansell">
        <name>Nigel Mansell</name>
        <power_scalar>1.10</power_scalar>
        <weight_scalar>0.97</weight_scalar>
        <drag_scalar>0.95</drag_scalar>
    </driver>
    <driver livery_name="Williams #6 N. Piquet">
        <name>Nelson Piquet</name>
        <power_scalar>1.10</power_scalar>
        <weight_scalar>0.97</weight_scalar>
        <drag_scalar>0.95</drag_scalar>
    </driver>
    <driver livery_name="Williams #5 N. Mansell" tracks="Monza_1991">
        <qualifying_skill>0.94</qualifying_skill>
    </driver>
    <driver livery_name="AGS #31 I. Capelli">
        <name>Ivan Capelli</name>
        <power_scalar>0.90</power_scalar>
        <weight_scalar>1.05</weight_scalar>
        <drag_scalar>1.10</drag_scalar>
    </driver>
</custom_ai_drivers>
"#;

#[test]
fn test_parse_car_performance_dedupes_by_team() {
    let cars = parse_car_performance_str(SAMPLE_WITH_SCALARS);
    // Two Williams drivers share one physical car — only one row.
    assert_eq!(cars.len(), 2);
    let williams = cars.iter().find(|c| c.team == "Williams").unwrap();
    assert_eq!(williams.power_scalar, 1.10);
    assert_eq!(williams.weight_scalar, 0.97);
    assert_eq!(williams.drag_scalar, 0.95);
}

#[test]
fn test_parse_car_performance_ignores_track_override_blocks() {
    let cars = parse_car_performance_str(SAMPLE_WITH_SCALARS);
    // The Monza override block for Mansell has no <name> and must not add a third row.
    assert_eq!(cars.len(), 2);
}

#[test]
fn test_parse_car_performance_sorted_alphabetically() {
    let cars = parse_car_performance_str(SAMPLE_WITH_SCALARS);
    assert_eq!(cars[0].team, "AGS");
    assert_eq!(cars[1].team, "Williams");
}

#[test]
fn test_parse_car_performance_missing_scalars_default_to_neutral() {
    let xml = r#"<custom_ai_drivers>
        <driver livery_name="Some Team #1 A. Driver">
            <name>A Driver</name>
            <qualifying_skill>0.9</qualifying_skill>
        </driver>
    </custom_ai_drivers>"#;
    let cars = parse_car_performance_str(xml);
    assert_eq!(cars.len(), 1);
    assert_eq!(cars[0].power_scalar, 1.0);
    assert_eq!(cars[0].weight_scalar, 1.0);
    assert_eq!(cars[0].drag_scalar, 1.0);
}

#[test]
fn test_parse_car_performance_empty_xml_returns_empty() {
    let cars = parse_car_performance_str("<custom_ai_drivers></custom_ai_drivers>");
    assert!(cars.is_empty());
}

#[test]
fn test_class_performance_ranks_fastest_first_with_zeroed_best() {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_custom_ai_perf_test_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("F-Test.xml"), SAMPLE_WITH_SCALARS).unwrap();

    let classes = class_performance(&dir);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].class, "F-Test");
    // Williams (power 1.10 / weight 0.97 / drag 0.95) is faster than AGS (0.90 / 1.05 / 1.10).
    assert_eq!(classes[0].cars[0].team, "Williams");
    assert_eq!(classes[0].cars[0].pace_delta_pct, 0.0);
    assert!(classes[0].cars[1].team == "AGS" && classes[0].cars[1].pace_delta_pct > 0.0);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_class_performance_missing_dir_returns_empty() {
    let classes = class_performance(std::path::Path::new("Z:/definitely/missing/dir"));
    assert!(classes.is_empty());
}

// Builds `<tmp>/UserData/CustomAIDrivers` (returned) alongside `<tmp>/GUI/HUD_1_6/HUD_ColoursDefs.xml`
// containing the given registered class names, mimicking the real AMS2 install layout.
fn make_install_with_registry(tag: &str, registered: &[&str]) -> std::path::PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let root = std::env::temp_dir().join(format!("ams2_custom_ai_install_{tag}_{ns}"));
    let ai_dir = root.join("UserData").join("CustomAIDrivers");
    let hud_dir = root.join("GUI").join("HUD_1_6");
    std::fs::create_dir_all(&ai_dir).unwrap();
    std::fs::create_dir_all(&hud_dir).unwrap();
    let colours = registered
        .iter()
        .map(|n| format!("<Colour\n\tname=\"{n}\" \n\tr=\"1\" g=\"2\" b=\"3\"\n/>\n"))
        .collect::<String>();
    std::fs::write(hud_dir.join("HUD_ColoursDefs.xml"), colours).unwrap();
    ai_dir
}

#[test]
fn test_known_class_names_parses_registered_names() {
    let ai_dir = make_install_with_registry("parse", &["F-Classic_Gen1", "F-Classic_Gen1_LD"]);
    let names = known_class_names(&ai_dir).expect("registry should be found");
    assert!(names.contains("F-Classic_Gen1"));
    assert!(names.contains("F-Classic_Gen1_LD"));
    assert!(!names.contains("F-Classic_Gen1_1986"));
    std::fs::remove_dir_all(ai_dir.parent().unwrap().parent().unwrap()).ok();
}

#[test]
fn test_known_class_names_missing_registry_returns_none() {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_custom_ai_no_registry_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(known_class_names(&dir).is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_class_performance_filters_out_unregistered_files() {
    let ai_dir = make_install_with_registry("filter", &["F-Classic_Gen1"]);
    std::fs::write(ai_dir.join("F-Classic_Gen1.xml"), SAMPLE_WITH_SCALARS).unwrap();
    // Same content, but this filename isn't a class AMS2 recognizes — must be excluded.
    std::fs::write(ai_dir.join("F-Classic_Gen1_1986.xml"), SAMPLE_WITH_SCALARS).unwrap();

    let classes = class_performance(&ai_dir);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].class, "F-Classic_Gen1");

    std::fs::remove_dir_all(ai_dir.parent().unwrap().parent().unwrap()).ok();
}

#[test]
fn test_class_performance_orders_classes_chronologically() {
    let ai_dir = make_install_with_registry(
        "chrono",
        &[
            "F-Classic_Gen1",
            "F-Vintage_Gen1",
            "F-Retro_Gen1",
            "F-Unmapped-Class",
        ],
    );
    // Written in a deliberately non-chronological, non-alphabetical order on disk.
    std::fs::write(ai_dir.join("F-Classic_Gen1.xml"), SAMPLE_WITH_SCALARS).unwrap(); // 1986
    std::fs::write(ai_dir.join("F-Vintage_Gen1.xml"), SAMPLE_WITH_SCALARS).unwrap(); // 1967
    std::fs::write(ai_dir.join("F-Retro_Gen1.xml"), SAMPLE_WITH_SCALARS).unwrap(); // 1974
    std::fs::write(ai_dir.join("F-Unmapped-Class.xml"), SAMPLE_WITH_SCALARS).unwrap(); // no year

    let classes = class_performance(&ai_dir);
    let names: Vec<&str> = classes.iter().map(|c| c.class.as_str()).collect();
    // Chronological: 1967, 1974, 1986, then the unmapped class last.
    assert_eq!(
        names,
        vec![
            "F-Vintage_Gen1",
            "F-Retro_Gen1",
            "F-Classic_Gen1",
            "F-Unmapped-Class"
        ]
    );
    assert_eq!(classes[0].year, Some(1967));
    assert_eq!(classes[3].year, None);

    std::fs::remove_dir_all(ai_dir.parent().unwrap().parent().unwrap()).ok();
}

#[test]
fn test_class_performance_includes_everything_when_registry_missing() {
    // No GUI/HUD_1_6 registry anywhere above this temp dir — filtering can't be verified,
    // so every file must still be included rather than none.
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_custom_ai_perf_no_registry_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Some_Unverifiable_Class.xml"), SAMPLE_WITH_SCALARS).unwrap();

    let classes = class_performance(&dir);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].class, "Some_Unverifiable_Class");

    std::fs::remove_dir_all(&dir).ok();
}

// ── Writing scalars back ──────────────────────────────────────────────────────

fn scalars(power: f32, weight: f32, drag: f32) -> Scalars {
    Scalars {
        power,
        weight,
        drag,
    }
}

#[test]
fn test_set_team_scalars_updates_every_driver_on_the_team() {
    let out =
        set_team_scalars_str(SAMPLE_WITH_SCALARS, "Williams", scalars(1.02, 1.01, 0.99)).unwrap();
    let cars = parse_car_performance_str(&out);
    let williams = cars.iter().find(|c| c.team == "Williams").unwrap();
    assert_eq!(williams.power_scalar, 1.02);
    assert_eq!(williams.weight_scalar, 1.01);
    assert_eq!(williams.drag_scalar, 0.99);
    // Both Williams entries, not just the first (parse dedupes, so count the text).
    assert_eq!(out.matches("<power_scalar>1.02</power_scalar>").count(), 2);
}

#[test]
fn test_set_team_scalars_leaves_other_teams_alone() {
    let out =
        set_team_scalars_str(SAMPLE_WITH_SCALARS, "Williams", scalars(1.02, 1.01, 0.99)).unwrap();
    let cars = parse_car_performance_str(&out);
    let ags = cars.iter().find(|c| c.team == "AGS").unwrap();
    assert_eq!(ags.power_scalar, 0.90);
    assert_eq!(ags.weight_scalar, 1.05);
    assert_eq!(ags.drag_scalar, 1.10);
}

#[test]
fn test_set_team_scalars_preserves_untouched_text() {
    let out = set_team_scalars_str(SAMPLE_WITH_SCALARS, "AGS", scalars(1.0, 1.0, 1.0)).unwrap();
    // Only the three AGS scalar values change; every other line survives verbatim.
    assert!(out.contains(r#"<driver livery_name="Williams #5 N. Mansell" tracks="Monza_1991">"#));
    assert!(out.contains("<qualifying_skill>0.94</qualifying_skill>"));
    assert!(out.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
    assert_eq!(out.lines().count(), SAMPLE_WITH_SCALARS.lines().count());
}

#[test]
fn test_set_team_scalars_does_not_touch_track_override_blocks() {
    let out =
        set_team_scalars_str(SAMPLE_WITH_SCALARS, "Williams", scalars(1.02, 1.01, 0.99)).unwrap();
    // The Monza block carries no <name>, so it stays a two-line block with no scalars added.
    let start = out.find(r#"tracks="Monza_1991""#).unwrap();
    let end = out[start..].find("</driver>").unwrap() + start;
    assert!(!out[start..end].contains("power_scalar"));
}

#[test]
fn test_set_team_scalars_inserts_missing_tags() {
    let xml = r#"<custom_ai_drivers>
    <driver livery_name="Some Team #1 A. Driver">
        <name>A Driver</name>
        <race_skill>0.9</race_skill>
    </driver>
</custom_ai_drivers>
"#;
    let out = set_team_scalars_str(xml, "Some Team", scalars(1.05, 0.95, 1.0)).unwrap();
    let cars = parse_car_performance_str(&out);
    assert_eq!(cars[0].power_scalar, 1.05);
    assert_eq!(cars[0].weight_scalar, 0.95);
    assert_eq!(cars[0].drag_scalar, 1.00);
    // Inserted as last children, indented like the tags already there.
    assert!(out.contains(
        "        <race_skill>0.9</race_skill>\n        <power_scalar>1.05</power_scalar>\n"
    ));
    assert!(out.contains("        <drag_scalar>1.00</drag_scalar>\n    </driver>"));
}

#[test]
fn test_set_team_scalars_matches_team_case_insensitively() {
    let out = set_team_scalars_str(SAMPLE_WITH_SCALARS, "  williams ", scalars(1.02, 1.0, 1.0));
    assert!(out.is_ok());
}

#[test]
fn test_set_team_scalars_unknown_team_is_an_error() {
    let err =
        set_team_scalars_str(SAMPLE_WITH_SCALARS, "Ferrari", scalars(1.0, 1.0, 1.0)).unwrap_err();
    assert!(err.contains("Ferrari"), "{err}");
}

#[test]
fn test_set_team_scalars_ignores_commented_out_drivers() {
    let xml = r#"<custom_ai_drivers>
    <!-- <driver livery_name="Williams #5 N. Mansell">
        <name>Nigel Mansell</name>
        <power_scalar>1.10</power_scalar>
    </driver> -->
    <driver livery_name="Williams #6 N. Piquet">
        <name>Nelson Piquet</name>
        <power_scalar>1.10</power_scalar>
        <weight_scalar>0.97</weight_scalar>
        <drag_scalar>0.95</drag_scalar>
    </driver>
</custom_ai_drivers>
"#;
    let out = set_team_scalars_str(xml, "Williams", scalars(1.02, 1.0, 1.0)).unwrap();
    assert_eq!(out.matches("<power_scalar>1.10</power_scalar>").count(), 1);
    assert_eq!(out.matches("<power_scalar>1.02</power_scalar>").count(), 1);
    // The commented-out block keeps its original value.
    let comment_end = out.find("-->").unwrap();
    assert!(out[..comment_end].contains("<power_scalar>1.10</power_scalar>"));
}

#[test]
fn test_scalars_validate_rejects_out_of_range_and_nan() {
    assert!(scalars(1.0, 1.0, 1.0).validate().is_ok());
    assert!(scalars(SCALAR_MIN, SCALAR_MAX, 1.0).validate().is_ok());
    let err = scalars(10.8, 1.0, 1.0).validate().unwrap_err();
    assert!(err.contains("power_scalar"), "{err}");
    assert!(scalars(1.0, 0.0, 1.0).validate().is_err());
    assert!(scalars(1.0, 1.0, f32::NAN).validate().is_err());
}

#[test]
fn test_set_team_scalars_writes_file_and_backs_it_up_once() {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_scalar_write_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("F-Test.xml");
    std::fs::write(&file, SAMPLE_WITH_SCALARS).unwrap();

    set_team_scalars(&file, "Williams", scalars(1.02, 1.0, 1.0)).unwrap();
    let backup = dir.join("F-Test.xml.bak");
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        SAMPLE_WITH_SCALARS,
        "the backup must hold the original, unedited file"
    );
    assert_eq!(parse_car_performance(&file)[1].power_scalar, 1.02);

    // A second edit must not overwrite the backup with an already-edited copy.
    set_team_scalars(&file, "Williams", scalars(1.04, 1.0, 1.0)).unwrap();
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        SAMPLE_WITH_SCALARS
    );
    assert_eq!(parse_car_performance(&file)[1].power_scalar, 1.04);

    // An invalid value is rejected before the file is opened.
    assert!(set_team_scalars(&file, "Williams", scalars(99.0, 1.0, 1.0)).is_err());
    assert_eq!(parse_car_performance(&file)[1].power_scalar, 1.04);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_parse_car_performance_collects_the_team_line_up() {
    let cars = parse_car_performance_str(SAMPLE_WITH_SCALARS);
    let williams = cars.iter().find(|c| c.team == "Williams").unwrap();
    // Both drivers, in document order — the scalars dedupe by team but the line-up accumulates.
    assert_eq!(williams.drivers, vec!["Nigel Mansell", "Nelson Piquet"]);
    let ags = cars.iter().find(|c| c.team == "AGS").unwrap();
    assert_eq!(ags.drivers, vec!["Ivan Capelli"]);
}

#[test]
fn test_parse_car_performance_line_up_skips_track_override_blocks() {
    // Mansell's Monza override block carries no <name>, so he is not listed twice.
    let cars = parse_car_performance_str(SAMPLE_WITH_SCALARS);
    let williams = cars.iter().find(|c| c.team == "Williams").unwrap();
    assert_eq!(williams.drivers.len(), 2);
}

#[test]
fn test_parse_car_performance_line_up_dedupes_repeated_names() {
    // The same driver under two liveries of one team is still one name.
    let xml = r#"<custom_ai_drivers>
        <driver livery_name="Lotus #11 E. de Angelis">
            <name>Elio de Angelis</name>
        </driver>
        <driver livery_name="Lotus #11 E. de Angelis (alt)">
            <name>Elio de Angelis</name>
        </driver>
    </custom_ai_drivers>"#;
    let cars = parse_car_performance_str(xml);
    assert_eq!(cars[0].drivers, vec!["Elio de Angelis"]);
}

// ── Per-driver AI attributes ──────────────────────────────────────────────────

/// Two teams, a per-track substitute sharing a livery with the regular entry (as F-Retro_Gen1
/// does), and the `wet_skills` misspelling that file also uses.
const SAMPLE_DRIVER_ATTRS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<custom_ai_drivers>
    <driver livery_name="Goodyear Racing - C. Pace #8">
        <name>Carlos Pace</name>
        <race_skill>0.87</race_skill>
        <qualifying_skill>0.91</qualifying_skill>
        <wet_skills>0.52</wet_skills>
    </driver>
    <driver livery_name="Goodyear Racing - C. Pace #8" tracks="Interlagos_Historic">
        <name>Richard Robarts</name>
        <race_skill>0.64</race_skill>
    </driver>
    <driver livery_name="AGS #31 I. Capelli">
        <name>Ivan Capelli</name>
        <race_skill>0.70</race_skill>
        <wet_skill>0.60</wet_skill>
    </driver>
</custom_ai_drivers>
"#;

#[test]
fn test_parse_driver_attributes_indexes_entries_in_document_order() {
    let drivers = parse_driver_attributes_str(SAMPLE_DRIVER_ATTRS);
    assert_eq!(drivers.len(), 3);
    assert_eq!(drivers[0].index, 0);
    assert_eq!(drivers[0].driver, "Carlos Pace");
    assert_eq!(drivers[0].team, "Goodyear Racing");
    assert_eq!(drivers[1].index, 1);
    assert_eq!(drivers[1].driver, "Richard Robarts");
    assert_eq!(drivers[2].driver, "Ivan Capelli");
}

#[test]
fn test_parse_driver_attributes_reports_track_scoped_entries() {
    let drivers = parse_driver_attributes_str(SAMPLE_DRIVER_ATTRS);
    assert_eq!(drivers[0].tracks, None);
    assert_eq!(drivers[1].tracks.as_deref(), Some("Interlagos_Historic"));
}

#[test]
fn test_parse_driver_attributes_omits_undeclared_attributes() {
    let drivers = parse_driver_attributes_str(SAMPLE_DRIVER_ATTRS);
    assert_eq!(drivers[0].attrs.get("race_skill"), Some(&0.87));
    // Robarts declares only race_skill — the rest are absent, not defaulted to zero.
    assert_eq!(drivers[1].attrs.len(), 1);
    assert!(!drivers[1].attrs.contains_key("aggression"));
}

#[test]
fn test_parse_driver_attributes_reads_the_wet_skills_misspelling() {
    let drivers = parse_driver_attributes_str(SAMPLE_DRIVER_ATTRS);
    // F-Retro_Gen1 spells it `wet_skills`; it still lands under the canonical key.
    assert_eq!(drivers[0].attrs.get("wet_skill"), Some(&0.52));
    assert_eq!(drivers[2].attrs.get("wet_skill"), Some(&0.60));
}

#[test]
fn test_set_driver_attr_updates_only_the_addressed_entry() {
    let out =
        set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 0, "Carlos Pace", "race_skill", 0.9).unwrap();
    let drivers = parse_driver_attributes_str(&out);
    assert_eq!(drivers[0].attrs.get("race_skill"), Some(&0.90));
    // The substitute sharing the livery keeps his own figure.
    assert_eq!(drivers[1].attrs.get("race_skill"), Some(&0.64));
    assert_eq!(drivers[2].attrs.get("race_skill"), Some(&0.70));
}

#[test]
fn test_set_driver_attr_keeps_the_files_own_wet_skill_spelling() {
    let out = set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 0, "Carlos Pace", "wet_skill", 0.7).unwrap();
    assert!(out.contains("<wet_skills>0.70</wet_skills>"), "{out}");
    // Scoped to Pace's own block — Capelli further down legitimately spells it `wet_skill`.
    let pace = &out[..out.find("Richard Robarts").unwrap()];
    assert!(
        !pace.contains("<wet_skill>"),
        "must not add a second, differently spelled tag: {pace}"
    );
    // And the correctly spelled entry elsewhere is untouched.
    assert!(out.contains("<wet_skill>0.60</wet_skill>"), "{out}");
}

#[test]
fn test_set_driver_attr_adds_an_attribute_the_entry_lacked() {
    let out = set_driver_attr_str(
        SAMPLE_DRIVER_ATTRS,
        1,
        "Richard Robarts",
        "aggression",
        0.55,
    )
    .unwrap();
    let drivers = parse_driver_attributes_str(&out);
    assert_eq!(drivers[1].attrs.get("aggression"), Some(&0.55));
    assert_eq!(drivers[0].attrs.get("aggression"), None);
}

#[test]
fn test_set_driver_attr_rejects_a_stale_index() {
    // Index 2 is Capelli; addressing it as Pace means the client's table is out of date.
    let err =
        set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 2, "Carlos Pace", "race_skill", 0.9).unwrap_err();
    assert!(err.contains("Ivan Capelli"), "{err}");
    let missing =
        set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 99, "Carlos Pace", "race_skill", 0.9).unwrap_err();
    assert!(missing.contains("99"), "{missing}");
}

#[test]
fn test_set_driver_attr_rejects_unknown_fields_and_out_of_range_values() {
    let field = set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 0, "Carlos Pace", "power_scalar", 1.0)
        .unwrap_err();
    assert!(field.contains("power_scalar"), "{field}");
    let high =
        set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 0, "Carlos Pace", "race_skill", 8.7).unwrap_err();
    assert!(high.contains("race_skill"), "{high}");
    // A track override may carry a negative offset, so the floor is -1.0, not 0.0.
    assert!(set_driver_attr_str(
        SAMPLE_DRIVER_ATTRS,
        1,
        "Richard Robarts",
        "vehicle_reliability",
        -0.25
    )
    .is_ok());
}

#[test]
fn test_set_driver_attr_leaves_the_rest_of_the_file_byte_identical() {
    let out =
        set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 2, "Ivan Capelli", "race_skill", 0.75).unwrap();
    assert_eq!(out.lines().count(), SAMPLE_DRIVER_ATTRS.lines().count());
    assert!(out.contains(
        r#"<driver livery_name="Goodyear Racing - C. Pace #8" tracks="Interlagos_Historic">"#
    ));
    assert_eq!(out.matches("<race_skill>0.87</race_skill>").count(), 1);
}

#[test]
fn test_set_driver_attr_writes_file_and_backs_it_up() {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ams2_attr_write_{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("F-Test.xml");
    std::fs::write(&file, SAMPLE_DRIVER_ATTRS).unwrap();

    set_driver_attr(&file, 2, "Ivan Capelli", "consistency", 0.81).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("F-Test.xml.bak")).unwrap(),
        SAMPLE_DRIVER_ATTRS
    );
    assert_eq!(
        parse_driver_attributes(&file)[2].attrs.get("consistency"),
        Some(&0.81)
    );

    // A rejected edit leaves the file alone.
    let before = std::fs::read_to_string(&file).unwrap();
    assert!(set_driver_attr(&file, 2, "Ivan Capelli", "race_skill", 9.9).is_err());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);

    std::fs::remove_dir_all(&dir).ok();
}

// ── Phantom entries (liveries AMS2 does not own) ──────────────────────────────

fn livery_set(items: &[&str]) -> std::collections::HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn test_mark_phantom_entries_flags_the_entry_with_no_livery() {
    let mut drivers = parse_driver_attributes_str(SAMPLE_DRIVER_ATTRS);
    // Capelli's livery is not installed; the other two are.
    let installed = livery_set(&["Goodyear Racing - C. Pace #8"]);
    mark_phantom_entries(&mut drivers, Some(&installed));
    assert_eq!(drivers[0].phantom, Some(false));
    // The per-track substitute shares Pace's livery, so it is real too.
    assert_eq!(drivers[1].phantom, Some(false));
    assert_eq!(drivers[2].phantom, Some(true));
}

#[test]
fn test_mark_phantom_entries_leaves_unknown_when_nothing_matches() {
    let mut drivers = parse_driver_attributes_str(SAMPLE_DRIVER_ATTRS);
    let installed = livery_set(&["A Completely Different Class #1 X. Y"]);
    mark_phantom_entries(&mut drivers, Some(&installed));
    // No livery mod for this class — absence proves nothing, so nothing is claimed.
    assert!(drivers.iter().all(|d| d.phantom.is_none()));
    mark_phantom_entries(&mut drivers, None);
    assert!(drivers.iter().all(|d| d.phantom.is_none()));
}

#[test]
fn test_without_phantom_seats_removes_seats_that_can_never_be_filled() {
    let seats = parse_seats_str(ROSTER);
    let all: Vec<String> = seats.iter().map(|s| s.livery.clone()).collect();
    // Everything installed except Brabham #8's De Angelis entry.
    let installed: std::collections::HashSet<String> = all
        .iter()
        .filter(|l| !l.contains("E. De Angelis"))
        .cloned()
        .collect();
    let kept = without_phantom_seats(seats.clone(), Some(&installed));
    assert_eq!(kept.len(), seats.len() - 1);
    assert!(!kept.iter().any(|s| s.driver == "Elio De Angelis"));
    // Warwick shares the Brabham #8 seat under his own livery, so the seat itself survives.
    assert!(kept.iter().any(|s| s.seat == "Brabham #8"));
}

#[test]
fn test_without_phantom_seats_is_a_no_op_when_unverifiable() {
    let seats = parse_seats_str(ROSTER);
    let unrelated = livery_set(&["Some Other Class #1 A. Driver"]);
    assert_eq!(
        without_phantom_seats(seats.clone(), Some(&unrelated)).len(),
        seats.len()
    );
    assert_eq!(
        without_phantom_seats(seats.clone(), None).len(),
        seats.len()
    );
}

#[test]
fn test_phantom_seat_removal_rescues_player_seat_inference() {
    // Reproduces the shipped 1978 problem: Shadow #17 is in the roster but AMS2 has no livery
    // for it, so no AI can ever fill it and it looks free in every session — competing with
    // ATS #9, the seat the player is really in.
    let roster = r#"<custom_ai_drivers>
        <driver livery_name="Shadow #16 H. Stuck"><name>Hans Stuck</name></driver>
        <driver livery_name="Shadow #17 C. Regazzoni"><name>Clay Regazzoni</name></driver>
        <driver livery_name="Arrows #35 R. Patrese"><name>Riccardo Patrese</name></driver>
        <driver livery_name="ATS #9 J. Mass"><name>Jochen Mass</name></driver>
    </custom_ai_drivers>"#;
    let seats = parse_seats_str(roster);
    let car = "Formula Retro Gen2";
    let rows: [(&str, &str, bool); 3] = [
        ("Nightrat", car, true),
        ("Hans Stuck", car, false),
        ("Riccardo Patrese", car, false),
    ];
    let g = grid(&rows);

    // Unfiltered, the dead seat is indistinguishable from the player's, so neither can be named.
    match infer_player_seat(&seats, &g) {
        PlayerSeat::Candidates(v) => {
            let names: Vec<&str> = v.iter().map(|s| s.seat.as_str()).collect();
            assert_eq!(names, vec!["Shadow #17", "ATS #9"]);
        }
        other => panic!("expected two candidates before filtering, got {other:?}"),
    }

    // With the phantom removed, only the player's real seat is left and it resolves outright.
    let installed = livery_set(&[
        "Shadow #16 H. Stuck",
        "Arrows #35 R. Patrese",
        "ATS #9 J. Mass",
    ]);
    let real = without_phantom_seats(seats, Some(&installed));
    match infer_player_seat(&real, &g) {
        PlayerSeat::Derived(seat) => assert_eq!(seat.seat, "ATS #9"),
        other => panic!("expected the player's seat to be derived, got {other:?}"),
    }
}

// ── Composite driver rating ───────────────────────────────────────────────────

fn attrs_of(pairs: &[(&str, f32)]) -> std::collections::BTreeMap<String, f32> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

#[test]
fn test_rating_weights_total_one_hundred() {
    let total: f32 = RATING_WEIGHTS.iter().map(|(_, w)| w).sum();
    assert_eq!(total, 100.0, "a driver at 1.00 everywhere must rate 100");
}

#[test]
fn test_rating_weights_only_use_real_attributes() {
    for (field, _) in RATING_WEIGHTS {
        assert!(
            DRIVER_ATTRS.contains(&field),
            "{field} is weighted but is not an attribute that gets parsed"
        );
    }
}

#[test]
fn test_rating_weights_exclude_non_skill_attributes() {
    // Higher is not better for these, so they must never move the rating.
    for field in [
        "aggression",
        "blue_flag_conceding",
        "weather_tyre_changes",
        "vehicle_reliability",
    ] {
        assert!(
            !RATING_WEIGHTS.iter().any(|(f, _)| *f == field),
            "{field} must not count toward a driver rating"
        );
    }
}

#[test]
fn test_rate_driver_is_100_at_the_ceiling_and_0_at_the_floor() {
    let top: Vec<(&str, f32)> = RATING_WEIGHTS.iter().map(|(f, _)| (*f, 1.0)).collect();
    assert_eq!(rate_driver(&attrs_of(&top)), Some(100.0));
    let bottom: Vec<(&str, f32)> = RATING_WEIGHTS.iter().map(|(f, _)| (*f, 0.0)).collect();
    assert_eq!(rate_driver(&attrs_of(&bottom)), Some(0.0));
}

#[test]
fn test_rate_driver_renormalises_over_declared_attributes_only() {
    // race_skill alone at 0.90 is a 90, not 27 — the other 70 points of weight are absent, not
    // zero, and a sparse entry must be rated on what it declares.
    let sparse = attrs_of(&[("race_skill", 0.90)]);
    assert_eq!(rate_driver(&sparse), Some(90.0));

    // Adding a weaker second attribute pulls it down by that attribute's share of the weight
    // present: (30*0.9 + 15*0.6) / 45 = 0.8.
    let two = attrs_of(&[("race_skill", 0.90), ("qualifying_skill", 0.60)]);
    assert_eq!(rate_driver(&two).map(|r| r.round()), Some(80.0));
}

#[test]
fn test_rate_driver_ignores_unweighted_attributes() {
    let plain = attrs_of(&[("race_skill", 0.80)]);
    let with_noise = attrs_of(&[
        ("race_skill", 0.80),
        ("aggression", 1.0),
        ("blue_flag_conceding", 0.0),
        ("vehicle_reliability", -0.25),
    ]);
    assert_eq!(rate_driver(&plain), rate_driver(&with_noise));
}

#[test]
fn test_rate_driver_needs_race_skill() {
    // Everything but pace: no basis for a number, so none is offered.
    let no_pace = attrs_of(&[("consistency", 0.9), ("stamina", 0.9), ("defending", 0.9)]);
    assert_eq!(rate_driver(&no_pace), None);
    assert_eq!(rate_driver(&attrs_of(&[])), None);
}

#[test]
fn test_parse_driver_attributes_carries_the_rating() {
    let drivers = parse_driver_attributes_str(SAMPLE_DRIVER_ATTRS);
    // Pace has 0.87 at weight 30 and 0.91 at weight 15; wet_skill 0.52 at weight 6.
    // (26.1 + 13.65 + 3.12) / 51 = 0.8406…
    assert_eq!(drivers[0].driver, "Carlos Pace");
    let r = drivers[0].rating.unwrap();
    assert!((r - 84.06).abs() < 0.01, "{r}");
    // Robarts declares race_skill only, so his rating is that alone.
    let robarts = drivers[1].rating.unwrap();
    assert!((robarts - 64.0).abs() < 0.01, "{robarts}");
}

// ── Documented value ranges ───────────────────────────────────────────────────
//
// Reiza publishes these in the thread `UserData/CustomAIDrivers/README.txt` links to. They are
// asserted literally so that loosening them has to be a deliberate edit to a test, not a quiet
// drift in a constant.

#[test]
fn test_scalar_range_matches_the_documented_one() {
    // "Valid values range from 0.900 to 1.100, where 1.000 means no change".
    assert_eq!(SCALAR_MIN, 0.9);
    assert_eq!(SCALAR_MAX, 1.1);
}

#[test]
fn test_scalars_validate_accepts_the_bounds_and_rejects_just_outside() {
    assert!(
        scalars(0.9, 1.0, 1.1).validate().is_ok(),
        "bounds are inclusive"
    );
    for bad in [(0.89, 1.0, 1.0), (1.0, 1.11, 1.0), (1.0, 1.0, 1.2)] {
        let s = scalars(bad.0, bad.1, bad.2);
        assert!(s.validate().is_err(), "{s:?} is outside 0.900-1.100");
    }
}

#[test]
fn test_attr_range_is_zero_to_one_for_personality_values() {
    // "valid personality values range is between 0 and 1 (inclusive)".
    assert_eq!(ATTR_MIN, 0.0);
    assert_eq!(ATTR_MAX, 1.0);
    for field in DRIVER_ATTRS {
        if field == "vehicle_reliability" {
            continue;
        }
        assert_eq!(attr_range(field), (0.0, 1.0), "{field}");
    }
}

#[test]
fn test_vehicle_reliability_is_the_documented_exception() {
    // "if you go below 0.0 or above 1.0, that can be done too" — and the shipped files rely on
    // it, with 166 entries at -0.25 in F-Classic_Gen1 alone.
    let (lo, hi) = attr_range("vehicle_reliability");
    assert!(lo < 0.0 && hi > 1.0, "got {lo}..{hi}");
}

#[test]
fn test_attr_ranges_covers_every_editable_attribute() {
    let ranges = attr_ranges();
    assert_eq!(ranges.len(), DRIVER_ATTRS.len());
    for (field, lo, hi) in ranges {
        assert!(DRIVER_ATTRS.contains(&field));
        assert_eq!((lo, hi), attr_range(field));
    }
}

#[test]
fn test_set_driver_attr_enforces_the_documented_range_per_field() {
    // 1.02 race_skill exists in the shipped F-Classic_Gen2 but is outside the documented range,
    // so the writer refuses it rather than propagating a value the docs call invalid.
    let err =
        set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 0, "Carlos Pace", "race_skill", 1.02).unwrap_err();
    assert!(err.contains("race_skill"), "{err}");
    assert!(set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 0, "Carlos Pace", "race_skill", 1.0).is_ok());
    assert!(set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 0, "Carlos Pace", "race_skill", 0.0).is_ok());
    assert!(
        set_driver_attr_str(SAMPLE_DRIVER_ATTRS, 0, "Carlos Pace", "aggression", -0.1).is_err()
    );

    // The same values are fine for reliability, which the docs leave unbounded.
    for v in [-0.25, 1.15] {
        assert!(
            set_driver_attr_str(
                SAMPLE_DRIVER_ATTRS,
                0,
                "Carlos Pace",
                "vehicle_reliability",
                v
            )
            .is_ok(),
            "reliability {v} must be allowed"
        );
    }
}
