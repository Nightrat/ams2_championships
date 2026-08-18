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
