use std::fs;
use std::sync::OnceLock;

/// Path to the minimal fixture XML.
const FIXTURE_XML: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/minimal.xml");

/// Raw XML content of the fixture, loaded once.
fn fixture_xml_str() -> &'static str {
    static XML: OnceLock<String> = OnceLock::new();
    XML.get_or_init(|| fs::read_to_string(FIXTURE_XML).expect("fixture xml must exist"))
}

/// Combined championships + stats fragment HTML, generated from the fixture XML.
fn fixture_html() -> &'static str {
    static HTML: OnceLock<String> = OnceLock::new();
    HTML.get_or_init(|| {
        let (champ_html, stats_html) =
            ams2_championship::import_fragment_from_xml_str(fixture_xml_str())
                .expect("import_fragment_from_xml_str should succeed");
        champ_html + &stats_html
    })
}

// ── basic file I/O ────────────────────────────────────────────────────────────

#[test]
fn test_convert_creates_output_file() {
    let out = format!("{}/target/test_creates.html", env!("CARGO_MANIFEST_DIR"));
    ams2_championship::convert(FIXTURE_XML, &out).expect("convert should not error");
    assert!(
        std::path::Path::new(&out).exists(),
        "output HTML file should be created"
    );
    let _ = fs::remove_file(&out);
}

#[test]
fn test_convert_output_is_valid_html_skeleton() {
    let out = format!("{}/target/test_skeleton.html", env!("CARGO_MANIFEST_DIR"));
    fs::create_dir_all(format!("{}/target", env!("CARGO_MANIFEST_DIR"))).ok();
    ams2_championship::convert(FIXTURE_XML, &out).expect("convert should not error");
    let html = fs::read_to_string(&out).expect("output file should exist");
    let _ = fs::remove_file(&out);
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<html"));
    assert!(html.contains("</html>"));
}

#[test]
fn test_convert_missing_xml_returns_error() {
    let result = ams2_championship::convert("/nonexistent/Championships.xml", "/tmp/out.html");
    assert!(result.is_err(), "missing XML should return an error");
}

// ── championship content ──────────────────────────────────────────────────────

#[test]
fn test_convert_contains_championship_name() {
    assert!(
        fixture_html().contains("Test Cup"),
        "championship name should appear in output"
    );
}

#[test]
fn test_convert_contains_class_name() {
    assert!(fixture_html().contains("GT3"));
}

#[test]
fn test_convert_contains_track_names() {
    let html = fixture_html();
    assert!(html.contains("Spa"));
    assert!(html.contains("Monza"));
}

#[test]
fn test_convert_player_marked_with_you_tag() {
    assert!(
        fixture_html().contains("player-tag"),
        "player should be highlighted with player-tag class"
    );
}

// ── driver stats table ────────────────────────────────────────────────────────

#[test]
fn test_convert_driver_stats_section_present() {
    let html = fixture_html();
    assert!(html.contains("Driver Statistics"));
    assert!(html.contains("stats-table"));
}

#[test]
fn test_convert_driver_stats_has_dnf_column() {
    assert!(
        fixture_html().contains(">DNF<"),
        "stats table should have DNF column header"
    );
}

#[test]
fn test_convert_player_dnf_counted() {
    let html = fixture_html();
    let stats_start = html
        .find(r#"id="stats-table""#)
        .expect("stats-table element must exist");
    let stats_html = &html[stats_start..];
    let pr_start = stats_html
        .find("player-row")
        .expect("player-row must exist in stats table");
    let pr_html = &stats_html[pr_start..];
    let row_end = pr_html.find("</tr>").expect("player row must close");
    let row = &pr_html[..row_end];
    assert!(
        row.contains(r#"<td class="stat-num">1</td>"#),
        "player row should show DNF=1 somewhere in its cells, got: {row}"
    );
}

#[test]
fn test_convert_player_race_count() {
    let html = fixture_html();
    let stats_start = html
        .find(r#"id="stats-table""#)
        .expect("stats-table element must exist");
    let stats_html = &html[stats_start..];
    let pr_start = stats_html
        .find("player-row")
        .expect("player-row must exist in stats");
    let pr_html = &stats_html[pr_start..];
    let row_end = pr_html.find("</tr>").expect("player row must close");
    let row = &pr_html[..row_end];
    assert!(
        row.contains(r#"<td class="stat-num">2</td>"#),
        "player should have 2 races, row: {row}"
    );
}

#[test]
fn test_convert_player_win_count() {
    let html = fixture_html();
    let stats_start = html
        .find(r#"id="stats-table""#)
        .expect("stats-table element must exist");
    let stats_html = &html[stats_start..];
    let pr_start = stats_html
        .find("player-row")
        .expect("player-row must exist in stats");
    let pr_html = &stats_html[pr_start..];
    let row_end = pr_html.find("</tr>").expect("player row must close");
    let row = &pr_html[..row_end];
    assert!(
        row.contains(r#"<td class="stat-num">1</td>"#),
        "player should have wins=1, row: {row}"
    );
}

#[test]
fn test_convert_ai_driver_dnf_is_zero() {
    let html = fixture_html();
    let stats_start = html
        .find(r#"id="stats-table""#)
        .expect("stats-table element must exist");
    let tbody_start = html[stats_start..]
        .find("<tbody>")
        .expect("<tbody> must exist");
    let tbody = &html[stats_start + tbody_start..];
    let ai_start = tbody
        .find("Bot Alpha")
        .expect("Bot Alpha must appear in stats");
    let before = &tbody[..ai_start];
    let tr_start = before.rfind("<tr>").expect("must have opening <tr>");
    let tr_html = &tbody[tr_start..];
    let row_end = tr_html.find("</tr>").expect("row must close");
    let row = &tr_html[..row_end];
    assert!(
        !row.contains("player-row"),
        "Bot Alpha should not be the player row"
    );
    assert!(
        row.contains(r#"<td class="stat-num">0</td>"#),
        "AI row should contain a 0 (DNF=0), row: {row}"
    );
}
