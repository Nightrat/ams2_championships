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
    assert_eq!(extract_team_name("Brabham-Repco #1 J. Brabham"), "Brabham-Repco");
}

#[test]
fn test_extract_team_name_strips_leading_year() {
    assert_eq!(extract_team_name("1986 AGS #31 - I. Capelli"), "AGS");
    assert_eq!(extract_team_name("1988 Eurobrun #32 - O. Larrauri"), "Eurobrun");
}

#[test]
fn test_extract_team_name_no_hash_returns_trimmed_input() {
    assert_eq!(extract_team_name("  Some Team Only  "), "Some Team Only");
}

#[test]
fn test_extract_team_name_driver_before_number_with_dash_separator() {
    // F-Retro_Gen1.xml's convention: "Team - Driver #Num", unlike the more common
    // "Team #Num Driver" / "Team #Num - Driver" used elsewhere.
    assert_eq!(extract_team_name("Marlboro Team Texaco - E. Fittipaldi #5"), "Marlboro Team Texaco");
    // Team name itself contains an unspaced hyphen — must not be mistaken for the separator.
    assert_eq!(extract_team_name("Dalton-Amon Int. - C. Amon #22"), "Dalton-Amon Int.");
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
    // No closing </driver>, still should not panic — livery captured with whatever block remains.
    assert!(map.get("Unclosed").is_none() || map.get("Unclosed").is_some());
}

#[test]
fn test_parse_driver_teams_file_not_found_returns_empty() {
    let map = parse_driver_teams(std::path::Path::new("Z:/does/not/exist_nope.xml"));
    assert!(map.is_empty());
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
        &["F-Classic_Gen1", "F-Vintage_Gen1", "F-Retro_Gen1", "F-Unmapped-Class"],
    );
    // Written in a deliberately non-chronological, non-alphabetical order on disk.
    std::fs::write(ai_dir.join("F-Classic_Gen1.xml"), SAMPLE_WITH_SCALARS).unwrap(); // 1986
    std::fs::write(ai_dir.join("F-Vintage_Gen1.xml"), SAMPLE_WITH_SCALARS).unwrap(); // 1967
    std::fs::write(ai_dir.join("F-Retro_Gen1.xml"), SAMPLE_WITH_SCALARS).unwrap();   // 1974
    std::fs::write(ai_dir.join("F-Unmapped-Class.xml"), SAMPLE_WITH_SCALARS).unwrap(); // no year

    let classes = class_performance(&ai_dir);
    let names: Vec<&str> = classes.iter().map(|c| c.class.as_str()).collect();
    // Chronological: 1967, 1974, 1986, then the unmapped class last.
    assert_eq!(names, vec!["F-Vintage_Gen1", "F-Retro_Gen1", "F-Classic_Gen1", "F-Unmapped-Class"]);
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
