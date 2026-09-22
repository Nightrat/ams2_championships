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

use std::collections::{HashMap, HashSet};
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

/// One `<LIVERY_OVERRIDE>`: the name a `livery_name` must match, and the preview picture the
/// entry declares for the game's own UI, if it declares one.
#[derive(Clone, Debug, PartialEq)]
pub struct ManifestEntry {
    pub name: String,
    /// The `PREVIEWIMAGE` path as written, relative to the manifest's own folder.
    pub preview: Option<String>,
}

/// The `<LIVERY_OVERRIDE>` entries one manifest declares.
///
/// Only the `LIVERY_OVERRIDE` opening tag is inspected for a name: `TEXTURE` elements carry a
/// `NAME` too (`NAME="BODY"`), and reading those would pollute the set with texture slot names.
/// Commented-out blocks are dropped first — the manifests are edited copies of Reiza's template,
/// which ships with commented examples, and the shipped `*-all-liveries.txt` listings use a
/// `LIVERY=" ## "` placeholder rather than a real id.
///
/// A `PREVIEWIMAGE` belongs to the entry it sits inside, so the search for one stops at whichever
/// comes first: the entry's own closing tag, or the next entry's opening one. A self-closing
/// `<LIVERY_OVERRIDE ... />` has no children at all and so never has a preview.
pub fn parse_manifest_entries(xml: &str) -> Vec<ManifestEntry> {
    let xml = crate::custom_ai::strip_comments(xml);
    let mut out = Vec::new();
    let mut rest = xml.as_str();
    while let Some(idx) = rest.find("<LIVERY_OVERRIDE") {
        rest = &rest[idx..];
        let Some(end) = rest.find('>') else { break };
        let open = &rest[..=end];
        let name = crate::custom_ai::attr_value(open, "NAME").map(|n| n.trim().to_string());
        rest = &rest[end + 1..];
        if let Some(name) = name {
            let body_end = [rest.find("</LIVERY_OVERRIDE"), rest.find("<LIVERY_OVERRIDE")]
                .into_iter()
                .flatten()
                .min()
                .unwrap_or(rest.len());
            let preview = (!open.ends_with("/>"))
                .then(|| find_preview(&rest[..body_end]))
                .flatten();
            out.push(ManifestEntry { name, preview });
        }
    }
    out
}

/// The `PATH` of the first `<PREVIEWIMAGE>` in one entry's body.
fn find_preview(body: &str) -> Option<String> {
    let at = body.find("<PREVIEWIMAGE")?;
    let end = body[at..].find('>')? + at;
    let path = crate::custom_ai::attr_value(&body[at..=end], "PATH")?
        .trim()
        .to_string();
    (!path.is_empty()).then_some(path)
}

/// Livery names declared by one manifest, without the previews.
pub fn parse_manifest_str(xml: &str) -> Vec<String> {
    parse_manifest_entries(xml)
        .into_iter()
        .map(|e| e.name)
        .collect()
}

/// Which car model declares a livery, and where its preview picture lives.
#[derive(Clone, Debug)]
struct Declared {
    /// The folder under `Overrides`, which is also the car model's name.
    model: String,
    /// The preview's path relative to the `Overrides` folder, forward-slashed. `None` when the
    /// entry declares no `PREVIEWIMAGE` — most do, but nothing obliges one to.
    preview: Option<String>,
}

/// Every livery the installed manifests declare, and what each one carries.
///
/// One scan of the whole `Overrides` folder, because the manifests are per car *model* while
/// everything asking about them works in car *classes*, and a class's cars are spread across
/// several models.
pub struct InstalledLiveries {
    by_name: HashMap<String, Vec<Declared>>,
}

impl InstalledLiveries {
    /// The names alone, for the phantom check — see [`phantom_liveries`].
    pub fn names(&self) -> HashSet<String> {
        self.by_name.keys().cloned().collect()
    }

    /// The preview picture for each of `roster`'s livery names that has one, as a path relative
    /// to the `Overrides` folder.
    ///
    /// **A livery name does not identify a car model.** "McLaren-Honda #1 A. Senna" is declared
    /// by the 1991 McLaren and by the 1992 one, and a `CustomAIDrivers` file names no model — the
    /// game resolves it by the car each grid slot is actually driving, which is not recorded
    /// anywhere readable. Where two models declare the same name, the one taken is whichever
    /// declares more of *this* roster besides: a 1991 grid matches the 1991 car's manifest almost
    /// entirely and the 1992 car's barely at all. Ties fall to the first model alphabetically, so
    /// the answer does not depend on the order the folder happened to be read in.
    pub fn previews_for(&self, roster: &[String]) -> HashMap<String, String> {
        let mut overlap: HashMap<&str, usize> = HashMap::new();
        for name in roster {
            for declared in self.by_name.get(name).into_iter().flatten() {
                *overlap.entry(declared.model.as_str()).or_default() += 1;
            }
        }
        let mut out = HashMap::new();
        for name in roster {
            let best = self
                .by_name
                .get(name)
                .into_iter()
                .flatten()
                .filter(|d| d.preview.is_some())
                .min_by_key(|d| {
                    let shared = overlap.get(d.model.as_str()).copied().unwrap_or(0);
                    (std::cmp::Reverse(shared), d.model.as_str())
                });
            if let Some(preview) = best.and_then(|d| d.preview.clone()) {
                out.insert(name.clone(), preview);
            }
        }
        out
    }
}

/// Reads every installed manifest under `Overrides`.
///
/// Only `<model>/<model>.xml` is read. The `<model>_dist.xml` beside it is Reiza's template — it
/// declares no real liveries — and is skipped.
///
/// `None` when the folder cannot be read at all. An existing folder with no manifests gives an
/// empty index, which [`phantom_liveries`] also treats as unverifiable.
pub fn installed_liveries(custom_ai_dir: &Path) -> Option<InstalledLiveries> {
    let dir = overrides_dir(custom_ai_dir)?;
    let entries = fs::read_dir(&dir).ok()?;
    let mut by_name: HashMap<String, Vec<Declared>> = HashMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(model) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let manifest = path.join(format!("{model}.xml"));
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        for item in parse_manifest_entries(&text) {
            // The manifest writes Windows separators; a URL and a `Path::join` both want the
            // other kind, and the model folder is what makes the path answerable from the
            // `Overrides` root rather than from one manifest's own directory.
            let preview = item
                .preview
                .map(|p| format!("{model}/{}", p.replace('\\', "/")));
            by_name.entry(item.name).or_default().push(Declared {
                model: model.to_string(),
                preview,
            });
        }
    }
    Some(InstalledLiveries { by_name })
}

/// Every livery name declared by every installed manifest under `Overrides`.
pub fn installed_livery_names(custom_ai_dir: &Path) -> Option<HashSet<String>> {
    installed_liveries(custom_ai_dir).map(|l| l.names())
}

/// The preview file a request names, relative to the `Overrides` folder, or `None` when it names
/// something that is not one.
///
/// The paths handed out are ones this program produced from the manifests, but they come *back*
/// through a URL anyone can type, so they are re-checked rather than trusted. Only plain path
/// components are allowed — no `..`, no root, no drive letter — only a `.dds`, and the resolved
/// file must still sit inside `Overrides`, which is the check that also catches a link pointing
/// out of it.
pub fn preview_file(overrides: &Path, requested: &str) -> Option<PathBuf> {
    use std::path::Component;
    if !requested.to_ascii_lowercase().ends_with(".dds") {
        return None;
    }
    let relative = Path::new(requested);
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let root = overrides.canonicalize().ok()?;
    let resolved = overrides.join(relative).canonicalize().ok()?;
    resolved.starts_with(&root).then_some(resolved)
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
