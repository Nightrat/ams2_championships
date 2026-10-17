use super::*;

#[test]
fn test_lap_times_read_as_the_web_ui_writes_them() {
    assert_eq!(lap_time(0.0), DASH);
    assert_eq!(lap_time(-1.0), DASH);
    assert_eq!(lap_time(9.5), "09.500");
    assert_eq!(lap_time(31.234), "31.234");
    assert_eq!(lap_time(83.456), "1:23.456");
    assert_eq!(lap_time(125.0), "2:05.000");
}

#[test]
fn test_utc_dates_match_known_timestamps() {
    let at = |y, mo, d, h, mi, s| Civil { year: y, month: mo, day: d, hour: h, minute: mi, second: s };
    assert_eq!(utc(0), at(1970, 1, 1, 0, 0, 0));
    // A leap day, and a century that is a leap year.
    assert_eq!(utc(951_782_400), at(2000, 2, 29, 0, 0, 0));
    assert_eq!(utc(1_709_210_096), at(2024, 2, 29, 12, 34, 56));
    // The last second of a year rolls into the next on the following second.
    assert_eq!(utc(1_735_689_599), at(2024, 12, 31, 23, 59, 59));
    assert_eq!(utc(1_735_689_600), at(2025, 1, 1, 0, 0, 0));
}

#[test]
fn test_a_date_is_minutes_not_seconds_and_zero_is_no_date() {
    assert_eq!(date_time(0), "");
    let shown = date_time(1_709_210_096);
    // Local time, so only the shape is fixed: YYYY-MM-DD HH:MM.
    assert_eq!(shown.len(), 16, "{shown}");
    assert_eq!(&shown[4..5], "-");
    assert_eq!(&shown[10..11], " ");
    assert_eq!(&shown[13..14], ":");
}

#[test]
fn test_credits_group_thousands_and_keep_debt_negative() {
    assert_eq!(credits(0), "0");
    assert_eq!(credits(999), "999");
    assert_eq!(credits(1_000), "1,000");
    assert_eq!(credits(1_500_000), "1,500,000");
    assert_eq!(credits(-25_000), "-25,000");
}

#[test]
fn test_track_and_ordinal() {
    assert_eq!(track("Spa", ""), "Spa");
    assert_eq!(track("Spa", "GP"), "Spa \u{2013} GP");
    let got: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 101, 111].iter().map(|n| ordinal(*n)).collect();
    assert_eq!(got, ["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "101st", "111th"]);
}
