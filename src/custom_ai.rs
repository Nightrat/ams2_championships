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
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
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

pub(crate) fn strip_comments(xml: &str) -> String {
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

pub(crate) fn attr_value<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
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
            map.entry(name.to_string())
                .or_insert_with(|| extract_team_name(&livery));
        }
    }
    map
}

/// One car's tuned physical performance within a class, as found in a `CustomAIDrivers` XML file.
#[derive(Clone, Debug, PartialEq)]
pub struct CarPerformance {
    pub team: String,
    /// Everyone listed for this team, in document order. A seat shared by alternate drivers
    /// (1986 Brabham #8 is both De Angelis and Warwick) contributes both names, so this can be
    /// longer than the team's seat count.
    pub drivers: Vec<String>,
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
/// The team's `drivers` accumulate across all its blocks, unlike its scalars.
/// Sorted alphabetically by team.
pub fn parse_car_performance_str(xml: &str) -> Vec<CarPerformance> {
    let mut map: BTreeMap<String, CarPerformance> = BTreeMap::new();
    for (livery, block) in primary_driver_blocks(xml) {
        let team = extract_team_name(&livery);
        let car = map.entry(team.clone()).or_insert_with(|| CarPerformance {
            team,
            drivers: Vec::new(),
            power_scalar: parse_scalar(&block, "power_scalar"),
            weight_scalar: parse_scalar(&block, "weight_scalar"),
            drag_scalar: parse_scalar(&block, "drag_scalar"),
        });
        // A driver listed twice for one team (a second livery of the same car) is still one
        // driver, so names are deduped even though the blocks are not.
        if let Some(name) = element_text(&block, "name") {
            if !car.drivers.iter().any(|d| d == name) {
                car.drivers.push(name.to_string());
            }
        }
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

// ── Writing scalars back to the XML ──────────────────────────────────────────

/// The three tuning scalars of one car, as edited from the Car Performance tab.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scalars {
    pub power: f32,
    pub weight: f32,
    pub drag: f32,
}

/// Accepted range for an edited scalar, straight from Reiza's Custom AI documentation: "Valid
/// values range from 0.900 to 1.100, where 1.000 means no change". All 253 scalars in the shipped
/// files sit inside it.
///
/// See `UserData/CustomAIDrivers/README.txt` for the thread this comes from.
pub const SCALAR_MIN: f32 = 0.9;
pub const SCALAR_MAX: f32 = 1.1;

impl Scalars {
    /// `Err` with a human-readable reason when any value is not a finite number inside
    /// [`SCALAR_MIN`]..=[`SCALAR_MAX`].
    pub fn validate(&self) -> Result<(), String> {
        for (name, v) in [
            ("power_scalar", self.power),
            ("weight_scalar", self.weight),
            ("drag_scalar", self.drag),
        ] {
            if !v.is_finite() || !(SCALAR_MIN..=SCALAR_MAX).contains(&v) {
                return Err(format!(
                    "{name} must be between {SCALAR_MIN:.2} and {SCALAR_MAX:.2}, got {v}"
                ));
            }
        }
        Ok(())
    }
}

/// Two decimals like the shipped files, unless a third is actually needed.
fn fmt_scalar(v: f32) -> String {
    let three = format!("{v:.3}");
    if three.ends_with('0') {
        format!("{v:.2}")
    } else {
        three
    }
}

/// Byte ranges of primary `<driver>` blocks in the **raw** text, paired with their `livery_name`.
///
/// [`primary_driver_blocks`] strips comments first and hands back copies, so its offsets do not
/// index the original file — no good for an edit that must leave every other byte alone. This
/// walks the untouched text instead, stepping over comment regions as it goes.
fn primary_driver_spans(xml: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(next) = xml[pos..].find('<') {
        let at = pos + next;
        let tail = &xml[at..];
        if tail.starts_with("<!--") {
            match tail.find("-->") {
                Some(end) => pos = at + end + 3,
                None => break,
            }
            continue;
        }
        // `<driver` must be the whole element name, not the prefix of a longer one.
        let is_driver = tail
            .strip_prefix("<driver")
            .is_some_and(|r| r.starts_with(['>', '/']) || r.starts_with(char::is_whitespace));
        if !is_driver {
            pos = at + 1;
            continue;
        }
        let Some(tag_end) = tail.find('>') else { break };
        let tag = &tail[..=tag_end];
        let block_len = if tag.trim_end().ends_with("/>") {
            tag_end + 1
        } else if let Some(close) = tail.find("</driver>") {
            close + "</driver>".len()
        } else {
            tail.len()
        };
        let block = &xml[at..at + block_len];
        if let (Some(_), Some(livery)) =
            (element_text(block, "name"), attr_value(tag, "livery_name"))
        {
            out.push((at..at + block_len, livery.to_string()));
        }
        pos = at + block_len;
    }
    out
}

/// Indentation used by the first child element of a `<driver>` block, so an inserted tag lines up
/// with the ones already there. Falls back to eight spaces, the shipped files' style.
fn child_indent(block: &str) -> String {
    block
        .lines()
        .skip(1)
        .find(|l| l.trim_start().starts_with('<'))
        .map(|l| l[..l.len() - l.trim_start().len()].to_string())
        .unwrap_or_else(|| " ".repeat(8))
}

/// Sets `<tag>` inside one `<driver>` block to `value`, replacing the existing text when the tag
/// is present and appending the tag as a last child when it is not. Everything else in the block
/// — spacing, tag order, trailing whitespace — is left exactly as found.
fn set_tag_in_block(block: &str, tag: &str, value: f32) -> String {
    let text = fmt_scalar(value);
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let Some(start) = block.find(open.as_str()) {
        let inner = start + open.len();
        if let Some(rel) = block[inner..].find(close.as_str()) {
            let mut out = String::with_capacity(block.len() + text.len());
            out.push_str(&block[..inner]);
            out.push_str(&text);
            out.push_str(&block[inner + rel..]);
            return out;
        }
    }
    let Some(close_idx) = block.rfind("</driver>") else {
        return block.to_string();
    };
    let element = format!("{open}{text}{close}");
    let head = &block[..close_idx];
    let line_start = head.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let mut out = String::with_capacity(block.len() + element.len() + 16);
    if head[line_start..].trim().is_empty() {
        // `</driver>` sits on its own line: give the new tag a line of its own above it.
        out.push_str(&block[..line_start]);
        out.push_str(&child_indent(block));
        out.push_str(&element);
        out.push('\n');
        out.push_str(&block[line_start..]);
    } else {
        out.push_str(head);
        out.push_str(&element);
        out.push_str(&block[close_idx..]);
    }
    out
}

/// Rewrites the three pace scalars of every primary `<driver>` block whose livery resolves to
/// `team`, returning the new file text. Comments, formatting, and every other tag survive byte
/// for byte.
///
/// One row in the Car Performance table is one *team*, so an edit applies to the whole team:
/// teammates that were tuned apart (the shipped 1980 Brabham gives Lauda 1.00 power and Watson
/// 0.98) end up sharing the edited value. Track-specific override blocks — which repeat
/// `livery_name` but carry no `<name>` — are left alone, so a per-track scalar there still wins
/// at that track.
///
/// `Err` when the file has no driver on that team.
pub fn set_team_scalars_str(xml: &str, team: &str, s: Scalars) -> Result<String, String> {
    let team = team.trim();
    let spans: Vec<(std::ops::Range<usize>, String)> = primary_driver_spans(xml)
        .into_iter()
        .filter(|(_, livery)| extract_team_name(livery).eq_ignore_ascii_case(team))
        .collect();
    if spans.is_empty() {
        return Err(format!("no driver in this file drives for {team}"));
    }
    let mut out = String::with_capacity(xml.len() + spans.len() * 48);
    let mut pos = 0usize;
    for (range, _) in spans {
        out.push_str(&xml[pos..range.start]);
        let mut block = xml[range.clone()].to_string();
        for (tag, v) in [
            ("power_scalar", s.power),
            ("weight_scalar", s.weight),
            ("drag_scalar", s.drag),
        ] {
            block = set_tag_in_block(&block, tag, v);
        }
        out.push_str(&block);
        pos = range.end;
    }
    out.push_str(&xml[pos..]);
    Ok(out)
}

/// Applies [`set_team_scalars_str`] to a file on disk.
///
/// These files live in the user's AMS2 install and were hand-tuned, so the first edit of a given
/// file copies it to `<name>.xml.bak` first. Later edits keep that original backup rather than
/// overwriting it with an already-edited copy.
pub fn set_team_scalars(path: &Path, team: &str, s: Scalars) -> Result<(), String> {
    s.validate()?;
    let xml =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let updated = set_team_scalars_str(&xml, team, s)?;
    write_with_backup(path, &updated)
}

/// Overwrites `path`, keeping a one-time `<name>.xml.bak` of whatever was there first.
fn write_with_backup(path: &Path, contents: &str) -> Result<(), String> {
    let backup = path.with_extension("xml.bak");
    if !backup.exists() {
        fs::copy(path, &backup).map_err(|e| format!("cannot write backup: {e}"))?;
    }
    fs::write(path, contents).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

// ── Per-driver AI attributes ─────────────────────────────────────────────────

/// The AI behaviour tags a driver entry can carry, in the order the Driver Performance table
/// shows them. This is the whole editable set: anything outside it is refused rather than
/// written, so a typo in a request cannot invent a tag AMS2 will not read.
pub const DRIVER_ATTRS: [&str; 15] = [
    "race_skill",
    "qualifying_skill",
    "aggression",
    "defending",
    "stamina",
    "consistency",
    "start_reactions",
    "wet_skill",
    "tyre_management",
    "fuel_management",
    "blue_flag_conceding",
    "weather_tyre_changes",
    "avoidance_of_mistakes",
    "avoidance_of_forced_mistakes",
    "vehicle_reliability",
];

/// Accepted range for a personality attribute: Reiza documents "valid personality values range
/// is between 0 and 1 (inclusive)" for every one of them.
pub const ATTR_MIN: f32 = 0.0;
pub const ATTR_MAX: f32 = 1.0;

/// `vehicle_reliability` is the documented exception — "if you go below 0.0 or above 1.0, that
/// can be done too", with a different formula applied when it is negative. The shipped files use
/// that freely (F-Classic_Gen1 has 166 entries at -0.25, and values run to 1.15 elsewhere).
///
/// Reiza states no bound, so these exist only to catch a slipped decimal point rather than to
/// express a rule.
pub const RELIABILITY_MIN: f32 = -1.0;
pub const RELIABILITY_MAX: f32 = 2.0;

/// The accepted range for one attribute — [`ATTR_MIN`]..=[`ATTR_MAX`] for the fourteen
/// personality tags, the wider reliability range for `vehicle_reliability`.
pub fn attr_range(field: &str) -> (f32, f32) {
    if field == "vehicle_reliability" {
        (RELIABILITY_MIN, RELIABILITY_MAX)
    } else {
        (ATTR_MIN, ATTR_MAX)
    }
}

/// Every editable attribute with the range it accepts, for clients that render one input per
/// attribute and must bound each correctly.
pub fn attr_ranges() -> Vec<(&'static str, f32, f32)> {
    DRIVER_ATTRS
        .iter()
        .map(|f| {
            let (lo, hi) = attr_range(f);
            (*f, lo, hi)
        })
        .collect()
}

/// F-Retro_Gen1 spells `wet_skill` as `wet_skills` throughout. That is almost certainly a typo —
/// AMS2 reads `wet_skill` — but silently renaming a tag across someone's hand-tuned file is not
/// this feature's job, so both spellings are read and an edit updates whichever the block uses.
const WET_SKILL_ALT: &str = "wet_skills";

/// How much each attribute counts toward a driver's composite rating, in percentage points.
///
/// Pace dominates because that is what AMS2 actually scales an AI's speed by; the rest adjusts
/// for how reliably that pace is delivered over a race.
///
/// Four attributes are deliberately absent, because "higher" does not mean "better driver":
/// - `aggression` is a driving *style*. A maximally aggressive AI is not a stronger one.
/// - `blue_flag_conceding` is courtesy toward faster cars, not ability.
/// - `weather_tyre_changes` is a pit-strategy trigger.
/// - `vehicle_reliability` is a property of the car, and track overrides carry it as a negative
///   offset (the shipped 1986 Prost has -0.25 at Jacarepagua), which no rating should absorb.
///
/// The weights total 100, so a driver at 1.00 across the board rates 100. That puts this on the
/// same nominal scale as the player reputation in the Car Performance tab — which already treats
/// `race_skill * 100` as comparable to a reputation (see `driver_rating::required_rating`) — but
/// the two are not measured the same way: this is an aggregate of declared stats, that one is
/// results against expectation.
pub const RATING_WEIGHTS: [(&str, f32); 11] = [
    ("race_skill", 30.0),
    ("qualifying_skill", 15.0),
    ("consistency", 12.0),
    ("avoidance_of_mistakes", 9.0),
    ("avoidance_of_forced_mistakes", 8.0),
    ("tyre_management", 7.0),
    ("wet_skill", 6.0),
    ("start_reactions", 5.0),
    ("defending", 4.0),
    ("stamina", 3.0),
    ("fuel_management", 1.0),
];

/// A 0–100 composite of one entry's declared attributes, weighted by [`RATING_WEIGHTS`].
///
/// Averaged over only the attributes the entry actually declares, then renormalised — otherwise
/// a sparse entry would be punished for silence rather than rated on what it says. The shipped
/// files are uneven about this: `fuel_management` appears in three of seven, and F-Retro_Gen1's
/// per-track substitutes declare only a handful of tags each.
///
/// `None` unless `race_skill` is present. It is the single largest term and the one AMS2 leans on
/// hardest; without it there is no pace to rate, and a number built from the trimmings would look
/// authoritative while meaning very little.
pub fn rate_driver(attrs: &BTreeMap<String, f32>) -> Option<f32> {
    attrs.get("race_skill")?;
    let (sum, weight) = RATING_WEIGHTS
        .iter()
        .filter_map(|(field, w)| attrs.get(*field).map(|v| (v * w, *w)))
        .fold((0.0, 0.0), |(s, t), (sv, w)| (s + sv, t + w));
    if weight <= 0.0 {
        return None;
    }
    Some((100.0 * sum / weight).clamp(0.0, 100.0))
}

/// The tag name to actually read or write in `block` for the attribute `field`.
fn block_tag<'a>(block: &str, field: &'a str) -> &'a str {
    if field == "wet_skill"
        && !block.contains("<wet_skill>")
        && block.contains(&format!("<{WET_SKILL_ALT}>"))
    {
        WET_SKILL_ALT
    } else {
        field
    }
}

/// One `<driver>` entry with a `<name>`, as shown in the Driver Performance table.
///
/// `index` is the entry's position in document order among named blocks — the same order
/// [`primary_driver_blocks`] yields — and is how an edit addresses it. Neither the driver name
/// nor the livery is unique: drivers changed teams mid-season, and F-Retro_Gen1 uses
/// `tracks="..."` blocks as full per-race driver substitutions under a livery it shares with the
/// regular entry.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct DriverAttributes {
    pub index: usize,
    pub driver: String,
    pub team: String,
    /// The raw `livery_name` this entry binds to — the string AMS2 matches against its own
    /// liveries, and the key [`crate::liveries`] checks for existence.
    pub livery: String,
    /// The `tracks="..."` attribute, when this entry only applies at certain circuits.
    pub tracks: Option<String>,
    /// Only the attributes the entry actually declares; a missing one shows as blank rather
    /// than as a default that was never written.
    pub attrs: BTreeMap<String, f32>,
    /// Composite 0–100 strength from those attributes — see [`rate_driver`].
    pub rating: Option<f32>,
    /// `Some(true)` when no installed livery matches `livery`, so AMS2 will never spawn this
    /// driver. `None` means it could not be checked — see [`crate::liveries::phantom_liveries`].
    pub phantom: Option<bool>,
}

/// Every named `<driver>` entry in a `CustomAIDrivers` XML file, in document order.
///
/// `phantom` is left `None`; run [`mark_phantom_entries`] to fill it in.
pub fn parse_driver_attributes_str(xml: &str) -> Vec<DriverAttributes> {
    primary_driver_blocks(xml)
        .into_iter()
        .enumerate()
        .filter_map(|(index, (livery, block))| {
            let driver = element_text(&block, "name")?.to_string();
            let tag_end = block.find('>')?;
            let tracks = attr_value(&block[..=tag_end], "tracks").map(|s| s.to_string());
            let attrs: BTreeMap<String, f32> = DRIVER_ATTRS
                .iter()
                .filter_map(|field| {
                    let text = element_text(&block, block_tag(&block, field))?;
                    Some((field.to_string(), text.parse::<f32>().ok()?))
                })
                .collect();
            Some(DriverAttributes {
                index,
                driver,
                team: extract_team_name(&livery),
                livery,
                tracks,
                rating: rate_driver(&attrs),
                attrs,
                phantom: None,
            })
        })
        .collect()
}

/// File-reading wrapper around [`parse_driver_attributes_str`]. Empty list if unreadable.
pub fn parse_driver_attributes(path: &Path) -> Vec<DriverAttributes> {
    match fs::read_to_string(path) {
        Ok(content) => parse_driver_attributes_str(&content),
        Err(_) => Vec::new(),
    }
}

/// Fills in each entry's `phantom` flag against the liveries actually installed.
///
/// Every entry keeps `None` when the check cannot be made for this roster, so "unknown" is never
/// rendered as "fine" or as "broken" — see [`crate::liveries::phantom_liveries`].
pub fn mark_phantom_entries(entries: &mut [DriverAttributes], installed: Option<&HashSet<String>>) {
    let roster: Vec<String> = entries.iter().map(|e| e.livery.clone()).collect();
    let Some(phantoms) = crate::liveries::phantom_liveries(installed, &roster) else {
        return;
    };
    for e in entries.iter_mut() {
        e.phantom = Some(phantoms.contains(&e.livery));
    }
}

/// Sets one attribute on the named `<driver>` entry at `index`, returning the new file text.
///
/// `expect_driver` must match the entry's `<name>`. The index comes from a table the client
/// loaded earlier, and the file can change underneath it (another edit, a hand edit, a different
/// roster dropped in), so the name is checked before anything is written — otherwise a stale
/// index would quietly retune the wrong driver.
pub fn set_driver_attr_str(
    xml: &str,
    index: usize,
    expect_driver: &str,
    field: &str,
    value: f32,
) -> Result<String, String> {
    if !DRIVER_ATTRS.contains(&field) {
        return Err(format!("{field} is not an editable driver attribute"));
    }
    let (lo, hi) = attr_range(field);
    if !value.is_finite() || !(lo..=hi).contains(&value) {
        return Err(format!(
            "{field} must be between {lo:.2} and {hi:.2}, got {value}"
        ));
    }
    let spans = primary_driver_spans(xml);
    let Some((range, _)) = spans.get(index).cloned() else {
        return Err(format!("this file has no driver entry at position {index}"));
    };
    let block = &xml[range.clone()];
    let found = element_text(block, "name").unwrap_or_default();
    if found != expect_driver.trim() {
        return Err(format!(
            "entry {index} is {found}, not {expect_driver} - reload the tab, the file changed"
        ));
    }
    let updated = set_tag_in_block(block, block_tag(block, field), value);
    let mut out = String::with_capacity(xml.len() + updated.len());
    out.push_str(&xml[..range.start]);
    out.push_str(&updated);
    out.push_str(&xml[range.end..]);
    Ok(out)
}

/// Applies [`set_driver_attr_str`] to a file on disk, keeping the same one-time `.xml.bak` that
/// [`set_team_scalars`] does.
pub fn set_driver_attr(
    path: &Path,
    index: usize,
    expect_driver: &str,
    field: &str,
    value: f32,
) -> Result<(), String> {
    let xml =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let updated = set_driver_attr_str(&xml, index, expect_driver, field, value)?;
    write_with_backup(path, &updated)
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
    /// The team's line-up — see [`CarPerformance::drivers`].
    pub drivers: Vec<String>,
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
    let hud_colours = install_root
        .join("GUI")
        .join("HUD_1_6")
        .join("HUD_ColoursDefs.xml");
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
            let class = Path::new(file)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(file);
            known.as_ref().is_none_or(|names| names.contains(class))
        })
        .map(|file| {
            let class = Path::new(&file)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file)
                .to_string();
            let cars = parse_car_performance(&dir.join(&file));
            let best = cars.iter().map(pace_score).fold(f32::INFINITY, f32::min);
            let mut rows: Vec<CarPerformanceRow> = cars
                .iter()
                .map(|c| {
                    let score = pace_score(c);
                    CarPerformanceRow {
                        team: c.team.clone(),
                        drivers: c.drivers.clone(),
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
            ClassPerformance {
                class,
                year,
                cars: rows,
            }
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

/// Lowest `race_skill` among each team's drivers.
///
/// A team's *weaker* driver is the seat a newcomer would realistically displace, so that is the
/// bar to clear — a Williams needs you to be better than Piquet-or-Mansell's weaker half, an
/// AGS only needs you to beat Capelli. Teams whose drivers declare no `race_skill` are absent
/// from the map rather than defaulted, so callers can tell "no bar known" from "a low bar".
pub fn parse_team_skills_str(xml: &str) -> HashMap<String, f32> {
    let mut out: HashMap<String, f32> = HashMap::new();
    for (livery, block) in primary_driver_blocks(xml) {
        let Some(skill) = element_text(&block, "race_skill").and_then(|s| s.parse::<f32>().ok())
        else {
            continue;
        };
        let team = extract_team_name(&livery);
        out.entry(team)
            .and_modify(|v| *v = v.min(skill))
            .or_insert(skill);
    }
    out
}

/// File-reading wrapper around [`parse_team_skills_str`]. Empty map if the file can't be read.
pub fn parse_team_skills(path: &Path) -> HashMap<String, f32> {
    match fs::read_to_string(path) {
        Ok(content) => parse_team_skills_str(&content),
        Err(_) => HashMap::new(),
    }
}

// ── Grid seats and player-team inference ─────────────────────────────────────

/// One `<driver>` entry resolved to the grid slot it occupies.
///
/// A *seat* is not the same as a livery entry: several seats carry two alternate drivers
/// (in the 1986 F-Classic_Gen1 roster, Brabham #8 is both De Angelis and Warwick), and AMS2
/// spawns only one of them per grid. Counting livery entries therefore overcounts the field —
/// that file has 32 entries but only 27 seats. Callers that need the field size must dedupe
/// on `seat`; this list keeps one entry per driver so a name can be looked up.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SeatEntry {
    /// Team plus car number, e.g. "Brabham #7".
    pub seat: String,
    /// Team alone, e.g. "Brabham".
    pub team: String,
    /// The `<name>` AMS2 shows for this entry.
    pub driver: String,
    /// The raw `livery_name`, so [`without_phantom_seats`] can tell whether AMS2 owns this car.
    pub livery: String,
}

/// A team plus car number, without the driver — the unit the player declares.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Seat {
    pub seat: String,
    pub team: String,
}

/// One car on a recorded grid, as telemetry saw it.
pub struct GridEntry<'a> {
    pub name: &'a str,
    pub car_name: &'a str,
    pub is_player: bool,
}

/// The car number in a `livery_name`: the digits following the first `" #"`.
fn extract_car_number(livery: &str) -> Option<String> {
    let idx = livery.find(" #")? + 2;
    let num: String = livery[idx..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if num.is_empty() {
        None
    } else {
        Some(num)
    }
}

/// Match key for a driver name: first initial + surname, lowercased, non-alphanumerics dropped.
///
/// AMS2's telemetry spelling does not always match the file's `<name>` — the 1986
/// F-Classic_Gen1 roster reports "Allen Berg" where the file says "Allan Berg", which affects
/// 15 of 47 recorded sessions. Exact string equality silently loses such a driver, and a lost
/// driver is indistinguishable from an empty seat, so it would invent a phantom candidate.
/// Parenthesised markers (AMS2's stock `"(AI)"` suffix) are ignored.
pub fn name_key(name: &str) -> String {
    let words: Vec<&str> = name
        .split_whitespace()
        .filter(|w| !w.starts_with('('))
        .collect();
    let initial: String = words
        .first()
        .and_then(|w| w.chars().next())
        .map(|c| c.to_lowercase().to_string())
        .unwrap_or_default();
    let surname: String = words
        .last()
        .map(|w| {
            w.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect()
        })
        .unwrap_or_default();
    format!("{initial}|{surname}")
}

/// Grid seats defined by a Custom AI Driver file, one entry per named driver.
pub fn parse_seats_str(xml: &str) -> Vec<SeatEntry> {
    primary_driver_blocks(xml)
        .into_iter()
        .filter_map(|(livery, block)| {
            let driver = element_text(&block, "name")?.to_string();
            let team = extract_team_name(&livery);
            let seat = match extract_car_number(&livery) {
                Some(num) => format!("{team} #{num}"),
                None => team.clone(),
            };
            Some(SeatEntry {
                seat,
                team,
                driver,
                livery,
            })
        })
        .collect()
}

/// File-reading wrapper around [`parse_seats_str`]. Empty list if the file can't be read.
pub fn parse_seats(path: &Path) -> Vec<SeatEntry> {
    match fs::read_to_string(path) {
        Ok(content) => parse_seats_str(&content),
        Err(_) => Vec::new(),
    }
}

/// Drops seats whose livery AMS2 does not have, so seat accounting only counts seats that can
/// actually be occupied.
///
/// A phantom seat is never filled by an AI, so [`infer_player_seat`] would see it as free for
/// every session forever — either offering it as the player's seat or, once the car-model filter
/// removes it, leaving no free seat at all and failing an otherwise legitimate session. Removing
/// them first is what makes the elimination sound. When the check cannot be made the list is
/// returned untouched, which is the existing behaviour.
pub fn without_phantom_seats(
    entries: Vec<SeatEntry>,
    installed: Option<&HashSet<String>>,
) -> Vec<SeatEntry> {
    let roster: Vec<String> = entries.iter().map(|e| e.livery.clone()).collect();
    match crate::liveries::phantom_liveries(installed, &roster) {
        Some(phantoms) => entries
            .into_iter()
            .filter(|e| !phantoms.contains(&e.livery))
            .collect(),
        None => entries,
    }
}

/// Which seat the human player occupied in a recorded session.
///
/// AMS2 exposes no livery field for any car, so the player's team is never read directly. It is
/// inferred instead: the player occupies a seat, so no AI can spawn in it, and every roster
/// driver *present* on the grid rules their own seat out. The remaining empty seats are then
/// filtered to those whose car model matches the one the player actually drove (`mCarName`),
/// since a livery belongs to exactly one vehicle model within a class.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayerSeat {
    /// Too little of the grid appears in the Custom AI file for seat accounting to mean
    /// anything — typically a session run on stock AMS2 AI. Nothing can be enforced.
    RosterNotDetected { matched: usize, grid: usize },
    /// Exactly one seat was unoccupied, so it must be the player's.
    Derived(Seat),
    /// Several seats were unoccupied and are consistent with the player's car model.
    Candidates(Vec<Seat>),
    /// Every roster seat was taken by an AI, so the player was not in a roster car.
    NoEmptySeat,
}

/// Infers the player's seat from a recorded grid. See [`PlayerSeat`].
///
/// Car models are compared as raw `mCarName` strings. AMS2's aero variants ("… - High
/// Downforce") are chosen per event and apply to the whole grid — no recorded session has ever
/// mixed them — so the player and the AI always carry the same suffix and no stripping is needed.
///
/// When some AI can't be matched to the roster the empty-seat set is a *superset* of the truth.
/// That errs toward accepting a session rather than rejecting a legitimate one, which is the
/// safe direction for enforcement.
pub fn infer_player_seat(entries: &[SeatEntry], grid: &[GridEntry]) -> PlayerSeat {
    let mut by_key: HashMap<String, Vec<&SeatEntry>> = HashMap::new();
    for e in entries {
        by_key.entry(name_key(&e.driver)).or_default().push(e);
    }

    let (player, ai): (Vec<&GridEntry>, Vec<&GridEntry>) = grid.iter().partition(|g| g.is_player);

    let matched: Vec<(&GridEntry, &Vec<&SeatEntry>)> = ai
        .iter()
        .filter_map(|g| by_key.get(&name_key(g.name)).map(|e| (*g, e)))
        .collect();
    if matched.len() * 2 < ai.len() {
        return PlayerSeat::RosterNotDetected {
            matched: matched.len(),
            grid: grid.len(),
        };
    }

    // team -> car model, learned from AI whose name maps to a single team. Drivers listed under
    // two teams (Danner drove both Osella and Arrows in 1986) are skipped here and resolved below.
    let mut team_model: HashMap<&str, &str> = HashMap::new();
    for (g, es) in &matched {
        let mut teams = es.iter().map(|e| e.team.as_str());
        let first = teams.next();
        if let Some(team) = first {
            if teams.all(|t| t == team) {
                team_model.insert(team, g.car_name);
            }
        }
    }

    let mut occupied: HashSet<&str> = HashSet::new();
    for (g, es) in &matched {
        if es.len() == 1 {
            occupied.insert(es[0].seat.as_str());
            continue;
        }
        // Two seats share this driver — the car model says which one they actually raced.
        let fits: Vec<&&SeatEntry> = es
            .iter()
            .filter(|e| team_model.get(e.team.as_str()) == Some(&g.car_name))
            .collect();
        if let [only] = fits.as_slice() {
            occupied.insert(only.seat.as_str());
        }
    }

    // Sessions recorded before `is_player` existed have the flag false on every row. Fall back
    // to the one car on the grid that isn't in the roster at all — that is the human.
    let player_car = player.first().map(|p| p.car_name).or_else(|| {
        let strangers: Vec<&GridEntry> = grid
            .iter()
            .filter(|g| !by_key.contains_key(&name_key(g.name)))
            .collect();
        match strangers.as_slice() {
            [only] => Some(only.car_name),
            _ => None,
        }
    });
    let mut seen: HashSet<&str> = HashSet::new();
    let empty: Vec<Seat> = entries
        .iter()
        .filter(|e| !occupied.contains(e.seat.as_str()) && seen.insert(e.seat.as_str()))
        // Keep a seat whose team model is unknown: never observed means never ruled out.
        .filter(|e| match (player_car, team_model.get(e.team.as_str())) {
            (Some(car), Some(model)) => *model == car,
            _ => true,
        })
        .map(|e| Seat {
            seat: e.seat.clone(),
            team: e.team.clone(),
        })
        .collect();

    match empty.len() {
        0 => PlayerSeat::NoEmptySeat,
        1 => PlayerSeat::Derived(empty.into_iter().next().unwrap()),
        _ => PlayerSeat::Candidates(empty),
    }
}

/// Result of checking a recorded session against a championship's declared player team.
#[derive(Clone, Debug, PartialEq)]
pub enum TeamCheck {
    /// Enforcement could not be applied; the session is accepted. Carries the reason.
    Skipped(String),
    /// The session is consistent with the declared team.
    Passed(String),
    /// The session contradicts the declared team and must be rejected. Carries the reason.
    Failed(String),
}

/// True when `declared` names the same team or seat as `seat`, ignoring case and surrounding
/// space. Both "Brabham" and "Brabham #7" are accepted for the Brabham #7 seat.
fn declares(seat: &Seat, declared: &str) -> bool {
    let d = declared.trim();
    seat.team.eq_ignore_ascii_case(d) || seat.seat.eq_ignore_ascii_case(d)
}

fn seat_list(seats: &[Seat]) -> String {
    seats
        .iter()
        .map(|s| s.seat.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Checks a recorded grid against the team the player declared for a championship.
///
/// Only ever rejects on positive evidence — a contradiction between the declared team and what
/// the grid shows. Anything it cannot determine is [`TeamCheck::Skipped`] and accepted.
pub fn check_player_team(entries: &[SeatEntry], grid: &[GridEntry], declared: &str) -> TeamCheck {
    let declared = declared.trim();
    if declared.is_empty() {
        return TeamCheck::Skipped("no player team declared".into());
    }
    if entries.is_empty() {
        return TeamCheck::Skipped("the Custom AI file lists no drivers".into());
    }
    match infer_player_seat(entries, grid) {
        PlayerSeat::RosterNotDetected { matched, grid } => TeamCheck::Skipped(format!(
            "only {matched} of {grid} drivers are in the Custom AI file - this session did not use it"
        )),
        PlayerSeat::NoEmptySeat => TeamCheck::Failed(format!(
            "every seat in the Custom AI file was taken by an AI, so you cannot have been driving for {declared}"
        )),
        PlayerSeat::Derived(seat) => {
            if declares(&seat, declared) {
                TeamCheck::Passed(format!("only {} was free - that is your seat", seat.seat))
            } else {
                TeamCheck::Failed(format!(
                    "you declared {declared} but the only free seat was {}",
                    seat.seat
                ))
            }
        }
        PlayerSeat::Candidates(seats) => {
            if seats.iter().any(|s| declares(s, declared)) {
                TeamCheck::Passed(format!("consistent with the free seats: {}", seat_list(&seats)))
            } else {
                TeamCheck::Failed(format!(
                    "you declared {declared} but the car you drove and the drivers on the grid leave only: {}",
                    seat_list(&seats)
                ))
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/custom_ai.rs"]
mod tests;
