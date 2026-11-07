use super::*;

fn teams(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

/// Read a season's team names straight off one of the shipped rosters.
fn shipped(season: &str) -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("custom_ai_files_with_perf_scalars")
        .join(season)
        .join("F-Retro_Gen3.xml");
    crate::roster::custom_ai::list_teams(&path)
}

#[test]
fn test_the_constructor_is_the_name_before_the_engine() {
    assert_eq!(constructor("Spirit-Hart"), "Spirit");
    assert_eq!(constructor("Alfa Romeo"), "Alfa Romeo");
    assert_eq!(constructor("Osella-Alfa Romeo"), "Osella");
}

#[test]
fn test_a_unique_constructor_links_and_a_shared_one_does_not() {
    let links = propose(
        &teams(&["Spirit-Honda", "Lotus-Ford", "Lotus-Renault", "Theodore-Ford"]),
        &teams(&["Spirit-Hart", "Lotus-Renault", "Ferrari"]),
    );
    assert_eq!(
        links,
        [
            Link { from: "Spirit-Honda".into(), to: Some("Spirit-Hart".into()) },
            // Two Lotus teams last season: which one became this season's is not for a name to say…
            Link { from: "Lotus-Ford".into(), to: None },
            // …except for the one still racing under the same name.
            Link { from: "Lotus-Renault".into(), to: Some("Lotus-Renault".into()) },
            // Gone from the grid.
            Link { from: "Theodore-Ford".into(), to: None },
        ]
    );
}

#[test]
fn test_the_player_choice_wins_over_the_proposal() {
    let prev = teams(&["Spirit-Honda", "Lotus-Renault", "Lotus-Ford"]);
    let cur = teams(&["Spirit-Hart", "Lotus-Renault"]);
    let chosen = [
        Link { from: "Lotus-Renault".into(), to: Some("Lotus-Renault".into()) },
        Link { from: "Spirit-Honda".into(), to: None },
    ];
    let resolved = resolve(&prev, &cur, &chosen);
    assert_eq!(resolved[0], (Link { from: "Spirit-Honda".into(), to: None }, Source::Chosen));
    assert_eq!(resolved[1].1, Source::Chosen);
    assert_eq!(resolved[2], (Link { from: "Lotus-Ford".into(), to: None }, Source::Unmatched));
    assert_eq!(successor(&prev, &cur, &chosen, "Lotus-Renault").as_deref(), Some("Lotus-Renault"));
    assert_eq!(successor(&prev, &cur, &chosen, "Spirit-Honda"), None);
    assert_eq!(successor(&prev, &cur, &[], "Spirit-Honda").as_deref(), Some("Spirit-Hart"));
}

#[test]
fn test_the_shipped_1983_to_1984_rosters_link_spirit_and_leave_lotus_open() {
    let (y83, y84) = (shipped("F1_1983"), shipped("F1_1984"));
    assert!(!y83.is_empty() && !y84.is_empty(), "the shipped rosters are readable");
    let links = propose(&y83, &y84);
    let to = |from: &str| links.iter().find(|l| l.from == from).and_then(|l| l.to.clone());
    assert_eq!(to("Spirit-Honda").as_deref(), Some("Spirit-Hart"));
    assert_eq!(to("Williams-Ford").as_deref(), Some("Williams-Honda"));
    assert_eq!(to("McLaren-Ford").as_deref(), Some("McLaren-TAG"));
    assert_eq!(to("Lotus-Ford"), None, "1983 ran two Lotus teams");
}

#[test]
fn test_a_team_under_the_same_name_carries_over_as_it_always_did() {
    let links = propose(&teams(&["Ferrari", "Lotus-Ford", "Lotus-Renault"]), &teams(&["Ferrari", "Lotus-Renault"]));
    assert_eq!(links[0].to.as_deref(), Some("Ferrari"));
    assert_eq!(links[2].to.as_deref(), Some("Lotus-Renault"));
}
