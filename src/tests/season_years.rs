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
