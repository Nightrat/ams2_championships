use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

/// List `*.xml` files directly inside `dir`, sorted alphabetically.
/// Returns an empty list if the directory does not exist or can't be read.
pub fn list_files(dir: &Path) -> Vec<String> {
    let mut files: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let is_xml = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("xml"))
                .unwrap_or(false);
            if is_xml {
                path.file_name().and_then(|n| n.to_str()).map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

/// Parse an AMS2 Custom AI Driver XML file into a map of driver display name -> livery/team name.
/// Returns an empty map if the file can't be read.
pub fn parse_driver_teams(path: &Path) -> HashMap<String, String> {
    match fs::read_to_string(path) {
        Ok(content) => parse_driver_teams_str(&content),
        Err(_) => HashMap::new(),
    }
}

/// Distinct team/livery names appearing in a Custom AI Driver XML file, sorted alphabetically.
/// Used to offer the same team names for a manual "player team" pick.
pub fn list_teams(path: &Path) -> Vec<String> {
    let mut teams: Vec<String> = parse_driver_teams(path)
        .into_values()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    teams.sort();
    teams
}

fn strip_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn attr_value<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let needle = format!("{attr}=\"");
    let start = tag.find(needle.as_str())? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn element_text<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(open.as_str())? + open.len();
    let end = block[start..].find(close.as_str())? + start;
    Some(block[start..end].trim())
}

/// Reduces a `livery_name` attribute to just the car/team name, dropping the leading season
/// year (e.g. "1986"), the car number ("#17"), and the driver name — whether the driver comes
/// after the number ("Team #17 Driver") or before it, separated by " - " ("Team - Driver #17").
/// Examples:
///   "Brabham-Repco #1 J. Brabham"        -> "Brabham-Repco"
///   "1986 AGS #31 - I. Capelli"          -> "AGS"
///   "Marlboro Team Texaco - E. Fittipaldi #5" -> "Marlboro Team Texaco"
fn extract_team_name(livery: &str) -> String {
    let livery = livery.trim();
    let without_year = match livery.split_once(' ') {
        Some((year, rest)) if year.len() == 4 && year.bytes().all(|b| b.is_ascii_digit()) => rest,
        _ => livery,
    };
    match without_year.find(" #") {
        Some(hash_idx) => {
            let before_hash = &without_year[..hash_idx];
            // "Team - Driver #Num": the driver sits between a " - " separator and the number.
            match before_hash.find(" - ") {
                Some(dash_idx) => before_hash[..dash_idx].trim().to_string(),
                None => before_hash.trim().to_string(),
            }
        }
        None => without_year.trim().to_string(),
    }
}

/// Walks primary `<driver>` blocks (those carrying a `<name>` tag — track-specific override
/// blocks repeat `livery_name` but omit `<name>`, and are skipped) in document order.
/// Yields `(livery_name, block_text)` pairs.
fn primary_driver_blocks(xml: &str) -> Vec<(String, String)> {
    let xml = strip_comments(xml);
    let mut out = Vec::new();
    let mut rest = xml.as_str();
    while let Some(tag_start) = rest.find("<driver") {
        rest = &rest[tag_start..];
        let Some(tag_end) = rest.find('>') else { break };
        let tag = &rest[..=tag_end];
        let self_closing = tag.trim_end().ends_with("/>");
        let livery = attr_value(tag, "livery_name").map(|s| s.to_string());

        let block_end = if self_closing {
            tag_end + 1
        } else if let Some(close) = rest.find("</driver>") {
            close + "</driver>".len()
        } else {
            rest.len()
        };
        let block = &rest[..block_end];

        if let (Some(_), Some(livery)) = (element_text(block, "name"), &livery) {
            out.push((livery.clone(), block.to_string()));
        }

        rest = &rest[block_end..];
    }
    out
}

/// Parses `<driver livery_name="..."><name>...</name>...</driver>` blocks.
/// Track-specific override blocks (which repeat `livery_name` but omit `<name>`) are skipped —
/// only the primary block per driver carries the display name.
pub fn parse_driver_teams_str(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (livery, block) in primary_driver_blocks(xml) {
        if let Some(name) = element_text(&block, "name") {
            map.entry(name.to_string()).or_insert_with(|| extract_team_name(&livery));
        }
    }
    map
}

/// One car's tuned physical performance within a class, as found in a `CustomAIDrivers` XML file.
#[derive(Clone, Debug, PartialEq)]
pub struct CarPerformance {
    pub team: String,
    pub power_scalar: f32,
    pub weight_scalar: f32,
    pub drag_scalar: f32,
}

fn parse_scalar(block: &str, tag: &str) -> f32 {
    element_text(block, tag)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0)
}

/// Parses `power_scalar`/`weight_scalar`/`drag_scalar` per team from a `CustomAIDrivers` XML file,
/// deduped by team (first occurrence wins — every driver on the same team shares one physical car).
/// Missing scalar tags default to `1.0` (no tuning applied yet), not an error.
/// Sorted alphabetically by team.
pub fn parse_car_performance_str(xml: &str) -> Vec<CarPerformance> {
    let mut map: BTreeMap<String, CarPerformance> = BTreeMap::new();
    for (livery, block) in primary_driver_blocks(xml) {
        let team = extract_team_name(&livery);
        map.entry(team.clone()).or_insert_with(|| CarPerformance {
            team,
            power_scalar: parse_scalar(&block, "power_scalar"),
            weight_scalar: parse_scalar(&block, "weight_scalar"),
            drag_scalar: parse_scalar(&block, "drag_scalar"),
        });
    }
    map.into_values().collect()
}

/// File-reading wrapper around [`parse_car_performance_str`]. Returns an empty list if the file
/// can't be read.
pub fn parse_car_performance(path: &Path) -> Vec<CarPerformance> {
    match fs::read_to_string(path) {
        Ok(content) => parse_car_performance_str(&content),
        Err(_) => Vec::new(),
    }
}

/// Estimated relative single-lap pace impact of a car's scalars, in "% lap time" units, lower is
/// faster. A rough motorsport rule-of-thumb (~0.1%/1% power, ~0.2%/1% weight, ~0.15%/1% drag) —
/// not an AMS2-measured figure.
fn pace_score(c: &CarPerformance) -> f32 {
    -(c.power_scalar - 1.0) * 10.0 + (c.weight_scalar - 1.0) * 20.0 + (c.drag_scalar - 1.0) * 15.0
}

/// One car's row in a class performance ranking table, with `pace_delta_pct` normalized so the
/// fastest car in the class is `0.0` and every other car shows its estimated % slower.
#[derive(Serialize, Clone, Debug)]
pub struct CarPerformanceRow {
    pub team: String,
    pub power_scalar: f32,
    pub weight_scalar: f32,
    pub drag_scalar: f32,
    pub pace_delta_pct: f32,
}

/// A car class (one `CustomAIDrivers` XML file) with its teams ranked fastest-to-slowest.
#[derive(Serialize, Clone, Debug)]
pub struct ClassPerformance {
    pub class: String,
    /// The real-world F1 season this class is modelled on, if known (see `season_years`).
    pub year: Option<u16>,
    pub cars: Vec<CarPerformanceRow>,
}

/// AMS2 only actually reads a `CustomAIDrivers` XML file if its filename (minus `.xml`) matches
/// a vehicle class name the game itself knows about — otherwise the file is silently ignored.
/// Reiza ships the authoritative list of registered class names as `Colour name="..."` entries
/// in the game's own `GUI/HUD_1_6/HUD_ColoursDefs.xml`.
///
/// `custom_ai_dir` is expected to be `<AMS2 install>/UserData/CustomAIDrivers`; the install root
/// is derived by walking up two directory levels. Returns `None` if that derivation fails or the
/// registry file can't be found/read — callers should treat `None` as "can't verify" (i.e. don't
/// filter), not as "no classes are valid".
pub fn known_class_names(custom_ai_dir: &Path) -> Option<HashSet<String>> {
    let install_root = custom_ai_dir.parent()?.parent()?;
    let hud_colours = install_root.join("GUI").join("HUD_1_6").join("HUD_ColoursDefs.xml");
    let content = fs::read_to_string(hud_colours).ok()?;
    let mut names = HashSet::new();
    let mut rest = content.as_str();
    while let Some(idx) = rest.find("name=\"") {
        rest = &rest[idx + "name=\"".len()..];
        let Some(end) = rest.find('"') else { break };
        names.insert(rest[..end].to_string());
        rest = &rest[end..];
    }
    Some(names)
}

/// Builds a ranked performance table per car class (`*.xml` file) found in `dir`, limited to
/// files whose name matches a class AMS2 actually reads (see [`known_class_names`]) — when that
/// can't be determined, every file is included rather than none. Classes are always returned in
/// chronological order (by the real F1 season they model, via `season_years`); classes with no
/// known season year sort last, alphabetically among themselves.
pub fn class_performance(dir: &Path) -> Vec<ClassPerformance> {
    let known = known_class_names(dir);
    let mut classes: Vec<ClassPerformance> = list_files(dir)
        .into_iter()
        .filter(|file| {
            let class = Path::new(file).file_stem().and_then(|s| s.to_str()).unwrap_or(file);
            known.as_ref().is_none_or(|names| names.contains(class))
        })
        .map(|file| {
            let class = Path::new(&file)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file)
                .to_string();
            let cars = parse_car_performance(&dir.join(&file));
            let best = cars
                .iter()
                .map(pace_score)
                .fold(f32::INFINITY, f32::min);
            let mut rows: Vec<CarPerformanceRow> = cars
                .iter()
                .map(|c| {
                    let score = pace_score(c);
                    CarPerformanceRow {
                        team: c.team.clone(),
                        power_scalar: c.power_scalar,
                        weight_scalar: c.weight_scalar,
                        drag_scalar: c.drag_scalar,
                        pace_delta_pct: if best.is_finite() { score - best } else { 0.0 },
                    }
                })
                .collect();
            rows.sort_by(|a, b| {
                a.pace_delta_pct
                    .partial_cmp(&b.pace_delta_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let year = crate::season_years::season_year(&class);
            ClassPerformance { class, year, cars: rows }
        })
        .collect();
    classes.sort_by(|a, b| {
        a.year
            .unwrap_or(u16::MAX)
            .cmp(&b.year.unwrap_or(u16::MAX))
            .then_with(|| a.class.cmp(&b.class))
    });
    classes
}

#[cfg(test)]
#[path = "tests/custom_ai.rs"]
mod tests;
