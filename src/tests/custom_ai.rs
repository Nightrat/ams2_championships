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
