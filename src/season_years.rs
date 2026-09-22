//! Maps AMS2 open-wheeler class names to the real-world F1 season each one is modelled on.
//!
//! Every entry through `F-V10_Gen3` is confirmed against the game's own class registry
//! (`GUI/HUD_1_6/HUD_ColoursDefs.xml`). The last four are a best-effort guess: informal names
//! ("F-V8 Gen1/2/3", "F-Hybrid Gen1/2/3") didn't match anything in that registry, but `F-Reiza`
//! (ungenned) and `F-Ultimate`/`F-Ultimate_Gen1`/`F-Ultimate_Gen2` sit chronologically where those
//! eras would go — `F-Reiza` stands in for the whole V8 era (2006-2012) at a representative single
//! year since there's only one class, not three.
//!
//! Formula Edge (`FE-G1` in the registry, which is why a search for "F-Edge" found nothing) is
//! deliberately absent: it is a *fictional* car, so the season it models is whichever livery mod
//! is installed on it. That is the case the table cannot answer for anyone, and it is what
//! [`season_year_with`] exists for.
use std::collections::BTreeMap;

pub const SEASON_YEARS: &[(&str, u16)] = &[
    ("F-Vintage_Gen1", 1967),
    ("F-Vintage_Gen2", 1969),
    ("F-Retro_Gen1", 1974),
    ("F-Retro_Gen2", 1978),
    ("F-Retro_Gen3", 1983),
    ("F-Classic_Gen1", 1986),
    ("F-Classic_Gen2", 1988),
    ("F-Classic_Gen3", 1990),
    ("F-Classic_Gen4", 1991),
    ("F-Hitech_Gen1", 1992),
    ("F-Hitech_Gen2", 1993),
    ("F-V10_Gen1", 1997),
    ("F-V10_Gen2", 2001),
    ("F-V10_Gen3", 2005),
    ("F-Reiza", 2008),
    ("F-Ultimate", 2016),
    ("F-Ultimate_Gen1", 2020),
    ("F-Ultimate_Gen2", 2025),
];

/// Bounds a hand-entered year is held within. Not a claim about what AMS2 models — just far
/// enough either side of the shipped table that a slipped digit cannot produce a year no table
/// could sort sensibly.
pub const YEAR_MIN: u16 = 1900;
pub const YEAR_MAX: u16 = 2100;

/// Looks up the real-world F1 season year a class is modelled on, if known.
pub fn season_year(class: &str) -> Option<u16> {
    SEASON_YEARS
        .iter()
        .find(|(name, _)| *name == class)
        .map(|(_, year)| *year)
}

/// [`season_year`], with the user's own answer taking precedence (`config.class_years`).
///
/// The table can only cover the ladder Reiza ships, and for some classes a name is not enough to
/// settle the question at all: a modded class models whatever grid its livery mod paints on it,
/// so one install's `FE-G1` is a 1995 season and another's is not. The override is therefore both
/// the answer for a class the table cannot know and the correction for one it has wrong. It is
/// keyed on the class name because that is the only thing the table, the roster file and AMS2's
/// own registry all agree on.
pub fn season_year_with(class: &str, overrides: &BTreeMap<String, u16>) -> Option<u16> {
    overrides.get(class).copied().or_else(|| season_year(class))
}

#[cfg(test)]
#[path = "tests/season_years.rs"]
mod tests;
