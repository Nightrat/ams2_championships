use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

// ── Data structures ──────────────────────────────────────────────────────────

#[derive(Debug)]
struct DriverResult {
    driver_guid: String,
    is_player: bool,
    finish_position: u32,
    points_gain: i32,
    driver_name: String,
    car_name: String,
    was_fastest_lap: bool,
    skipped: bool,
}

#[derive(Debug)]
struct SessionData {
    name: String,
    completion_percentage: f64,
    results: Vec<DriverResult>,
}

#[derive(Debug)]
struct EventData {
    name: String,
    track: String,
    status: String,
    date: String,
    sessions: Vec<SessionData>,
}

#[derive(Debug, Clone)]
struct DriverStanding {
    position: u32,
    name: String,
    total_points: i32,
    is_player: bool,
    last_car: String,
    is_inactive: bool,
    guid: String,
}

#[derive(Debug)]
struct ChampData {
    name: String,
    class: String,
    state: String,
    total_events: u32,
    current_event_index: u32,
    creation_date: String,
    manufacturer_scoring: bool,
    events: Vec<EventData>,
    standings: Vec<DriverStanding>,
}

// ── Driver statistics ─────────────────────────────────────────────────────────

#[derive(Default)]
struct DriverStats {
    name: String,
    is_player: bool,
    finished_seasons: u32,
    races: u32,
    wins: u32,
    top3: u32,
    top10: u32,
    champ_wins: u32,
    champ_top3: u32,
    champ_top10: u32,
    position_sum: u32,
    dnf: u32,
}

// ── String helpers ────────────────────────────────────────────────────────────

fn driver_base_name(name: &str) -> &str {
    name.trim_end_matches(" (AI)").trim_end()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ── XML deserialization types ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct XmlRoot {
    #[serde(rename = "Championships")]
    championships: XmlChampionshipsWrapper,
}

#[derive(Deserialize, Default)]
struct XmlChampionshipsWrapper {
    #[serde(rename = "ChampionshipDto", default)]
    items: Vec<XmlChamp>,
}

#[derive(Deserialize)]
struct XmlChamp {
    #[serde(rename = "@ChampionshipName", default)]
    name: String,
    #[serde(rename = "@ClassName", default)]
    class: String,
    #[serde(rename = "@ChampionshipState", default)]
    state: String,
    #[serde(rename = "@TotalEvents", default)]
    total_events: u32,
    #[serde(rename = "@CurrentEventIndex", default)]
    current_event_index: u32,
    #[serde(rename = "CreationDateTime", default)]
    creation_date: String,
    #[serde(rename = "@IsManufacturerScoringEnabled", default)]
    manufacturer_scoring: bool,
    #[serde(rename = "Events", default)]
    events: XmlEventsWrapper,
    #[serde(rename = "Drivers", default)]
    drivers: XmlDriversWrapper,
}

#[derive(Deserialize, Default)]
struct XmlEventsWrapper {
    #[serde(rename = "EventDto", default)]
    items: Vec<XmlEvent>,
}

#[derive(Deserialize, Default)]
struct XmlEvent {
    #[serde(rename = "@EventName", default)]
    name: String,
    #[serde(rename = "@TrackName", default)]
    track: String,
    #[serde(rename = "@EventStatus", default)]
    status: String,
    #[serde(rename = "@EventDate", default)]
    date: String,
    #[serde(rename = "Sessions", default)]
    sessions: XmlSessionsWrapper,
}

#[derive(Deserialize, Default)]
struct XmlSessionsWrapper {
    #[serde(rename = "SessionDto", default)]
    items: Vec<XmlSession>,
}

#[derive(Deserialize, Default)]
struct XmlSession {
    #[serde(rename = "@Name", default)]
    name: String,
    #[serde(rename = "SessionResult")]
    result: Option<XmlSessionResult>,
}

#[derive(Deserialize, Default)]
struct XmlSessionResult {
    #[serde(rename = "@CompletionPercentage")]
    completion_percentage: Option<f64>,
    #[serde(rename = "DriverSessionResult")]
    driver_results: Option<XmlDriverResultsWrapper>,
}

#[derive(Deserialize, Default)]
struct XmlDriverResultsWrapper {
    #[serde(rename = "DriverSessionResultDto", default)]
    items: Vec<XmlDriverResult>,
}

#[derive(Deserialize, Default)]
struct XmlDriverResult {
    #[serde(rename = "@DriverGuid", default)]
    driver_guid: String,
    #[serde(rename = "@IsPlayer", default)]
    is_player: bool,
    #[serde(rename = "@FinishPosition", default)]
    finish_position: u32,
    #[serde(rename = "@PointsGain", default)]
    points_gain: i32,
    #[serde(rename = "@DriverName", default)]
    driver_name: String,
    #[serde(rename = "@WasFastestLap", default)]
    was_fastest_lap: bool,
    #[serde(rename = "@SkippedEvent", default)]
    skipped: bool,
    #[serde(rename = "@CarName", default)]
    car_name: String,
}

#[derive(Deserialize, Default)]
struct XmlDriversWrapper {
    #[serde(rename = "DriverDto", default)]
    items: Vec<XmlDriverStanding>,
}

#[derive(Deserialize, Default)]
struct XmlDriverStanding {
    #[serde(rename = "@Position", default)]
    position: u32,
    #[serde(rename = "@LastUsedName", default)]
    name: String,
    #[serde(rename = "@TotalPoints", default)]
    total_points: i32,
    #[serde(rename = "@IsPlayer", default)]
    is_player: bool,
    #[serde(rename = "@LastCarName", default)]
    last_car: String,
    #[serde(rename = "@IsInactive", default)]
    is_inactive: bool,
    #[serde(rename = "@GlobalKey", default)]
    guid: String,
}

// ── Conversion from XML types to internal types ───────────────────────────────

impl From<XmlDriverResult> for DriverResult {
    fn from(x: XmlDriverResult) -> Self {
        DriverResult {
            driver_guid: x.driver_guid,
            is_player: x.is_player,
            finish_position: x.finish_position,
            points_gain: x.points_gain,
            driver_name: x.driver_name,
            car_name: x.car_name,
            was_fastest_lap: x.was_fastest_lap,
            skipped: x.skipped,
        }
    }
}

impl From<XmlSession> for SessionData {
    fn from(x: XmlSession) -> Self {
        let (completion_percentage, mut results) = match x.result {
            Some(r) => {
                let pct = r.completion_percentage.unwrap_or(1.0);
                let results = r
                    .driver_results
                    .map(|d| d.items.into_iter().map(DriverResult::from).collect())
                    .unwrap_or_default();
                (pct, results)
            }
            None => (1.0, vec![]),
        };
        results.sort_by_key(|r| r.finish_position);
        SessionData {
            name: x.name,
            completion_percentage,
            results,
        }
    }
}

impl From<XmlEvent> for EventData {
    fn from(x: XmlEvent) -> Self {
        let date = x.date.split('T').next().unwrap_or("").to_string();
        EventData {
            name: x.name,
            track: x.track,
            status: x.status,
            date,
            sessions: x
                .sessions
                .items
                .into_iter()
                .map(SessionData::from)
                .collect(),
        }
    }
}

impl From<XmlDriverStanding> for DriverStanding {
    fn from(x: XmlDriverStanding) -> Self {
        DriverStanding {
            position: if x.position == 0 { 99 } else { x.position },
            name: x.name,
            total_points: x.total_points,
            is_player: x.is_player,
            last_car: x.last_car,
            is_inactive: x.is_inactive,
            guid: x.guid,
        }
    }
}

impl From<XmlChamp> for ChampData {
    fn from(x: XmlChamp) -> Self {
        let creation_date = x.creation_date.split('T').next().unwrap_or("").to_string();
        let mut standings: Vec<DriverStanding> = x
            .drivers
            .items
            .into_iter()
            .map(DriverStanding::from)
            .collect();
        standings.sort_by_key(|d| d.position);
        ChampData {
            name: x.name,
            class: x.class,
            state: x.state,
            total_events: x.total_events,
            current_event_index: x.current_event_index,
            creation_date,
            manufacturer_scoring: x.manufacturer_scoring,
            events: x.events.items.into_iter().map(EventData::from).collect(),
            standings,
        }
    }
}

// ── HTML generation ──────────────────────────────────────────────────────────

fn status_badge(state: &str) -> &'static str {
    match state {
        "Finished" => r#"<span class="badge badge-finished">Finished</span>"#,
        "Active" => r#"<span class="badge badge-active">Active</span>"#,
        _ => r#"<span class="badge badge-pending">Pending</span>"#,
    }
}

fn generate_standings_table(standings: &[DriverStanding]) -> String {
    let rows: String = standings
        .iter()
        .filter(|d| !d.is_inactive)
        .map(|d| {
            let row_cls = if d.is_player { " class=\"player-row\"" } else { "" };
            let player_tag = if d.is_player { " <span class=\"player-tag\">YOU</span>" } else { "" };
            format!(
                "<tr{row_cls}><td class=\"pos\">{pos}</td><td>{name}{player_tag}</td><td class=\"car\">{car}</td><td class=\"pts\">{pts}</td></tr>",
                row_cls = row_cls,
                pos = d.position,
                name = esc(&d.name),
                player_tag = player_tag,
                car = esc(&d.last_car),
                pts = d.total_points,
            )
        })
        .collect();

    format!(
        r#"<table class="standings-table">
  <thead><tr><th>Pos</th><th>Driver</th><th>Car</th><th>Pts</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#,
        rows = rows
    )
}

fn generate_results_grid(c: &ChampData) -> String {
    if c.events.is_empty() || c.standings.is_empty() {
        return "<p>No results available.</p>".to_string();
    }

    // Build per-event lookup: guid -> DriverResult
    let event_maps: Vec<HashMap<&str, &DriverResult>> = c
        .events
        .iter()
        .map(|e| {
            let mut map: HashMap<&str, &DriverResult> = HashMap::new();
            for s in &e.sessions {
                for r in &s.results {
                    map.entry(r.driver_guid.as_str()).or_insert(r);
                }
            }
            map
        })
        .collect();

    let headers: String = c
        .events
        .iter()
        .map(|e| {
            format!(
                "<th title=\"{track}\">{name}</th>",
                track = esc(&e.track),
                name = esc(&e.name)
            )
        })
        .collect();

    let rows: String = c
        .standings
        .iter()
        .filter(|d| !d.is_inactive)
        .map(|driver| {
            let row_cls = if driver.is_player { " class=\"player-row\"" } else { "" };
            let player_tag = if driver.is_player { " <span class=\"player-tag\">YOU</span>" } else { "" };

            let cells: String = event_maps
                .iter()
                .map(|emap| {
                    match emap.get(driver.guid.as_str()) {
                        Some(r) if r.skipped => {
                            "<td class=\"cell-dns\">DNS</td>".to_string()
                        }
                        Some(r) => {
                            let fl = if r.was_fastest_lap { " FL" } else { "" };
                            let pts_cls = if r.points_gain > 0 { "cell-pts" } else { "cell-npts" };
                            format!(
                                "<td class=\"{pts_cls}\" title=\"P{pos}{fl}\">P{pos}<br><small>{pts}pt{fl}</small></td>",
                                pts_cls = pts_cls,
                                pos = r.finish_position,
                                pts = r.points_gain,
                                fl = fl,
                            )
                        }
                        None => "<td class=\"cell-empty\">-</td>".to_string(),
                    }
                })
                .collect();

            format!(
                "<tr{row_cls}><td class=\"grid-driver\">{name}{player_tag}</td>{cells}<td class=\"grid-total\">{pts}</td></tr>",
                row_cls = row_cls,
                name = esc(&driver.name),
                player_tag = player_tag,
                cells = cells,
                pts = driver.total_points,
            )
        })
        .collect();

    format!(
        r#"<div class="grid-scroll">
<table class="results-grid">
  <thead><tr><th>Driver</th>{headers}<th>Total</th></tr></thead>
  <tbody>{rows}</tbody>
</table>
</div>"#,
        headers = headers,
        rows = rows,
    )
}

fn generate_events_detail(events: &[EventData]) -> String {
    events
        .iter()
        .map(|e| {
            let status_cls = match e.status.as_str() {
                "Finished" => "ev-finished",
                "Active" => "ev-active",
                _ => "ev-pending",
            };

            let sessions_html: String = e
                .sessions
                .iter()
                .map(|s| {
                    let top5: String = s
                        .results
                        .iter()
                        .take(5)
                        .map(|r| {
                            let player_cls = if r.is_player { " player-row" } else { "" };
                            let fl = if r.was_fastest_lap { " *FL*" } else { "" };
                            format!(
                                "<tr class=\"{player_cls}\"><td>P{pos}</td><td>{name}{fl}</td><td class=\"pts\">{pts}pt</td></tr>",
                                player_cls = player_cls,
                                pos = r.finish_position,
                                name = esc(&r.driver_name),
                                pts = r.points_gain,
                                fl = fl,
                            )
                        })
                        .collect();

                    format!(
                        r#"<div class="session-block">
  <div class="session-name">{name}</div>
  <table class="session-table"><tbody>{top5}</tbody></table>
</div>"#,
                        name = esc(&s.name),
                        top5 = top5,
                    )
                })
                .collect();

            format!(
                r#"<div class="event-card {status_cls}">
  <div class="event-header">
    <span class="event-name">{name}</span>
    <span class="event-track">{track}</span>
    <span class="event-date">{date}</span>
  </div>
  {sessions_html}
</div>"#,
                status_cls = status_cls,
                name = esc(&e.name),
                track = esc(&e.track),
                date = esc(&e.date),
                sessions_html = sessions_html,
            )
        })
        .collect()
}

fn compute_constructor_standings(events: &[EventData]) -> Vec<(String, i32)> {
    let mut totals: HashMap<String, i32> = HashMap::new();
    for event in events {
        for session in &event.sessions {
            for result in &session.results {
                if !result.skipped && !result.car_name.is_empty() {
                    *totals.entry(result.car_name.clone()).or_default() += result.points_gain;
                }
            }
        }
    }
    let mut standings: Vec<(String, i32)> = totals.into_iter().collect();
    standings.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    standings
}

fn generate_constructor_standings(events: &[EventData]) -> String {
    let standings = compute_constructor_standings(events);
    if standings.len() < 2 {
        return String::new();
    }
    let rows: String = standings
        .iter()
        .enumerate()
        .map(|(i, (car, pts))| {
            format!(
                "<tr><td class=\"pos\">{pos}</td><td>{car}</td><td class=\"pts\">{pts}</td></tr>",
                pos = i + 1,
                car = esc(car),
                pts = pts,
            )
        })
        .collect();
    format!(
        r#"<div class="constructor-panel">
  <h3>Constructor Standings</h3>
  <table class="standings-table">
    <thead><tr><th class="pos">Pos</th><th>Constructor</th><th class="pts">Pts</th></tr></thead>
    <tbody>{rows}</tbody>
  </table>
</div>"#,
        rows = rows,
    )
}

fn generate_championship_section(idx: usize, c: &ChampData) -> String {
    let progress_pct = if c.total_events > 0 {
        (c.current_event_index as f64 / c.total_events as f64 * 100.0) as u32
    } else {
        0
    };

    let standings_html = generate_standings_table(&c.standings);
    let grid_html = generate_results_grid(c);
    let events_html = generate_events_detail(&c.events);
    let constructor_html = if c.manufacturer_scoring {
        generate_constructor_standings(&c.events)
    } else {
        String::new()
    };

    let open_attr = if c.state == "Finished" { "" } else { " open" };

    format!(
        r#"<details id="champ-{idx}" class="championship"{open}>
  <summary class="champ-header">
    <div class="champ-title">
      <h2>{name}</h2>
      {state_badge}
      <span class="class-badge">{class}</span>
    </div>
    <div class="champ-meta">
      <span>Created: {date}</span>
      <span>Events: {current}/{total}</span>
    </div>
    <div class="progress-bar"><div class="progress-fill" style="width:{pct}%"></div></div>
  </summary>

  <div class="champ-body">
    <div class="standings-panel">
      <h3>Standings</h3>
      {standings_html}
    </div>
    {constructor_html}
    <div class="results-panel">
      <h3>Round-by-Round</h3>
      {grid_html}
    </div>
  </div>

  <details class="events-detail">
    <summary>Event Details ({total} rounds)</summary>
    <div class="events-grid">{events_html}</div>
  </details>
</details>"#,
        idx = idx,
        open = open_attr,
        name = esc(&c.name),
        state_badge = status_badge(&c.state),
        class = esc(&c.class),
        date = esc(&c.creation_date),
        current = c.current_event_index,
        total = c.total_events,
        pct = progress_pct,
        standings_html = standings_html,
        constructor_html = constructor_html,
        grid_html = grid_html,
        events_html = events_html,
    )
}

fn compute_driver_stats(championships: &[ChampData]) -> Vec<DriverStats> {
    let mut map: HashMap<String, DriverStats> = HashMap::new();

    for champ in championships {
        // Seed map from standings, keyed by base name to merge across championships
        for standing in &champ.standings {
            let key = driver_base_name(&standing.name).to_string();
            map.entry(key.clone()).or_insert_with(|| DriverStats {
                name: key,
                is_player: standing.is_player,
                ..Default::default()
            });
        }

        if champ.state == "Finished" {
            for standing in &champ.standings {
                let key = driver_base_name(&standing.name);
                if let Some(s) = map.get_mut(key) {
                    s.finished_seasons += 1;
                    if standing.position == 1 {
                        s.champ_wins += 1;
                    }
                    if standing.position <= 3 {
                        s.champ_top3 += 1;
                    }
                    if standing.position <= 10 {
                        s.champ_top10 += 1;
                    }
                }
            }
        }

        for event in &champ.events {
            for session in &event.sessions {
                if !session.name.to_lowercase().contains("race") {
                    continue;
                }
                let is_dnf = session.completion_percentage < 1.0;
                for result in &session.results {
                    if result.skipped {
                        continue;
                    }
                    if let Some(s) = map.get_mut(driver_base_name(&result.driver_name)) {
                        s.races += 1;
                        if is_dnf && result.is_player {
                            s.dnf += 1;
                        }
                        s.position_sum += result.finish_position;
                        if result.finish_position == 1 {
                            s.wins += 1;
                        }
                        if result.finish_position <= 3 {
                            s.top3 += 1;
                        }
                        if result.finish_position <= 10 {
                            s.top10 += 1;
                        }
                    }
                }
            }
        }
    }

    let mut stats: Vec<DriverStats> = map.into_values().collect();
    stats.sort_by(|a, b| {
        b.is_player
            .cmp(&a.is_player)
            .then(b.wins.cmp(&a.wins))
            .then(b.races.cmp(&a.races))
            .then(a.name.cmp(&b.name))
    });
    stats
}

fn fetch_driver_portraits(stats: &[DriverStats]) -> HashMap<String, String> {
    let mut portraits = HashMap::new();
    for s in stats {
        let wiki_name = s.name.replace(' ', "_");
        let url = format!(
            "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
            wiki_name
        );
        let result = ureq::get(&url)
            .set("User-Agent", "ams2-career-tool/1.0 (local HTML generator)")
            .call();
        if let Ok(response) = result {
            if let Ok(json) = response.into_json::<serde_json::Value>() {
                if let Some(src) = json["thumbnail"]["source"].as_str() {
                    portraits.insert(s.name.clone(), src.to_string());
                }
            }
        }
    }
    portraits
}

fn generate_driver_stats_section(
    stats: &[DriverStats],
    portraits: &HashMap<String, String>,
) -> String {
    if stats.is_empty() {
        return String::new();
    }

    let rows: String = stats
        .iter()
        .map(|s| {
            let row_cls = if s.is_player { " class=\"player-row\"" } else { "" };
            let player_tag = if s.is_player { " <span class=\"player-tag\">YOU</span>" } else { "" };
            let portrait_html = if let Some(url) = portraits.get(&s.name) {
                format!(r#"<img class="driver-portrait" src="{url}" alt="{name}">"#, url = url, name = esc(&s.name))
            } else {
                r#"<span class="driver-portrait-placeholder"></span>"#.to_string()
            };
            let avg_pos = if s.races > 0 {
                format!("{:.1}", s.position_sum as f64 / s.races as f64)
            } else {
                "-".to_string()
            };
            format!(
                "<tr{row_cls}><td class=\"stat-name\">{portrait}{name}{player_tag}</td><td class=\"stat-num\">{seasons}</td><td class=\"stat-num\">{races}</td><td class=\"stat-num\">{wins}</td><td class=\"stat-num\">{top3}</td><td class=\"stat-num\">{top10}</td><td class=\"stat-num\">{dnf}</td><td class=\"stat-num\">{champ_wins}</td><td class=\"stat-num\">{champ_top3}</td><td class=\"stat-num\">{champ_top10}</td><td class=\"stat-num\">{avg_pos}</td></tr>",
                row_cls = row_cls,
                portrait = portrait_html,
                name = esc(&s.name),
                player_tag = player_tag,
                seasons = s.finished_seasons,
                races = s.races,
                wins = s.wins,
                top3 = s.top3,
                top10 = s.top10,
                dnf = s.dnf,
                champ_wins = s.champ_wins,
                champ_top3 = s.champ_top3,
                champ_top10 = s.champ_top10,
                avg_pos = avg_pos,
            )
        })
        .collect();

    format!(
        r#"<section id="driver-stats" class="championship">
  <div class="champ-header">
    <div class="champ-title"><h2>Driver Statistics</h2></div>
  </div>
  <div class="stats-body">
    <table class="stats-table sortable" id="stats-table">
      <thead>
        <tr>
          <th class="stat-name sort-asc" data-col="0" data-type="str">Driver</th>
          <th class="stat-num" data-col="1" data-type="num">Seasons</th>
          <th class="stat-num" data-col="2" data-type="num">Races</th>
          <th class="stat-num" data-col="3" data-type="num">Wins</th>
          <th class="stat-num" data-col="4" data-type="num">Top 3</th>
          <th class="stat-num" data-col="5" data-type="num">Top 10</th>
          <th class="stat-num" data-col="6" data-type="num">DNF</th>
          <th class="stat-num" data-col="7" data-type="num">Champ Wins</th>
          <th class="stat-num" data-col="8" data-type="num">Champ Top 3</th>
          <th class="stat-num" data-col="9" data-type="num">Champ Top 10</th>
          <th class="stat-num" data-col="10" data-type="num">Avg Pos</th>
        </tr>
      </thead>
      <tbody>{rows}</tbody>
    </table>
  </div>
</section>"#,
        rows = rows,
    )
}

fn generate_html(
    championships: &[ChampData],
    stats: &[DriverStats],
    portraits: &HashMap<String, String>,
) -> String {
    let nav: String = championships
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let state_cls = match c.state.as_str() {
                "Finished" => "nav-finished",
                "Active" => "nav-active",
                _ => "nav-pending",
            };
            format!(
                r##"<li><a href="#champ-{i}" class="{state_cls}">{name} <small>{class}</small></a></li>"##,
                i = i,
                name = esc(&c.name),
                class = esc(&c.class),
                state_cls = state_cls,
            )
        })
        .collect();

    let sections: String = championships
        .iter()
        .enumerate()
        .map(|(i, c)| generate_championship_section(i, c))
        .collect();

    let stats_section = generate_driver_stats_section(stats, portraits);

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>AMS2 Career Championships</title>
<style>{css}</style>
</head>
<body>
<header>
  <h1>AMS2 Career Championships</h1>
  <div class="tab-bar">
    <button class="tab-btn tab-active" data-tab="live">&#9679; Live Session</button>
    <button class="tab-btn" data-tab="career">&#127942; Championships</button>
    <button class="tab-btn" data-tab="manage">&#9881; Manage</button>
    <button class="tab-btn" data-tab="import">SecondMonitor Import</button>
  </div>
</header>
<main>
  <div id="tab-import" class="tab-panel tab-panel-hidden">
    <div class="sub-tab-bar">
      <button class="sub-tab-btn sub-tab-active" data-subtab="championships">Championships</button>
      <button class="sub-tab-btn" data-subtab="driver-stats">Driver Stats</button>
    </div>
    <div id="subtab-championships" class="sub-tab-panel">
      <nav><ul>{nav}</ul></nav>
      {sections}
    </div>
    <div id="subtab-driver-stats" class="sub-tab-panel sub-tab-panel-hidden">
      {stats_section}
    </div>
  </div>
  <div id="tab-career" class="tab-panel tab-panel-hidden">
    <div id="career-container"></div>
  </div>
  <div id="tab-live" class="tab-panel">
    <section class="live-section">
      <div id="live-status" class="live-status live-disconnected">
        <span class="live-dot"></span>
        <span id="live-status-text">Not connected — start AMS2 and open this page via the server</span>
      </div>
      <div id="live-info" class="live-info">
        <span id="live-session-type"></span>
        <span id="live-race-state"></span>
        <span id="live-track" class="live-track"></span>
        <span id="live-raw-states" class="live-raw-states"></span>
      </div>
      <div class="grid-scroll">
        <table id="live-table" class="live-table">
          <thead>
            <tr>
              <th>Pos</th>
              <th>Driver</th>
              <th>Lap</th>
              <th>Gap</th>
              <th>S1</th>
              <th>S2</th>
              <th>S3</th>
              <th>Best Lap</th>
              <th>Last Lap</th>
            </tr>
          </thead>
          <tbody id="live-tbody">
            <tr><td colspan="9" class="live-empty">Waiting for session data&hellip;</td></tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
  <div id="tab-manage" class="tab-panel tab-panel-hidden">
    <div class="manage-layout">
      <div id="manage-new-form" class="manage-new-form" style="display:none">
        <h3>New Championship</h3>
        <div class="manage-form-row">
          <input id="new-champ-name" type="text" placeholder="Championship name" class="manage-input" style="flex:1">
          <select id="new-champ-points" class="manage-select">
            <option value="25,18,15,12,10,8,6,4,2,1">F1 Modern (25-18-15&hellip;)</option>
            <option value="10,6,4,3,2,1">F1 1991-2002 (10-6-4-3-2-1)</option>
            <option value="9,6,4,3,2,1">F1 Classic (9-6-4-3-2-1)</option>
            <option value="custom">Custom&hellip;</option>
          </select>
          <input id="new-champ-custom" type="text" placeholder="e.g. 25,18,15,12,10" class="manage-input" style="display:none;flex:1">
          <label class="manage-checkbox-label"><input type="checkbox" id="new-champ-manufacturer"> Constructor Scoring</label>
          <button id="new-champ-save" class="manage-btn manage-btn-primary">Create</button>
          <button id="new-champ-cancel" class="manage-btn">Cancel</button>
        </div>
      </div>
      <div class="manage-columns">
        <div class="manage-left">
          <div class="manage-left-header">
            <span>Championships</span>
            <button id="add-champ-btn" class="manage-btn manage-btn-primary">+ New</button>
          </div>
          <div id="champ-list"></div>
        </div>
        <div class="manage-right" id="manage-right">
          <div class="manage-placeholder">Select a championship or create a new one.</div>
        </div>
      </div>
      <div class="manage-danger-zone">
        <button id="purge-sessions-btn" class="manage-btn manage-btn-danger">&#x1f5d1; Delete unassigned sessions</button>
      </div>
      <div id="manage-sessions-panel" class="manage-sessions-panel" style="display:none">
        <div class="manage-sessions-header">
          <span>Available Sessions</span>
          <button id="close-sessions-btn" class="manage-btn">&#x2715; Close</button>
        </div>
        <div id="available-sessions"></div>
      </div>
    </div>
  </div>
</main>
<button id="download-btn" title="Download as static HTML file">&#11015; Download</button>
<script>{js}</script>
</body>
</html>"##,
        css = CSS,
        js = JS,
        nav = nav,
        stats_section = stats_section,
        sections = sections,
    )
}

// ── Entry points ─────────────────────────────────────────────────────────────

/// Generate the full page with no SecondMonitor import data.
/// Used when the server is started without an XML file.
pub fn build_base_html() -> String {
    generate_html(&[], &[], &std::collections::HashMap::new())
}

pub fn build_html_from_xml(xml_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let xml = fs::read_to_string(xml_path)?;
    let root: XmlRoot = quick_xml::de::from_str(&xml)?;

    let mut championships: Vec<ChampData> = root
        .championships
        .items
        .into_iter()
        .map(ChampData::from)
        .collect();

    championships.sort_by(|a, b| b.creation_date.cmp(&a.creation_date));

    let stats = compute_driver_stats(&championships);
    println!("Fetching driver portraits from Wikipedia...");
    let portraits = fetch_driver_portraits(&stats);
    println!(
        "Found portraits for {}/{} drivers.",
        portraits.len(),
        stats.len()
    );

    let html = generate_html(&championships, &stats, &portraits);
    println!("Generated {} championship(s).", championships.len());
    Ok(html)
}

pub fn convert(xml_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let html = build_html_from_xml(xml_path)?;
    fs::write(output_path, &html)?;
    println!("Written to {}", output_path);
    Ok(())
}

// ── Styles ───────────────────────────────────────────────────────────────────

const CSS: &str = include_str!("assets/style.css");

// ── Scripts ──────────────────────────────────────────────────────────────────

const JS: &str = include_str!("assets/script.js");

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn make_result(name: &str, pos: u32, is_player: bool, skipped: bool) -> DriverResult {
        DriverResult {
            driver_guid: format!("guid-{}", name),
            is_player,
            finish_position: pos,
            points_gain: 0,
            driver_name: name.to_string(),
            car_name: String::new(),
            was_fastest_lap: false,
            skipped,
        }
    }

    fn make_session(name: &str, completion: f64, results: Vec<DriverResult>) -> SessionData {
        SessionData {
            name: name.to_string(),
            completion_percentage: completion,
            results,
        }
    }

    fn make_standing(name: &str, pos: u32, is_player: bool) -> DriverStanding {
        DriverStanding {
            position: pos,
            name: name.to_string(),
            total_points: 0,
            is_player,
            last_car: String::new(),
            is_inactive: false,
            guid: format!("guid-{}", name),
        }
    }

    fn make_champ(
        state: &str,
        standings: Vec<DriverStanding>,
        sessions: Vec<SessionData>,
    ) -> ChampData {
        let events = if sessions.is_empty() {
            vec![]
        } else {
            vec![EventData {
                name: "Round 1".into(),
                track: "Track".into(),
                status: "Finished".into(),
                date: "2025-01-01".into(),
                sessions,
            }]
        };
        ChampData {
            name: "Test Champ".into(),
            class: "Class A".into(),
            state: state.to_string(),
            total_events: 1,
            current_event_index: 1,
            creation_date: "2025-01-01".into(),
            manufacturer_scoring: false,
            events,
            standings,
        }
    }

    // ── driver_base_name ─────────────────────────────────────────────────────

    #[test]
    fn test_driver_base_name_strips_ai_suffix() {
        assert_eq!(driver_base_name("Alice (AI)"), "Alice");
    }

    #[test]
    fn test_driver_base_name_strips_ai_with_extra_spaces() {
        assert_eq!(driver_base_name("Alice  (AI)"), "Alice");
    }

    #[test]
    fn test_driver_base_name_preserves_player_name() {
        assert_eq!(driver_base_name("Alice"), "Alice");
    }

    #[test]
    fn test_driver_base_name_empty_string() {
        assert_eq!(driver_base_name(""), "");
    }

    // ── esc ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_esc_ampersand() {
        assert_eq!(esc("a & b"), "a &amp; b");
    }

    #[test]
    fn test_esc_angle_brackets() {
        assert_eq!(esc("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn test_esc_quote() {
        assert_eq!(esc(r#"say "hi""#), "say &quot;hi&quot;");
    }

    #[test]
    fn test_esc_no_special_chars() {
        assert_eq!(esc("plain text"), "plain text");
    }

    // ── status_badge ─────────────────────────────────────────────────────────

    #[test]
    fn test_status_badge_finished() {
        assert!(status_badge("Finished").contains("badge-finished"));
    }

    #[test]
    fn test_status_badge_active() {
        assert!(status_badge("Active").contains("badge-active"));
    }

    #[test]
    fn test_status_badge_unknown_is_pending() {
        assert!(status_badge("Pending").contains("badge-pending"));
        assert!(status_badge("").contains("badge-pending"));
    }

    // ── compute_driver_stats: race counting ──────────────────────────────────

    #[test]
    fn test_stats_counts_race_sessions_only() {
        let standings = vec![make_standing("Alice", 1, false)];
        let sessions = vec![
            make_session(
                "Qualifying",
                1.0,
                vec![make_result("Alice", 3, false, false)],
            ),
            make_session("Race 1", 1.0, vec![make_result("Alice", 1, false, false)]),
        ];
        let champ = make_champ("Finished", standings, sessions);
        let stats = compute_driver_stats(&[champ]);
        let alice = stats.iter().find(|s| s.name == "Alice").unwrap();
        assert_eq!(alice.races, 1, "qualifying should not count as a race");
    }

    #[test]
    fn test_stats_skipped_race_not_counted() {
        let standings = vec![make_standing("Alice", 1, false)];
        let sessions = vec![make_session(
            "Race 1",
            1.0,
            vec![make_result("Alice", 1, false, true)],
        )];
        let champ = make_champ("Finished", standings, sessions);
        let stats = compute_driver_stats(&[champ]);
        let alice = stats.iter().find(|s| s.name == "Alice").unwrap();
        assert_eq!(alice.races, 0);
    }

    #[test]
    fn test_stats_wins_top3_top10() {
        let standings = vec![
            make_standing("Alice", 1, false),
            make_standing("Bob", 2, false),
            make_standing("Carol", 3, false),
            make_standing("Dave", 4, false),
        ];
        let sessions = vec![make_session(
            "Race 1",
            1.0,
            vec![
                make_result("Alice", 1, false, false),
                make_result("Bob", 3, false, false),
                make_result("Carol", 5, false, false),
                make_result("Dave", 11, false, false),
            ],
        )];
        let champ = make_champ("Finished", standings, sessions);
        let stats = compute_driver_stats(&[champ]);

        let alice = stats.iter().find(|s| s.name == "Alice").unwrap();
        assert_eq!(alice.wins, 1);
        assert_eq!(alice.top3, 1);
        assert_eq!(alice.top10, 1);

        let bob = stats.iter().find(|s| s.name == "Bob").unwrap();
        assert_eq!(bob.wins, 0);
        assert_eq!(bob.top3, 1);
        assert_eq!(bob.top10, 1);

        let carol = stats.iter().find(|s| s.name == "Carol").unwrap();
        assert_eq!(carol.wins, 0);
        assert_eq!(carol.top3, 0);
        assert_eq!(carol.top10, 1);

        let dave = stats.iter().find(|s| s.name == "Dave").unwrap();
        assert_eq!(dave.wins, 0);
        assert_eq!(dave.top3, 0);
        assert_eq!(dave.top10, 0);
    }

    // ── compute_driver_stats: DNF ─────────────────────────────────────────────

    #[test]
    fn test_stats_dnf_counted_for_player_on_incomplete_race() {
        let standings = vec![make_standing("Player1", 1, true)];
        let sessions = vec![make_session(
            "Race 1",
            0.6,
            vec![make_result("Player1", 10, true, false)],
        )];
        let champ = make_champ("Finished", standings, sessions);
        let stats = compute_driver_stats(&[champ]);
        let p = stats.iter().find(|s| s.name == "Player1").unwrap();
        assert_eq!(p.dnf, 1);
        assert_eq!(p.races, 1, "DNF still counts as a race entry");
    }

    #[test]
    fn test_stats_dnf_not_counted_on_complete_race() {
        let standings = vec![make_standing("Player1", 1, true)];
        let sessions = vec![make_session(
            "Race 1",
            1.0,
            vec![make_result("Player1", 1, true, false)],
        )];
        let champ = make_champ("Finished", standings, sessions);
        let stats = compute_driver_stats(&[champ]);
        let p = stats.iter().find(|s| s.name == "Player1").unwrap();
        assert_eq!(p.dnf, 0);
    }

    #[test]
    fn test_stats_dnf_not_counted_for_ai_on_incomplete_race() {
        let standings = vec![
            make_standing("Player1", 1, true),
            make_standing("Alice", 2, false),
        ];
        let sessions = vec![make_session(
            "Race 1",
            0.5,
            vec![
                make_result("Player1", 20, true, false),
                make_result("Alice", 1, false, false),
            ],
        )];
        let champ = make_champ("Finished", standings, sessions);
        let stats = compute_driver_stats(&[champ]);
        let alice = stats.iter().find(|s| s.name == "Alice").unwrap();
        assert_eq!(alice.dnf, 0, "AI drivers should not have DNFs attributed");
    }

    #[test]
    fn test_stats_multiple_dnfs_accumulate() {
        let standings = vec![make_standing("Player1", 1, true)];
        let sessions = vec![
            make_session("Race 1", 0.5, vec![make_result("Player1", 10, true, false)]),
            make_session("Race 2", 1.0, vec![make_result("Player1", 1, true, false)]),
            make_session("Race 3", 0.2, vec![make_result("Player1", 10, true, false)]),
        ];
        let events = vec![EventData {
            name: "R1".into(),
            track: "T".into(),
            status: "Finished".into(),
            date: "2025-01-01".into(),
            sessions,
        }];
        let champ = ChampData {
            name: "C".into(),
            class: "X".into(),
            state: "Finished".into(),
            total_events: 1,
            current_event_index: 1,
            creation_date: "2025-01-01".into(),
            manufacturer_scoring: false,
            events,
            standings,
        };
        let stats = compute_driver_stats(&[champ]);
        let p = stats.iter().find(|s| s.name == "Player1").unwrap();
        assert_eq!(p.dnf, 2);
        assert_eq!(p.races, 3);
    }

    // ── compute_driver_stats: seasons and champ positions ────────────────────

    #[test]
    fn test_stats_finished_seasons_only_counted_when_state_finished() {
        let standings = vec![make_standing("Alice", 1, false)];
        let active_champ = make_champ("Active", standings.clone(), vec![]);
        let finished_champ = make_champ("Finished", standings, vec![]);
        let stats = compute_driver_stats(&[active_champ, finished_champ]);
        let alice = stats.iter().find(|s| s.name == "Alice").unwrap();
        assert_eq!(alice.finished_seasons, 1);
    }

    #[test]
    fn test_stats_champ_positions() {
        let standings = vec![
            make_standing("Alice", 1, false),
            make_standing("Bob", 2, false),
            make_standing("Carol", 5, false),
            make_standing("Dave", 11, false),
        ];
        let champ = make_champ("Finished", standings, vec![]);
        let stats = compute_driver_stats(&[champ]);

        let alice = stats.iter().find(|s| s.name == "Alice").unwrap();
        assert_eq!(alice.champ_wins, 1);
        assert_eq!(alice.champ_top3, 1);
        assert_eq!(alice.champ_top10, 1);

        let bob = stats.iter().find(|s| s.name == "Bob").unwrap();
        assert_eq!(bob.champ_wins, 0);
        assert_eq!(bob.champ_top3, 1);
        assert_eq!(bob.champ_top10, 1);

        let carol = stats.iter().find(|s| s.name == "Carol").unwrap();
        assert_eq!(carol.champ_wins, 0);
        assert_eq!(carol.champ_top3, 0);
        assert_eq!(carol.champ_top10, 1);

        let dave = stats.iter().find(|s| s.name == "Dave").unwrap();
        assert_eq!(dave.champ_wins, 0);
        assert_eq!(dave.champ_top3, 0);
        assert_eq!(dave.champ_top10, 0);
    }

    // ── compute_driver_stats: AI name merging ────────────────────────────────

    #[test]
    fn test_stats_merges_ai_and_player_name() {
        let champ1 = make_champ(
            "Finished",
            vec![make_standing("Alice", 1, true)],
            vec![make_session(
                "Race 1",
                1.0,
                vec![make_result("Alice", 1, true, false)],
            )],
        );
        let champ2 = make_champ(
            "Finished",
            vec![make_standing("Alice (AI)", 2, false)],
            vec![make_session(
                "Race 1",
                1.0,
                vec![make_result("Alice (AI)", 2, false, false)],
            )],
        );
        let stats = compute_driver_stats(&[champ1, champ2]);
        let alice: Vec<_> = stats.iter().filter(|s| s.name == "Alice").collect();
        assert_eq!(alice.len(), 1, "Alice and Alice (AI) should be merged");
        assert_eq!(alice[0].races, 2);
        assert_eq!(alice[0].wins, 1);
    }

    // ── compute_driver_stats: average position ───────────────────────────────

    #[test]
    fn test_stats_position_sum_accumulates() {
        let standings = vec![make_standing("Alice", 1, false)];
        let sessions = vec![
            make_session("Race 1", 1.0, vec![make_result("Alice", 4, false, false)]),
            make_session("Race 2", 1.0, vec![make_result("Alice", 2, false, false)]),
        ];
        let events = vec![EventData {
            name: "E".into(),
            track: "T".into(),
            status: "Finished".into(),
            date: "2025-01-01".into(),
            sessions,
        }];
        let champ = ChampData {
            name: "C".into(),
            class: "X".into(),
            state: "Finished".into(),
            total_events: 1,
            current_event_index: 1,
            creation_date: "2025-01-01".into(),
            manufacturer_scoring: false,
            events,
            standings,
        };
        let stats = compute_driver_stats(&[champ]);
        let alice = stats.iter().find(|s| s.name == "Alice").unwrap();
        assert_eq!(alice.position_sum, 6);
        assert_eq!(alice.races, 2);
    }

    // ── generate_driver_stats_section ────────────────────────────────────────

    #[test]
    fn test_generate_driver_stats_section_empty_returns_empty() {
        let html = generate_driver_stats_section(&[], &HashMap::new());
        assert!(html.is_empty());
    }

    #[test]
    fn test_generate_driver_stats_section_contains_dnf_header() {
        let s = DriverStats {
            name: "Alice".into(),
            is_player: false,
            finished_seasons: 1,
            races: 5,
            wins: 1,
            top3: 2,
            top10: 4,
            champ_wins: 0,
            champ_top3: 1,
            champ_top10: 1,
            position_sum: 25,
            dnf: 0,
        };
        let html = generate_driver_stats_section(&[s], &HashMap::new());
        assert!(
            html.contains(">DNF<"),
            "table should have a DNF column header"
        );
    }

    #[test]
    fn test_generate_driver_stats_section_dnf_value_rendered() {
        let s = DriverStats {
            name: "Player1".into(),
            is_player: true,
            finished_seasons: 2,
            races: 10,
            wins: 3,
            top3: 5,
            top10: 8,
            champ_wins: 1,
            champ_top3: 1,
            champ_top10: 2,
            position_sum: 30,
            dnf: 3,
        };
        let html = generate_driver_stats_section(&[s], &HashMap::new());
        assert!(html.contains("YOU"), "player row should be marked");
        assert!(html.contains("player-row"), "player row should have class");
        assert!(html.contains(r#"<td class="stat-num">3</td>"#));
    }

    // ── XML deserialization ───────────────────────────────────────────────────

    fn parse_session_xml(xml: &str) -> SessionData {
        let x: XmlSession = quick_xml::de::from_str(xml).unwrap();
        SessionData::from(x)
    }

    #[test]
    fn test_parse_session_reads_completion_percentage() {
        let xml = r#"<SessionDto Name="Race 1">
  <SessionResult CompletionPercentage="0.75">
    <DriverSessionResult>
      <DriverSessionResultDto IsPlayer="true" FinishPosition="5" PointsGain="2"
        DriverGuid="abc" DriverName="Alice" WasFastestLap="false" SkippedEvent="false" />
    </DriverSessionResult>
  </SessionResult>
</SessionDto>"#;
        let session = parse_session_xml(xml);
        assert!((session.completion_percentage - 0.75).abs() < f64::EPSILON);
        assert_eq!(session.results.len(), 1);
        assert_eq!(session.results[0].finish_position, 5);
    }

    #[test]
    fn test_parse_session_defaults_completion_to_1_when_missing() {
        let xml = r#"<SessionDto Name="Race 1">
  <SessionResult>
    <DriverSessionResult />
  </SessionResult>
</SessionDto>"#;
        let session = parse_session_xml(xml);
        assert!((session.completion_percentage - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_session_skipped_event_flag() {
        let xml = r#"<SessionDto Name="Race 1">
  <SessionResult CompletionPercentage="1">
    <DriverSessionResult>
      <DriverSessionResultDto IsPlayer="false" FinishPosition="1" PointsGain="9"
        DriverGuid="g1" DriverName="Bob" WasFastestLap="false" SkippedEvent="true" />
    </DriverSessionResult>
  </SessionResult>
</SessionDto>"#;
        let session = parse_session_xml(xml);
        assert!(session.results[0].skipped);
    }

    #[test]
    fn test_parse_session_results_sorted_by_finish_position() {
        let xml = r#"<SessionDto Name="Race 1">
  <SessionResult CompletionPercentage="1">
    <DriverSessionResult>
      <DriverSessionResultDto IsPlayer="false" FinishPosition="3" PointsGain="0"
        DriverGuid="g3" DriverName="C" WasFastestLap="false" SkippedEvent="false" />
      <DriverSessionResultDto IsPlayer="false" FinishPosition="1" PointsGain="9"
        DriverGuid="g1" DriverName="A" WasFastestLap="false" SkippedEvent="false" />
      <DriverSessionResultDto IsPlayer="false" FinishPosition="2" PointsGain="6"
        DriverGuid="g2" DriverName="B" WasFastestLap="false" SkippedEvent="false" />
    </DriverSessionResult>
  </SessionResult>
</SessionDto>"#;
        let session = parse_session_xml(xml);
        let positions: Vec<u32> = session.results.iter().map(|r| r.finish_position).collect();
        assert_eq!(positions, vec![1, 2, 3]);
    }

    // ── compute_constructor_standings ─────────────────────────────────────────

    fn make_result_with_car(name: &str, pos: u32, car: &str, pts: i32, skipped: bool) -> DriverResult {
        DriverResult {
            driver_guid: format!("guid-{}", name),
            is_player: false,
            finish_position: pos,
            points_gain: pts,
            driver_name: name.to_string(),
            car_name: car.to_string(),
            was_fastest_lap: false,
            skipped,
        }
    }

    fn make_event(results: Vec<DriverResult>) -> EventData {
        EventData {
            name: "Round 1".into(),
            track: "Track".into(),
            status: "Finished".into(),
            date: "2025-01-01".into(),
            sessions: vec![make_session("Race", 1.0, results)],
        }
    }

    #[test]
    fn test_constructor_standings_sums_points_by_car() {
        let events = vec![make_event(vec![
            make_result_with_car("Alice", 1, "Ferrari", 25, false),
            make_result_with_car("Bob",   2, "Mercedes", 18, false),
            make_result_with_car("Carol", 3, "Ferrari", 15, false),
        ])];
        let standings = compute_constructor_standings(&events);
        let ferrari = standings.iter().find(|(c, _)| c == "Ferrari").map(|(_, p)| *p);
        let mercedes = standings.iter().find(|(c, _)| c == "Mercedes").map(|(_, p)| *p);
        assert_eq!(ferrari, Some(40));
        assert_eq!(mercedes, Some(18));
    }

    #[test]
    fn test_constructor_standings_sorted_descending() {
        let events = vec![make_event(vec![
            make_result_with_car("A", 1, "Alfa",    10, false),
            make_result_with_car("B", 2, "BMW",     20, false),
            make_result_with_car("C", 3, "Citroën", 15, false),
        ])];
        let standings = compute_constructor_standings(&events);
        let pts: Vec<i32> = standings.iter().map(|(_, p)| *p).collect();
        assert!(pts.windows(2).all(|w| w[0] >= w[1]), "standings must be sorted descending");
    }

    #[test]
    fn test_constructor_standings_skipped_results_excluded() {
        let events = vec![make_event(vec![
            make_result_with_car("Alice", 1, "Ferrari", 25, false),
            make_result_with_car("Bob",   2, "Ferrari", 18, true), // skipped
        ])];
        let standings = compute_constructor_standings(&events);
        let ferrari_pts = standings.iter().find(|(c, _)| c == "Ferrari").map(|(_, p)| *p);
        assert_eq!(ferrari_pts, Some(25), "skipped results must not count");
    }

    #[test]
    fn test_constructor_standings_empty_car_name_excluded() {
        let events = vec![make_event(vec![
            make_result_with_car("Alice", 1, "",        25, false),
            make_result_with_car("Bob",   2, "Ferrari", 18, false),
        ])];
        let standings = compute_constructor_standings(&events);
        assert!(!standings.iter().any(|(c, _)| c.is_empty()), "empty car name must be excluded");
        assert_eq!(standings.len(), 1);
    }

    #[test]
    fn test_constructor_standings_empty_events_returns_empty() {
        let standings = compute_constructor_standings(&[]);
        assert!(standings.is_empty());
    }

    // ── generate_standings_table ──────────────────────────────────────────────

    fn make_standing_with_pts(name: &str, pos: u32, pts: i32) -> DriverStanding {
        DriverStanding {
            position: pos,
            name: name.to_string(),
            total_points: pts,
            is_player: false,
            last_car: "Car".into(),
            is_inactive: false,
            guid: format!("guid-{}", name),
        }
    }

    #[test]
    fn test_generate_standings_table_contains_driver_names() {
        let standings = vec![
            make_standing_with_pts("Alice", 1, 50),
            make_standing_with_pts("Bob",   2, 30),
        ];
        let html = generate_standings_table(&standings);
        assert!(html.contains("Alice"));
        assert!(html.contains("Bob"));
    }

    #[test]
    fn test_generate_standings_table_contains_points() {
        let standings = vec![make_standing_with_pts("Alice", 1, 75)];
        let html = generate_standings_table(&standings);
        assert!(html.contains("75"));
    }

    #[test]
    fn test_generate_standings_table_inactive_drivers_excluded() {
        let mut inactive = make_standing_with_pts("Ghost", 3, 10);
        inactive.is_inactive = true;
        let standings = vec![make_standing_with_pts("Alice", 1, 50), inactive];
        let html = generate_standings_table(&standings);
        assert!(!html.contains("Ghost"), "inactive drivers must not appear in standings");
    }

    #[test]
    fn test_generate_standings_table_player_marked() {
        let mut s = make_standing_with_pts("Player1", 1, 100);
        s.is_player = true;
        let html = generate_standings_table(&[s]);
        assert!(html.contains("player-row") || html.contains("YOU"));
    }

    #[test]
    fn test_generate_standings_table_escapes_html() {
        let standings = vec![make_standing_with_pts("<b>Evil</b>", 1, 10)];
        let html = generate_standings_table(&standings);
        assert!(!html.contains("<b>"), "driver name must be HTML-escaped");
        assert!(html.contains("&lt;b&gt;"));
    }

    // ── generate_championship_section ─────────────────────────────────────────

    #[test]
    fn test_championship_section_contains_name() {
        let c = make_champ("Active", vec![make_standing("Alice", 1, false)], vec![]);
        let html = generate_championship_section(0, &c);
        assert!(html.contains("Test Champ"));
    }

    #[test]
    fn test_championship_section_active_is_open() {
        let c = make_champ("Active", vec![], vec![]);
        let html = generate_championship_section(0, &c);
        assert!(html.contains("<details") && html.contains(" open"));
    }

    #[test]
    fn test_championship_section_finished_is_closed() {
        let c = make_champ("Finished", vec![], vec![]);
        let html = generate_championship_section(0, &c);
        // A closed <details> has no `open` attribute
        assert!(!html.contains(" open"));
    }

    #[test]
    fn test_championship_section_has_status_badge() {
        let c = make_champ("Active", vec![], vec![]);
        let html = generate_championship_section(0, &c);
        assert!(html.contains("badge-active"));
    }

    #[test]
    fn test_championship_section_escapes_name() {
        let mut c = make_champ("Active", vec![], vec![]);
        c.name = "M&M's <Special>".into();
        let html = generate_championship_section(0, &c);
        assert!(!html.contains("<Special>"), "name must be HTML-escaped");
        assert!(html.contains("M&amp;M"));
    }

    #[test]
    fn test_championship_section_manufacturer_scoring_shows_constructor_panel() {
        let mut c = make_champ(
            "Active",
            vec![],
            vec![make_session("Race", 1.0, vec![
                make_result_with_car("Alice", 1, "Ferrari", 25, false),
                make_result_with_car("Bob",   2, "Renault", 18, false),
            ])],
        );
        c.manufacturer_scoring = true;
        let html = generate_championship_section(0, &c);
        assert!(html.contains("Constructor"), "constructor panel must be present when manufacturer scoring is on");
    }

    #[test]
    fn test_championship_section_no_manufacturer_scoring_hides_constructor_panel() {
        let c = make_champ(
            "Active",
            vec![],
            vec![make_session("Race", 1.0, vec![
                make_result_with_car("Alice", 1, "Ferrari", 25, false),
            ])],
        );
        // manufacturer_scoring defaults to false in make_champ
        let html = generate_championship_section(0, &c);
        assert!(!html.contains("Constructor"));
    }
}
