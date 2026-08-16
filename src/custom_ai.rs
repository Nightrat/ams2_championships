use std::collections::HashMap;
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
/// year (e.g. "1986"), the car number ("#17"), and the driver name that follow it.
/// Examples:
///   "Brabham-Repco #1 J. Brabham"        -> "Brabham-Repco"
///   "1986 AGS #31 - I. Capelli"          -> "AGS"
fn extract_team_name(livery: &str) -> String {
    let livery = livery.trim();
    let without_year = match livery.split_once(' ') {
        Some((year, rest)) if year.len() == 4 && year.bytes().all(|b| b.is_ascii_digit()) => rest,
        _ => livery,
    };
    match without_year.split_once(" #") {
        Some((team, _)) => team.trim().to_string(),
        None => without_year.trim().to_string(),
    }
}

/// Parses `<driver livery_name="..."><name>...</name>...</driver>` blocks.
/// Track-specific override blocks (which repeat `livery_name` but omit `<name>`) are skipped —
/// only the primary block per driver carries the display name.
pub fn parse_driver_teams_str(xml: &str) -> HashMap<String, String> {
    let xml = strip_comments(xml);
    let mut map = HashMap::new();
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

        if let (Some(name), Some(livery)) = (element_text(block, "name"), livery) {
            map.entry(name.to_string()).or_insert_with(|| extract_team_name(&livery));
        }

        rest = &rest[block_end..];
    }
    map
}

#[cfg(test)]
#[path = "tests/custom_ai.rs"]
mod tests;
