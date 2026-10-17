use super::*;

#[test]
fn test_season_year_known_class_returns_year() {
    assert_eq!(season_year("F-Classic_Gen1"), Some(1986));
    assert_eq!(season_year("F-Vintage_Gen1"), Some(1967));
}

#[test]
fn test_season_year_unknown_class_returns_none() {
    assert_eq!(season_year("F-Some-Unlisted-Class"), None);
}

#[test]
fn test_season_years_table_has_no_duplicate_names() {
    let mut names: Vec<&str> = SEASON_YEARS.iter().map(|(n, _)| *n).collect();
    let before = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), before, "duplicate class name in SEASON_YEARS");
}

#[test]
fn test_an_override_answers_for_a_class_the_table_has_never_heard_of() {
    // The case this exists for: Formula Edge is a fictional car, registered as `FE-G1`, so no
    // shipped table can say what season it models - it is whichever livery mod is installed.
    let mut years = BTreeMap::new();
    years.insert("FE-G1".to_string(), 1995);
    assert_eq!(season_year("FE-G1"), None);
    assert_eq!(season_year_with("FE-G1", &years), Some(1995));
}

#[test]
fn test_an_override_beats_the_shipped_table() {
    let mut years = BTreeMap::new();
    years.insert("F-Classic_Gen1".to_string(), 1987);
    assert_eq!(season_year_with("F-Classic_Gen1", &years), Some(1987));
    // ...and only for the class it names.
    assert_eq!(season_year_with("F-Classic_Gen2", &years), Some(1988));
}

#[test]
fn test_no_overrides_is_the_shipped_table() {
    let none = BTreeMap::new();
    for (class, year) in SEASON_YEARS {
        assert_eq!(season_year_with(class, &none), Some(*year));
    }
    assert_eq!(season_year_with("Formula Renault", &none), None);
}
