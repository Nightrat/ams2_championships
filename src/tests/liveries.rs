use super::*;

/// Shaped like a real override manifest: a couple of liveries, a HELMET_OVERRIDE that carries no
/// NAME, TEXTURE elements whose NAME is a texture slot rather than a livery, and a commented-out
/// example of the kind Reiza's template ships with.
const MANIFEST: &str = r#"<USER_OVERRIDES>
    <LIVERY_OVERRIDE LIVERY="12" NAME="Shadow-Ford Cosworth #16 H-J. Stuck" BASELIVERY="Default">
        <PREVIEWIMAGE PATH="F1_1978_Season\preview16.dds" />
        <TEXTURE NAME="BODY" PATH="F1_1978_Season\body16.dds" />
    </LIVERY_OVERRIDE>
    <HELMET_OVERRIDE LIVERY="12" BASEHELMET="DEFAULT">
        <TEXTURE NAME="BODY_DIFF" PATH="F1_1978_Season\stuck_helmet.dds" />
    </HELMET_OVERRIDE>
    <LIVERY_OVERRIDE LIVERY="13" NAME="Arrows-Ford Cosworth #35 R. Patrese" BASELIVERY="Default">
        <TEXTURE NAME="BODY" PATH="F1_1978_Season\body35.dds" />
    </LIVERY_OVERRIDE>
    <!-- <LIVERY_OVERRIDE LIVERY="99" NAME="My Racing Team #12" BASELIVERY="Default">
      <TEXTURE NAME="BODY" PATH="myteam\body12.dds" />
    </LIVERY_OVERRIDE> -->
</USER_OVERRIDES>
"#;

#[test]
fn test_parse_manifest_reads_livery_names_only() {
    let names = parse_manifest_str(MANIFEST);
    assert_eq!(
        names,
        vec![
            "Shadow-Ford Cosworth #16 H-J. Stuck",
            "Arrows-Ford Cosworth #35 R. Patrese"
        ]
    );
}

#[test]
fn test_parse_manifest_ignores_texture_slot_names() {
    // TEXTURE elements carry NAME="BODY" etc; reading those would poison the livery set.
    let names = parse_manifest_str(MANIFEST);
    assert!(!names.iter().any(|n| n == "BODY" || n == "BODY_DIFF"));
}

#[test]
fn test_parse_manifest_ignores_commented_out_examples() {
    let names = parse_manifest_str(MANIFEST);
    assert!(!names.iter().any(|n| n.contains("My Racing Team")));
}

#[test]
fn test_parse_manifest_empty_input() {
    assert!(parse_manifest_str("<USER_OVERRIDES></USER_OVERRIDES>").is_empty());
}

fn set(items: &[&str]) -> std::collections::HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn test_phantom_liveries_flags_only_unmatched_entries() {
    let installed = set(&["Shadow #16 Stuck", "Arrows #35 Patrese"]);
    let roster = vec![
        "Shadow #16 Stuck".to_string(),
        "Shadow #17 Regazzoni".to_string(),
        "Arrows #35 Patrese".to_string(),
    ];
    let phantoms = phantom_liveries(Some(&installed), &roster).unwrap();
    assert_eq!(phantoms, set(&["Shadow #17 Regazzoni"]));
}

#[test]
fn test_phantom_liveries_unverifiable_when_nothing_matches() {
    // An unmodded class: its liveries are still sealed in the game's paks, so every entry
    // "missing" proves nothing. The user's Formula Renault file is exactly this case.
    let installed = set(&["Some Other Class #1 A. Driver"]);
    let roster = vec!["Formula Renault #4 X".to_string()];
    assert_eq!(phantom_liveries(Some(&installed), &roster), None);
}

#[test]
fn test_phantom_liveries_unverifiable_without_manifests() {
    assert_eq!(phantom_liveries(None, &["Anything".to_string()]), None);
    // An empty installed set can never match, so it lands in the same unverifiable case.
    let empty = std::collections::HashSet::new();
    assert_eq!(
        phantom_liveries(Some(&empty), &["Anything".to_string()]),
        None
    );
}

#[test]
fn test_phantom_liveries_empty_when_every_entry_matches() {
    let installed = set(&["A", "B", "C"]);
    let roster = vec!["A".to_string(), "B".to_string()];
    assert_eq!(
        phantom_liveries(Some(&installed), &roster),
        Some(std::collections::HashSet::new())
    );
}

#[test]
fn test_overrides_dir_is_derived_from_the_custom_ai_folder() {
    let ai = Path::new("D:/Games/Automobilista 2/UserData/CustomAIDrivers");
    let dir = overrides_dir(ai).unwrap();
    assert!(
        dir.ends_with("Vehicles/Textures/CustomLiveries/Overrides"),
        "{dir:?}"
    );
    assert!(dir.starts_with("D:/Games/Automobilista 2"), "{dir:?}");
}

#[test]
fn test_installed_livery_names_reads_manifests_and_skips_dist_templates() {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ams2_liv_{ns}"));
    let ai_dir = root.join("UserData").join("CustomAIDrivers");
    let overrides = root
        .join("Vehicles")
        .join("Textures")
        .join("CustomLiveries")
        .join("Overrides");
    std::fs::create_dir_all(&ai_dir).unwrap();
    let model = overrides.join("formula_retro_g2");
    std::fs::create_dir_all(&model).unwrap();
    std::fs::write(model.join("formula_retro_g2.xml"), MANIFEST).unwrap();
    // The _dist template declares no real liveries and must be skipped.
    std::fs::write(
        model.join("formula_retro_g2_dist.xml"),
        r#"<USER_OVERRIDES>
        <LIVERY_OVERRIDE LIVERY=" ## " NAME="Template Entry" BASELIVERY="Default" />
        </USER_OVERRIDES>"#,
    )
    .unwrap();
    // A model folder with only a template contributes nothing.
    let bare = overrides.join("some_other_car");
    std::fs::create_dir_all(&bare).unwrap();
    std::fs::write(bare.join("some_other_car_dist.xml"), MANIFEST).unwrap();

    let names = installed_livery_names(&ai_dir).unwrap();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.contains("Arrows-Ford Cosworth #35 R. Patrese"));
    assert!(!names.contains("Template Entry"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_installed_livery_names_none_when_the_folder_is_missing() {
    let ai = std::env::temp_dir().join("ams2_no_such_install/UserData/CustomAIDrivers");
    assert_eq!(installed_livery_names(&ai), None);
}
