//! Which liveries AMS2 will actually put on a grid.
//!
//! A `CustomAIDrivers` file cannot create a car. It only binds a name, skills and pace scalars to
//! a livery the game already owns, matched on the `livery_name` attribute. A `livery_name` that
//! matches nothing is silently ignored — no error, no warning, the driver simply never spawns.
//! The 1978 roster shipped with 24 entries against 22 real liveries, so two drivers could never
//! appear and two seats looked permanently empty to the seat accounting in [`crate::custom_ai`].
//!
//! The game's own livery data is sealed in Oodle-compressed `*_livery.bff` paks, so it cannot be
//! read directly. What *can* be read is the livery-override manifest a livery mod installs at
//! `<install>/Vehicles/Textures/CustomLiveries/Overrides/<model>/<model>.xml`, whose
//! `<LIVERY_OVERRIDE NAME="...">` entries carry exactly the strings a `livery_name` must match.
//!
//! That makes this a *partial* index: it covers modded car models and nothing else. A class with
//! no livery mod yields no names at all, and absence there proves nothing — hence the
//! can't-verify handling in [`phantom_liveries`], mirroring what
//! [`crate::custom_ai::known_class_names`] does when the class registry is unreadable.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// `<install>/Vehicles/Textures/CustomLiveries/Overrides`, derived from the configured
/// `CustomAIDrivers` folder by walking up to the install root — the same two-level climb
/// [`crate::custom_ai::known_class_names`] uses. `None` if that derivation fails.
pub fn overrides_dir(custom_ai_dir: &Path) -> Option<PathBuf> {
    let install_root = custom_ai_dir.parent()?.parent()?;
    Some(
        install_root
            .join("Vehicles")
            .join("Textures")
            .join("CustomLiveries")
            .join("Overrides"),
    )
}

/// Livery names declared by one manifest's `<LIVERY_OVERRIDE>` entries.
///
/// Only the `LIVERY_OVERRIDE` opening tag is inspected: `TEXTURE` elements carry a `NAME` too
/// (`NAME="BODY"`), and reading those would pollute the set with texture slot names. Commented-out
/// blocks are dropped first — the manifests are edited copies of Reiza's template, which ships
/// with commented examples, and the shipped `*-all-liveries.txt` listings use a `LIVERY=" ## "`
/// placeholder rather than a real id.
pub fn parse_manifest_str(xml: &str) -> Vec<String> {
    let xml = crate::custom_ai::strip_comments(xml);
    let mut out = Vec::new();
    let mut rest = xml.as_str();
    while let Some(idx) = rest.find("<LIVERY_OVERRIDE") {
        rest = &rest[idx..];
        let Some(end) = rest.find('>') else { break };
        if let Some(name) = crate::custom_ai::attr_value(&rest[..=end], "NAME") {
            out.push(name.trim().to_string());
        }
        rest = &rest[end + 1..];
    }
    out
}

/// Every livery name declared by every installed manifest under `Overrides`.
///
/// Only `<model>/<model>.xml` is read. The `<model>_dist.xml` beside it is Reiza's template — it
/// declares no real liveries — and is skipped.
///
/// `None` when the folder cannot be read at all. An existing folder with no manifests gives an
/// empty set, which [`phantom_liveries`] also treats as unverifiable.
pub fn installed_livery_names(custom_ai_dir: &Path) -> Option<HashSet<String>> {
    let dir = overrides_dir(custom_ai_dir)?;
    let entries = fs::read_dir(&dir).ok()?;
    let mut names = HashSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(model) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let manifest = path.join(format!("{model}.xml"));
        if let Ok(text) = fs::read_to_string(&manifest) {
            names.extend(parse_manifest_str(&text));
        }
    }
    Some(names)
}

/// The `livery_name`s in `roster` that no installed livery matches — entries AMS2 ignores.
///
/// `None` means "cannot verify", and callers must treat it as *no* phantoms rather than as all of
/// them. That covers two cases which look identical from here and are both common:
/// - the manifests could not be read at all (`installed` is `None`);
/// - not one roster entry matches, so this class has no livery mod installed and its real
///   liveries are still sealed in the game's paks. Flagging the whole roster there would be
///   wrong every time; the user's unmodded Formula Renault file is exactly this case.
pub fn phantom_liveries(
    installed: Option<&HashSet<String>>,
    roster: &[String],
) -> Option<HashSet<String>> {
    let installed = installed?;
    let (present, missing): (Vec<&String>, Vec<&String>) =
        roster.iter().partition(|l| installed.contains(l.as_str()));
    if present.is_empty() {
        return None;
    }
    Some(missing.into_iter().cloned().collect())
}

#[cfg(test)]
#[path = "tests/liveries.rs"]
mod tests;
