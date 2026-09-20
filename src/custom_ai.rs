use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

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

/// One `<driver>` block: the livery it binds to, its text, and the `tracks` attribute when it
/// carries one.
///
/// **`tracks` is what makes a block an override, not the absence of a `<name>`.** AMS2 lets a
/// per-track block name a *substitute driver* — F-Vintage_Gen2 puts Tino Brambilla in Amon's
/// Ferrari for Monza 1971 — and reading the old way counted that stand-in as a 27th full-time
/// driver of a 26-car roster, and handed Ferrari his skill as its incumbent bar.
struct DriverBlock {
    livery: String,
    block: String,
    /// `Some` for a per-track override, whether or not it renames the driver.
    tracks: Option<String>,
}

/// Every `<driver>` block that binds a livery, in document order — overrides included.
fn all_driver_blocks(xml: &str) -> Vec<DriverBlock> {
    let xml = strip_comments(xml);
    let mut out = Vec::new();
    let mut rest = xml.as_str();
    while let Some(tag_start) = rest.find("<driver") {
        rest = &rest[tag_start..];
        let Some(tag_end) = rest.find('>') else { break };
        let tag = &rest[..=tag_end];
        let self_closing = tag.trim_end().ends_with("/>");
        let livery = attr_value(tag, "livery_name").map(|s| s.to_string());
        let tracks = attr_value(tag, "tracks")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let block_end = if self_closing {
            tag_end + 1
        } else if let Some(close) = rest.find("</driver>") {
            close + "</driver>".len()
        } else {
            rest.len()
        };
        let block = &rest[..block_end];

        if let Some(livery) = livery {
            out.push(DriverBlock {
                livery,
                block: block.to_string(),
                tracks,
            });
        }

        rest = &rest[block_end..];
    }
    out
}

/// Blocks that name a driver: the regular entries plus the overrides that field a stand-in.
///
/// This is the list for **matching a name to a seat** — a stand-in on track is in that team's
/// car, and the seat they are standing in for is occupied while they are. It is *not* the list
/// for counting cars (seats repeat, so count with [`car_count`]) or for asking what a team
/// demands of a newcomer (a one-race substitute is not the incumbent — see
/// [`regular_driver_blocks`]).
fn named_driver_blocks(xml: &str) -> Vec<(String, String)> {
    all_driver_blocks(xml)
        .into_iter()
        .filter(|b| element_text(&b.block, "name").is_some())
        .map(|b| (b.livery, b.block))
        .collect()
}

/// Blocks for the driver who *holds* each seat: named, and not restricted to certain tracks.
///
/// What a team is, for a season: its cars, its scalars and the drivers whose skill a newcomer
/// has to beat. A per-track substitute is none of those.
fn regular_driver_blocks(xml: &str) -> Vec<(String, String)> {
    all_driver_blocks(xml)
        .into_iter()
        .filter(|b| b.tracks.is_none() && element_text(&b.block, "name").is_some())
        .map(|b| (b.livery, b.block))
        .collect()
}

/// Every driver name in the file mapped to their team, **stand-ins included**: a substitute on
/// track is driving that team's car, so the live grid should say so.
pub fn parse_driver_teams_str(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (livery, block) in named_driver_blocks(xml) {
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
    // Regular entries only: a driver who appears for one weekend is not one of the team's.
    for (livery, block) in regular_driver_blocks(xml) {
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
/// [`all_driver_blocks`] strips comments first and hands back copies, so its offsets do not
/// index the original file — no good for an edit that must leave every other byte alone. This
/// walks the untouched text instead, stepping over comment regions as it goes.
///
/// It must stay **positionally identical** to [`all_driver_blocks`]: the Driver Performance tab
/// sends back the index it was given, and the writer resolves it here. Filter, never re-walk.
fn driver_spans(xml: &str) -> Vec<DriverSpan> {
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
        if let Some(livery) = attr_value(tag, "livery_name") {
            out.push(DriverSpan {
                range: at..at + block_len,
                livery: livery.to_string(),
                tracks: attr_value(tag, "tracks")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                named: element_text(block, "name").is_some(),
            });
        }
        pos = at + block_len;
    }
    out
}

/// One `<driver>` block located in the raw file text. See [`driver_spans`].
struct DriverSpan {
    range: std::ops::Range<usize>,
    livery: String,
    /// `Some` for a per-track override — see [`DriverBlock::tracks`].
    tracks: Option<String>,
    /// Whether the block declares its own `<name>`.
    named: bool,
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
/// 0.98) end up sharing the edited value. Track-specific override blocks are left alone, so a
/// per-track scalar there still wins at that track — that now holds for an override that names
/// a stand-in too, which used to be written like a regular entry because it had a `<name>`.
///
/// `Err` when the file has no driver on that team.
pub fn set_team_scalars_str(xml: &str, team: &str, s: Scalars) -> Result<String, String> {
    let team = team.trim();
    let spans: Vec<(std::ops::Range<usize>, String)> = driver_spans(xml)
        .into_iter()
        .filter(|d| d.tracks.is_none() && d.named)
        .filter(|d| extract_team_name(&d.livery).eq_ignore_ascii_case(team))
        .map(|d| (d.range, d.livery))
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
/// file records a baseline first. See [`ensure_baseline`].
pub fn set_team_scalars(path: &Path, team: &str, s: Scalars) -> Result<(), String> {
    s.validate()?;
    let xml =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let updated = set_team_scalars_str(&xml, team, s)?;
    write_with_backup(path, &updated)
}

/// Overwrites `path`, recording a baseline first if it has none.
fn write_with_backup(path: &Path, contents: &str) -> Result<(), String> {
    ensure_baseline(path)?;
    fs::write(path, contents).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

// ── Baselines ────────────────────────────────────────────────────────────────

/// Where a class's baseline lives: `<name>.xml.bak`, beside the file it describes.
///
/// AMS2 ignores it — the game loads a `CustomAIDrivers` file only when its stem is a class in its
/// own registry, and this one's stem is `<name>.xml` — so it is safe to keep in the install
/// folder, next to the roster it belongs to rather than in the app's own data.
pub fn baseline_path(path: &Path) -> PathBuf {
    path.with_extension("xml.bak")
}

/// True when `path` has a baseline to reset to.
pub fn has_baseline(path: &Path) -> bool {
    baseline_path(path).is_file()
}

/// Record `path` as its own baseline, unless it already has one.
///
/// **One-time by design.** A file that has been backed up keeps the copy it has, which is the
/// *original* — re-taking it after an edit would quietly redefine what "reset" means, and the
/// original would be gone. [`set_baseline`] is the deliberate way to do that.
///
/// Called before every write, and when a season picks a roster: a class a career races must have
/// a baseline whether or not the app has ever edited it.
pub fn ensure_baseline(path: &Path) -> Result<(), String> {
    let backup = baseline_path(path);
    if backup.exists() {
        return Ok(());
    }
    if !path.is_file() {
        return Err(format!("{} does not exist", path.display()));
    }
    fs::copy(path, &backup)
        .map(|_| ())
        .map_err(|e| format!("cannot write {}: {e}", backup.display()))
}

/// Restore `path` from its baseline, discarding every change made since.
pub fn reset_from_baseline(path: &Path) -> Result<(), String> {
    let backup = baseline_path(path);
    if !backup.is_file() {
        return Err(format!(
            "{} has no baseline to reset to",
            path.display()
        ));
    }
    fs::copy(&backup, path)
        .map(|_| ())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Replace the baseline with the file as it stands now.
///
/// For a roster tuned by hand *after* the app first recorded one: without this the baseline holds
/// the older version forever, and a reset would throw that tuning away. It overwrites, which is
/// the whole point and the opposite of [`ensure_baseline`] — so the caller is responsible for
/// warning first. What it discards is the ability to get back to the earlier file, and it also
/// moves every figure derived from the baseline.
pub fn set_baseline(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("{} does not exist", path.display()));
    }
    let backup = baseline_path(path);
    fs::copy(path, &backup)
        .map(|_| ())
        .map_err(|e| format!("cannot write {}: {e}", backup.display()))
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

/// Every `<driver>` entry in a `CustomAIDrivers` XML file, in document order — the regular
/// entries and the per-track overrides between them.
///
/// Overrides were previously visible only when they renamed the driver, because the walk kept
/// blocks by the presence of a `<name>`. That showed a substitute as a full-time driver while
/// hiding every override that only retunes the regular one, which is the more common kind. An
/// override with no `<name>` inherits the name of the entry it modifies — that *is* who drives
/// the car at those tracks — so the column stays meaningful and the row can still be edited.
///
/// `phantom` is left `None`; run [`mark_phantom_entries`] to fill it in.
pub fn parse_driver_attributes_str(xml: &str) -> Vec<DriverAttributes> {
    let blocks = all_driver_blocks(xml);
    let names = inherited_names(&blocks);
    blocks
        .iter()
        .enumerate()
        .filter_map(|(index, b)| {
            let driver = names.get(index)?.clone()?;
            let attrs: BTreeMap<String, f32> = DRIVER_ATTRS
                .iter()
                .filter_map(|field| {
                    let text = element_text(&b.block, block_tag(&b.block, field))?;
                    Some((field.to_string(), text.parse::<f32>().ok()?))
                })
                .collect();
            Some(DriverAttributes {
                index,
                driver,
                team: extract_team_name(&b.livery),
                livery: b.livery.clone(),
                tracks: b.tracks.clone(),
                rating: rate_driver(&attrs),
                attrs,
                phantom: None,
            })
        })
        .collect()
}

/// The driver each block describes: its own `<name>`, or the name of the regular entry for the
/// same livery when it has none.
///
/// Positional, so it lines up with [`all_driver_blocks`] — and with the spans the writer edits,
/// which is what lets an index from the table address the same block later. `None` for a block
/// whose livery has no named entry at all, which is a malformed file rather than an override.
fn inherited_names(blocks: &[DriverBlock]) -> Vec<Option<String>> {
    let mut by_livery: HashMap<&str, &str> = HashMap::new();
    for b in blocks {
        if b.tracks.is_none() {
            if let Some(name) = element_text(&b.block, "name") {
                by_livery.entry(b.livery.as_str()).or_insert(name);
            }
        }
    }
    blocks
        .iter()
        .map(|b| {
            element_text(&b.block, "name")
                .map(str::to_string)
                .or_else(|| by_livery.get(b.livery.as_str()).map(|s| s.to_string()))
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
    let spans = driver_spans(xml);
    let Some(span) = spans.get(index) else {
        return Err(format!("this file has no driver entry at position {index}"));
    };
    let range = span.range.clone();
    let block = &xml[range.clone()];
    // An override with no `<name>` is shown under the name of the entry it modifies, so that is
    // what comes back — [`driver_name_at`] resolves it exactly as the reader did, and the
    // removal guard uses the same one.
    let found = driver_name_at(xml, &spans, index).unwrap_or_default();
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

/// Deletes the per-track `<driver>` entry at `index`, returning the new file text.
///
/// **Only a per-track block may go.** A regular entry is a car on the grid: removing one would
/// shrink the field, move every expected finishing position derived from it, and leave a livery
/// with nobody in it — a thing the user would have to hand-edit back. An override carries no
/// car of its own, so deleting it only takes the tuning it applied at those circuits, which is
/// exactly what someone wants gone before a season: an entry that quietly hands one driver a
/// different skill at one round is a result that cannot be compared with the others.
///
/// Guarded like [`set_driver_attr_str`], and for the same reason — the index comes from a table
/// the client loaded earlier, and deleting the wrong row is worse than retuning one.
///
/// The block is taken with the whitespace that led up to it, so the file does not accumulate
/// blank lines where entries used to be.
pub fn remove_driver_entry_str(
    xml: &str,
    index: usize,
    expect_driver: &str,
) -> Result<String, String> {
    let spans = driver_spans(xml);
    let Some(span) = spans.get(index) else {
        return Err(format!("this file has no driver entry at position {index}"));
    };
    if span.tracks.is_none() {
        return Err(
            "only a per-track entry can be removed — a regular one is a car on the grid".into(),
        );
    }
    let found = driver_name_at(xml, &spans, index).unwrap_or_default();
    if found != expect_driver.trim() {
        return Err(format!(
            "entry {index} is {found}, not {expect_driver} - reload the tab, the file changed"
        ));
    }
    // Back up over the indentation on the block's own line, and the newline before it.
    let mut start = span.range.start;
    while start > 0 && matches!(xml.as_bytes()[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    if start > 0 && xml.as_bytes()[start - 1] == b'\n' {
        start -= 1;
        if start > 0 && xml.as_bytes()[start - 1] == b'\r' {
            start -= 1;
        }
    }
    let mut out = String::with_capacity(xml.len());
    out.push_str(&xml[..start]);
    out.push_str(&xml[span.range.end..]);
    Ok(out)
}

/// Deletes **every** per-track entry in a file, returning the new text and how many went.
///
/// A roster can carry a lot of them — F-Retro_Gen1 has 17 — and removing them one at a time
/// from the table means every remaining index shifts under the client after each one. Doing the
/// whole sweep in one pass here is both safer and what someone clearing a roster before a
/// season actually wants.
///
/// Regular entries are untouched, so the grid is exactly the grid it was.
pub fn remove_track_entries_str(xml: &str) -> (String, usize) {
    let spans = driver_spans(xml);
    let mut doomed: Vec<std::ops::Range<usize>> = spans
        .iter()
        .filter(|d| d.tracks.is_some())
        .map(|d| d.range.clone())
        .collect();
    if doomed.is_empty() {
        return (xml.to_string(), 0);
    }
    let removed = doomed.len();
    // Back to front, so each cut leaves the earlier offsets valid.
    doomed.sort_by_key(|r| std::cmp::Reverse(r.start));
    let mut out = xml.to_string();
    for range in doomed {
        let mut start = range.start;
        while start > 0 && matches!(out.as_bytes()[start - 1], b' ' | b'\t') {
            start -= 1;
        }
        if start > 0 && out.as_bytes()[start - 1] == b'\n' {
            start -= 1;
            if start > 0 && out.as_bytes()[start - 1] == b'\r' {
                start -= 1;
            }
        }
        out.replace_range(start..range.end, "");
    }
    (out, removed)
}

/// Applies [`remove_track_entries_str`] to a file on disk. Writes nothing when there was
/// nothing to remove, so a second click does not take a pointless backup or touch the mtime.
pub fn remove_track_entries(path: &Path) -> Result<usize, String> {
    let xml =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let (updated, removed) = remove_track_entries_str(&xml);
    if removed > 0 {
        write_with_backup(path, &updated)?;
    }
    Ok(removed)
}

/// The driver a span is listed under: its own `<name>`, or the one it inherits from the regular
/// entry for the same livery. The reader shows the inherited name, so the guards compare
/// against the same thing.
fn driver_name_at(xml: &str, spans: &[DriverSpan], index: usize) -> Option<String> {
    let span = spans.get(index)?;
    let block = &xml[span.range.clone()];
    if let Some(name) = element_text(block, "name") {
        return Some(name.to_string());
    }
    spans
        .iter()
        .find(|d| d.tracks.is_none() && d.named && d.livery == span.livery)
        .and_then(|d| element_text(&xml[d.range.clone()], "name"))
        .map(str::to_string)
}

/// Applies [`remove_driver_entry_str`] to a file on disk, keeping the same one-time `.xml.bak`
/// every other writer takes — so **Reset to baseline** brings a removed entry back.
pub fn remove_driver_entry(path: &Path, index: usize, expect_driver: &str) -> Result<(), String> {
    let xml =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let updated = remove_driver_entry_str(&xml, index, expect_driver)?;
    write_with_backup(path, &updated)
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

/// The class name a `CustomAIDrivers` file claims by its filename — the stem AMS2 matches against
/// its registry. Falls back to the whole filename when it has no stem.
pub fn class_of_file(file: &str) -> &str {
    Path::new(file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file)
}

/// [`list_files`], limited to the files AMS2 actually reads.
///
/// The game loads a `CustomAIDrivers` file only when its name minus `.xml` is a class in its own
/// registry. Anything else is silently ignored — a per-track variant kept beside the real file
/// (`F-Vintage_Gen2_03Nordschleiffe.xml`), or a class written the way the UI spells it rather than
/// the way the registry does (`Formula Renault.xml`). Such a file cannot affect a session, so
/// assigning a championship to it would promise AI behaviour that never happens.
///
/// An unreadable registry returns every file rather than none, the same can't-verify rule
/// [`class_performance`] follows — see [`known_class_names`].
pub fn list_files_for_known_classes(dir: &Path) -> Vec<String> {
    let known = known_class_names(dir);
    list_files(dir)
        .into_iter()
        .filter(|file| {
            known
                .as_ref()
                .is_none_or(|names| names.contains(class_of_file(file)))
        })
        .collect()
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
            known
                .as_ref()
                .is_none_or(|names| names.contains(class_of_file(file)))
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
    // **Regular drivers only.** The bar is the seat a newcomer would displace, and a one-race
    // substitute holds no seat — F-Vintage_Gen2's Monza stand-in is `race_skill` 0.68 against
    // Rodríguez's 0.73, so counting him quietly made the Ferrari seat five points cheaper.
    for (livery, block) in regular_driver_blocks(xml) {
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
///
/// A per-track stand-in gets an entry too, sharing the seat of the driver they replace: at that
/// track they are the car, and leaving them out would make the seat look empty and the car look
/// like an AI the roster does not know. Entries therefore repeat per seat — count cars with
/// [`car_count`], never with `len()`.
pub fn parse_seats_str(xml: &str) -> Vec<SeatEntry> {
    named_driver_blocks(xml)
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

/// How a grid — recorded or live — lines up with the roster it was meant to be raced on.
///
/// The rating measures a finish against where the car ranks across the **whole** roster, while
/// the finish itself is a position within the field that actually raced. Those two agree only
/// when AMS2 fielded the roster, so a grid that did not is worth saying out loud rather than
/// being scored quietly on the wrong scale. Nothing here rejects anything: it reports.
///
/// Counts rather than a verdict, because the problems are not exclusive — a grid can be short
/// *and* padded with stock AI at once, and the caller decides which to lead with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct GridFit {
    /// Cars on track, the player included. A full grid has one per seat.
    pub cars: usize,
    /// Seats the roster can actually field — phantom liveries already removed, since a car AMS2
    /// cannot spawn was never going to be on the grid.
    pub seats: usize,
    /// AI on track, i.e. everyone but the player.
    pub ai: usize,
    /// AI whose name appears in the roster.
    pub matched: usize,
}

impl GridFit {
    /// Measures a grid against a roster. `seats` must already be phantom-filtered.
    pub fn measure(seats: &[SeatEntry], grid: &[GridEntry]) -> Self {
        let by_key: HashSet<String> = seats.iter().map(|e| name_key(&e.driver)).collect();
        let ai = grid.iter().filter(|g| !g.is_player).count();
        let matched = grid
            .iter()
            .filter(|g| !g.is_player && by_key.contains(&name_key(g.name)))
            .count();
        Self {
            cars: grid.len(),
            seats: car_count(seats),
            ai,
            matched,
        }
    }

    /// The roster was not what AMS2 ran. The same majority test [`infer_player_seat`] gives up
    /// on, deliberately: a session either identifies its roster well enough to reason about or
    /// it does not, and two thresholds could disagree about the same session.
    pub fn not_roster(&self) -> bool {
        self.seats > 0 && self.matched * 2 < self.ai
    }

    /// AI on track the roster does not name — stock cars filling out an over-long grid.
    pub fn stock_fill(&self) -> usize {
        self.ai - self.matched
    }

    /// Seats the roster could have filled and did not.
    pub fn short_by(&self) -> usize {
        self.seats.saturating_sub(self.cars)
    }

    /// Nothing to report: every seat raced, and nothing else did.
    pub fn is_full(&self) -> bool {
        self.seats > 0 && self.stock_fill() == 0 && self.short_by() == 0 && self.cars <= self.seats
    }

    /// One sentence for the user, or `None` when the grid is what it should have been.
    ///
    /// The wording lives here rather than in the browser so the live banner, the Manage tab and
    /// the Career tab cannot drift apart — the discipline `offerWhy()` already follows. A
    /// session that did not use the roster at all is said first, because the other two are then
    /// beside the point.
    pub fn note(&self) -> Option<String> {
        if self.seats == 0 {
            return None;
        }
        if self.not_roster() {
            return Some(format!(
                "Not raced on this roster — only {} of {} AI are in it, so it cannot count towards your rating.",
                self.matched, self.ai
            ));
        }
        let (short, stock) = (self.short_by(), self.stock_fill());
        if short > 0 && stock > 0 {
            return Some(format!(
                "Short grid: {} cars against the roster's {}, {} of them stock AI. Your finish is judged against the full {}-car roster, so it is measured on the wrong scale.",
                self.cars, self.seats, stock, self.seats
            ));
        }
        if short > 0 {
            return Some(format!(
                "Short grid: {} cars against the roster's {}. Your finish is judged against where your car ranks in the full {}-car roster, so a smaller field flatters it.",
                self.cars, self.seats, self.seats
            ));
        }
        if stock > 0 {
            return Some(format!(
                "{} car{} on this grid {} not in the roster: the opponent count is above the {} it can field, so AMS2 filled the rest with stock AI.",
                stock,
                if stock == 1 { "" } else { "s" },
                if stock == 1 { "is" } else { "are" },
                self.seats
            ));
        }
        None
    }

    /// Confirmation that this grid *is* the roster, for a surface that says so out loud.
    ///
    /// Only the live tab uses it, and only because silence there is ambiguous: with nothing on
    /// screen the driver cannot tell a grid that was checked and passed from one that was
    /// never checked, and the whole point of the banner is to be trusted before the lights go
    /// out. The Manage and Career tabs stay quiet on a clean session — a flag per race that
    /// says "fine" is noise in a list of forty.
    ///
    /// `None` whenever [`Self::note`] has something to say, so a caller cannot show both.
    pub fn confirmation(&self) -> Option<String> {
        self.is_full().then(|| {
            format!(
                "Full grid: all {} cars of the roster are out. This session will be judged on it.",
                self.cars
            )
        })
    }
}

/// Cars a roster can put on the grid: distinct `seat` values, i.e. team plus car number.
///
/// **This must agree with `driver_rating::expected_positions`**, which ranks the same distinct
/// seats to decide where a car is expected to finish. A count derived any other way would
/// contradict the thing it is used to judge.
///
/// It is also why nothing should count `parse_seats` entries with `len()`: the list holds one
/// entry per *driver*, so a car with two skins across a season, or a per-track stand-in, adds
/// entries without adding a car. F-Vintage_Gen2 is 27 entries and 26 cars.
pub fn car_count(seats: &[SeatEntry]) -> usize {
    seats
        .iter()
        .map(|e| e.seat.as_str())
        .collect::<HashSet<&str>>()
        .len()
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
