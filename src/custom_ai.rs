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
        out.entry(team).and_modify(|v| *v = v.min(skill)).or_insert(skill);
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
    let num: String = livery[idx..].chars().take_while(|c| c.is_ascii_digit()).collect();
    if num.is_empty() { None } else { Some(num) }
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
        .map(|w| w.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect())
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
            Some(SeatEntry { seat, team, driver })
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

    let (player, ai): (Vec<&GridEntry>, Vec<&GridEntry>) =
        grid.iter().partition(|g| g.is_player);

    let matched: Vec<(&GridEntry, &Vec<&SeatEntry>)> = ai
        .iter()
        .filter_map(|g| by_key.get(&name_key(g.name)).map(|e| (*g, e)))
        .collect();
    if matched.len() * 2 < ai.len() {
        return PlayerSeat::RosterNotDetected { matched: matched.len(), grid: grid.len() };
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
        .map(|e| Seat { seat: e.seat.clone(), team: e.team.clone() })
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
    seats.iter().map(|s| s.seat.as_str()).collect::<Vec<_>>().join(", ")
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
