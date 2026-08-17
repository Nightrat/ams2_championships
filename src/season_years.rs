//! Maps AMS2 open-wheeler class names to the real-world F1 season each one is modelled on.
//!
//! Every entry through `F-V10_Gen3` is confirmed against the game's own class registry
//! (`GUI/HUD_1_6/HUD_ColoursDefs.xml`). The last four are a best-effort guess: informal names
//! ("F-V8 Gen1/2/3", "F-Hybrid Gen1/2/3") didn't match anything in that registry, but `F-Reiza`
//! (ungenned) and `F-Ultimate`/`F-Ultimate_Gen1`/`F-Ultimate_Gen2` sit chronologically where those
//! eras would go — `F-Reiza` stands in for the whole V8 era (2006-2012) at a representative single
//! year since there's only one class, not three. `F-Edge` had no plausible match at all and is
//! omitted entirely.
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

/// Looks up the real-world F1 season year a class is modelled on, if known.
pub fn season_year(class: &str) -> Option<u16> {
    SEASON_YEARS
        .iter()
        .find(|(name, _)| *name == class)
        .map(|(_, year)| *year)
}

#[cfg(test)]
#[path = "tests/season_years.rs"]
mod tests;
