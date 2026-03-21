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

#[derive(Debug)]
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

// ── XML helpers ──────────────────────────────────────────────────────────────

fn attr(node: roxmltree::Node, name: &str) -> String {
    node.attribute(name).unwrap_or("").to_string()
}

fn driver_base_name(name: &str) -> &str {
    name.trim_end_matches(" (AI)").trim_end()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ── XML parsing ──────────────────────────────────────────────────────────────

fn parse_driver_result(node: roxmltree::Node) -> DriverResult {
    DriverResult {
        driver_guid: attr(node, "DriverGuid"),
        is_player: attr(node, "IsPlayer") == "true",
        finish_position: attr(node, "FinishPosition").parse().unwrap_or(0),
        points_gain: attr(node, "PointsGain").parse().unwrap_or(0),
        driver_name: attr(node, "DriverName"),
        was_fastest_lap: attr(node, "WasFastestLap") == "true",
        skipped: attr(node, "SkippedEvent") == "true",
    }
}

fn parse_session(node: roxmltree::Node) -> SessionData {
    let name = attr(node, "Name");
    let mut results = Vec::new();
    let mut completion_percentage = 1.0f64;

    if let Some(result_node) = node
        .children()
        .find(|n| n.tag_name().name() == "SessionResult")
    {
        completion_percentage = attr(result_node, "CompletionPercentage")
            .parse()
            .unwrap_or(1.0);
        if let Some(dr_node) = result_node
            .children()
            .find(|n| n.tag_name().name() == "DriverSessionResult")
        {
            results = dr_node
                .children()
                .filter(|n| n.tag_name().name() == "DriverSessionResultDto")
                .map(parse_driver_result)
                .collect();
        }
    }

    results.sort_by_key(|r| r.finish_position);
    SessionData { name, completion_percentage, results }
}

fn parse_event(node: roxmltree::Node) -> EventData {
    let name = attr(node, "EventName");
    let track = attr(node, "TrackName");
    let status = attr(node, "EventStatus");
    let date_raw = attr(node, "EventDate");
    let date = date_raw.split('T').next().unwrap_or("").to_string();

    let sessions = node
        .children()
        .find(|n| n.tag_name().name() == "Sessions")
        .map(|sn| {
            sn.children()
                .filter(|n| n.tag_name().name() == "SessionDto")
                .map(parse_session)
                .collect()
        })
        .unwrap_or_default();

    EventData {
        name,
        track,
        status,
        date,
        sessions,
    }
}

fn parse_championship(node: roxmltree::Node) -> ChampData {
    let name = attr(node, "ChampionshipName");
    let class = attr(node, "ClassName");
    let state = attr(node, "ChampionshipState");
    let total_events = attr(node, "TotalEvents").parse().unwrap_or(0);
    let current_event_index = attr(node, "CurrentEventIndex").parse().unwrap_or(0);

    let mut events = Vec::new();
    let mut standings = Vec::new();
    let mut creation_date = String::new();

    for child in node.children() {
        match child.tag_name().name() {
            "CreationDateTime" => {
                creation_date = child
                    .text()
                    .unwrap_or("")
                    .split('T')
                    .next()
                    .unwrap_or("")
                    .to_string();
            }
            "Events" => {
                events = child
                    .children()
                    .filter(|n| n.tag_name().name() == "EventDto")
                    .map(parse_event)
                    .collect();
            }
            "Drivers" => {
                standings = child
                    .children()
                    .filter(|n| n.tag_name().name() == "DriverDto")
                    .map(|n| DriverStanding {
                        position: attr(n, "Position").parse().unwrap_or(99),
                        name: attr(n, "LastUsedName"),
                        total_points: attr(n, "TotalPoints").parse().unwrap_or(0),
                        is_player: attr(n, "IsPlayer") == "true",
                        last_car: attr(n, "LastCarName"),
                        is_inactive: attr(n, "IsInactive") == "true",
                        guid: attr(n, "GlobalKey"),
                    })
                    .collect();
                standings.sort_by_key(|d| d.position);
            }
            _ => {}
        }
    }

    ChampData {
        name,
        class,
        state,
        total_events,
        current_event_index,
        creation_date,
        events,
        standings,
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

fn generate_championship_section(idx: usize, c: &ChampData) -> String {
    let progress_pct = if c.total_events > 0 {
        (c.current_event_index as f64 / c.total_events as f64 * 100.0) as u32
    } else {
        0
    };

    let standings_html = generate_standings_table(&c.standings);
    let grid_html = generate_results_grid(c);
    let events_html = generate_events_detail(&c.events);

    format!(
        r#"<section id="champ-{idx}" class="championship">
  <div class="champ-header">
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
  </div>

  <div class="champ-body">
    <div class="standings-panel">
      <h3>Standings</h3>
      {standings_html}
    </div>
    <div class="results-panel">
      <h3>Round-by-Round</h3>
      {grid_html}
    </div>
  </div>

  <details class="events-detail">
    <summary>Event Details ({total} rounds)</summary>
    <div class="events-grid">{events_html}</div>
  </details>
</section>"#,
        idx = idx,
        name = esc(&c.name),
        state_badge = status_badge(&c.state),
        class = esc(&c.class),
        date = esc(&c.creation_date),
        current = c.current_event_index,
        total = c.total_events,
        pct = progress_pct,
        standings_html = standings_html,
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
            map.entry(key.clone())
                .or_insert_with(|| DriverStats {
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
                    if standing.position == 1 { s.champ_wins += 1; }
                    if standing.position <= 3  { s.champ_top3 += 1; }
                    if standing.position <= 10 { s.champ_top10 += 1; }
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

fn generate_driver_stats_section(stats: &[DriverStats], portraits: &HashMap<String, String>) -> String {
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

fn generate_html(championships: &[ChampData], stats: &[DriverStats], portraits: &HashMap<String, String>) -> String {
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
    <button class="tab-btn tab-active" data-tab="championships">Championships</button>
    <button class="tab-btn" data-tab="driver-stats">Driver Stats</button>
  </div>
</header>
<main>
  <div id="tab-championships" class="tab-panel">
    <nav><ul>{nav}</ul></nav>
    {sections}
  </div>
  <div id="tab-driver-stats" class="tab-panel tab-panel-hidden">
    {stats_section}
  </div>
</main>
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

// ── Entry point ──────────────────────────────────────────────────────────────

pub fn convert(xml_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let xml = fs::read_to_string(xml_path)?;
    let doc = roxmltree::Document::parse(&xml)?;
    let root = doc.root_element();

    let championships_node = root
        .children()
        .find(|n| n.tag_name().name() == "Championships")
        .ok_or("No <Championships> element found")?;

    let mut championships: Vec<ChampData> = championships_node
        .children()
        .filter(|n| n.tag_name().name() == "ChampionshipDto")
        .map(parse_championship)
        .collect();

    championships.sort_by(|a, b| b.creation_date.cmp(&a.creation_date));

    let stats = compute_driver_stats(&championships);
    println!("Fetching driver portraits from Wikipedia...");
    let portraits = fetch_driver_portraits(&stats);
    println!("Found portraits for {}/{} drivers.", portraits.len(), stats.len());

    let html = generate_html(&championships, &stats, &portraits);
    fs::write(output_path, &html)?;

    println!(
        "Generated {} championship(s) -> {}",
        championships.len(),
        output_path
    );
    Ok(())
}

// ── Styles ───────────────────────────────────────────────────────────────────

const CSS: &str = r#"
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

body {
  background: #0d0d1a;
  color: #e0e0e0;
  font-family: 'Segoe UI', system-ui, sans-serif;
  font-size: 14px;
  line-height: 1.5;
}

/* Header */
header {
  background: linear-gradient(135deg, #1a0a2e 0%, #0d0d1a 100%);
  border-bottom: 2px solid #c0392b;
  padding: 1rem 2rem;
  position: sticky;
  top: 0;
  z-index: 100;
}

header h1 {
  font-size: 1.4rem;
  color: #e74c3c;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  margin-bottom: 0.5rem;
}

/* Tabs */
.tab-bar {
  display: flex;
  gap: 0.25rem;
  margin-top: 0.75rem;
}

.tab-btn {
  background: #1a1a2a;
  color: #95a5a6;
  border: 1px solid #2a2a4a;
  border-bottom: none;
  padding: 0.35rem 1rem;
  border-radius: 4px 4px 0 0;
  cursor: pointer;
  font-size: 0.82rem;
  font-weight: 600;
  font-family: inherit;
  letter-spacing: 0.04em;
  transition: background 0.15s;
}

.tab-btn:hover { background: #22223a; color: #bdc3c7; }

.tab-btn.tab-active {
  background: #12122a;
  color: #e74c3c;
  border-color: #e74c3c;
  border-bottom-color: #12122a;
}

.tab-panel { display: block; }
.tab-panel-hidden { display: none; }

nav {
  padding: 0.6rem 2rem;
  background: #12122a;
  border-bottom: 1px solid #2a2a4a;
}

nav ul {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  list-style: none;
}

nav a {
  display: inline-block;
  padding: 0.25rem 0.7rem;
  border-radius: 3px;
  text-decoration: none;
  font-size: 0.8rem;
  font-weight: 600;
  border: 1px solid transparent;
  transition: background 0.15s;
}

nav a small { font-weight: normal; opacity: 0.7; margin-left: 0.3rem; }

nav a.nav-finished { background: #1a2a1a; border-color: #2ecc71; color: #2ecc71; }
nav a.nav-active   { background: #2a1a0a; border-color: #f39c12; color: #f39c12; }
nav a.nav-pending  { background: #1a1a2a; border-color: #7f8c8d; color: #7f8c8d; }
nav a:hover        { filter: brightness(1.3); }

/* Main */
main { padding: 1.5rem 2rem; }

/* Championship section */
.championship {
  background: #12122a;
  border: 1px solid #2a2a4a;
  border-radius: 6px;
  margin-bottom: 2rem;
  overflow: hidden;
}

.champ-header {
  background: linear-gradient(135deg, #1e1e3a, #151528);
  padding: 1rem 1.5rem;
  border-bottom: 1px solid #2a2a4a;
}

.champ-title {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-wrap: wrap;
  margin-bottom: 0.4rem;
}

.champ-title h2 {
  font-size: 1.2rem;
  color: #e0e0f0;
}

.badge {
  padding: 0.15rem 0.5rem;
  border-radius: 3px;
  font-size: 0.7rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.badge-finished { background: #1e4d2b; color: #2ecc71; border: 1px solid #2ecc71; }
.badge-active   { background: #4d3000; color: #f39c12; border: 1px solid #f39c12; }
.badge-pending  { background: #2a2a2a; color: #95a5a6; border: 1px solid #95a5a6; }

.class-badge {
  background: #2a1a3a;
  color: #9b59b6;
  border: 1px solid #9b59b6;
  padding: 0.15rem 0.5rem;
  border-radius: 3px;
  font-size: 0.7rem;
  font-weight: 700;
}

.champ-meta {
  display: flex;
  gap: 1.5rem;
  font-size: 0.78rem;
  color: #888;
  margin-bottom: 0.5rem;
}

.progress-bar {
  height: 4px;
  background: #2a2a4a;
  border-radius: 2px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #c0392b, #e74c3c);
  border-radius: 2px;
}

/* Championship body */
.champ-body {
  display: flex;
  gap: 0;
  padding: 1rem 1.5rem;
  flex-wrap: wrap;
}

.standings-panel {
  min-width: 260px;
  flex: 0 0 auto;
  margin-right: 1.5rem;
}

.results-panel { flex: 1 1 auto; min-width: 0; }

.standings-panel h3,
.results-panel h3 {
  font-size: 0.85rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: #e74c3c;
  margin-bottom: 0.6rem;
  padding-bottom: 0.3rem;
  border-bottom: 1px solid #2a2a4a;
}

/* Tables */
table { border-collapse: collapse; width: 100%; }

.standings-table td,
.standings-table th {
  padding: 0.35rem 0.6rem;
  text-align: left;
  border-bottom: 1px solid #1e1e38;
}

.standings-table th {
  font-size: 0.72rem;
  text-transform: uppercase;
  color: #7f8c8d;
  background: #0f0f25;
  letter-spacing: 0.04em;
}

.standings-table tr:hover { background: #1a1a35; }

.standings-table .pos { text-align: center; width: 2.5rem; color: #95a5a6; font-size: 0.8rem; }
.standings-table .pts { text-align: right; font-weight: 700; color: #e0e0f0; width: 3rem; }
.standings-table .car { color: #888; font-size: 0.82rem; }

/* Player rows */
.player-row { background: #1a1a00 !important; }
.player-row td { border-bottom-color: #333300 !important; }
.player-tag {
  background: #4d4d00;
  color: #f1c40f;
  font-size: 0.65rem;
  font-weight: 700;
  padding: 0.1rem 0.3rem;
  border-radius: 2px;
  margin-left: 0.3rem;
  vertical-align: middle;
}

/* Results grid */
.grid-scroll { overflow-x: auto; }

.results-grid th,
.results-grid td {
  padding: 0.3rem 0.5rem;
  text-align: center;
  border: 1px solid #1e1e38;
  font-size: 0.78rem;
  white-space: nowrap;
}

.results-grid th {
  background: #0f0f25;
  color: #7f8c8d;
  font-size: 0.7rem;
  text-transform: uppercase;
}

.results-grid .grid-driver {
  text-align: left;
  font-weight: 500;
  min-width: 120px;
  color: #ccc;
}

.results-grid .grid-total {
  font-weight: 700;
  color: #e0e0f0;
  background: #141428;
}

.results-grid .cell-pts  { color: #2ecc71; }
.results-grid .cell-npts { color: #666; }
.results-grid .cell-dns  { color: #e74c3c; font-size: 0.7rem; }
.results-grid .cell-empty { color: #333; }

.results-grid small { font-size: 0.65rem; color: #888; }

/* Events detail */
.events-detail {
  border-top: 1px solid #2a2a4a;
  padding: 0 1.5rem;
}

.events-detail summary {
  padding: 0.75rem 0;
  font-size: 0.82rem;
  color: #95a5a6;
  cursor: pointer;
  user-select: none;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.events-detail summary:hover { color: #bdc3c7; }

.events-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
  padding: 0.75rem 0 1rem;
}

.event-card {
  background: #0f0f22;
  border: 1px solid #2a2a4a;
  border-radius: 4px;
  padding: 0.75rem;
  min-width: 160px;
  flex: 0 0 auto;
}

.event-card.ev-finished { border-color: #1e4d2b; }
.event-card.ev-active   { border-color: #4d3000; }

.event-header {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  margin-bottom: 0.5rem;
  padding-bottom: 0.4rem;
  border-bottom: 1px solid #1e1e38;
}

.event-name  { font-weight: 700; font-size: 0.82rem; color: #e0e0f0; }
.event-track { font-size: 0.72rem; color: #888; }
.event-date  { font-size: 0.7rem; color: #555; }

.session-block { margin-top: 0.4rem; }
.session-name  { font-size: 0.7rem; color: #e74c3c; text-transform: uppercase; margin-bottom: 0.2rem; }

.session-table td { padding: 0.15rem 0.3rem; font-size: 0.75rem; border: none; }
.session-table .pts { text-align: right; color: #2ecc71; }

/* Driver stats section */
.stats-body { padding: 1rem 1.5rem; }

.stats-table { border-collapse: collapse; width: auto; }

.stats-table th,
.stats-table td {
  padding: 0.35rem 0.8rem;
  border-bottom: 1px solid #1e1e38;
  text-align: left;
}

.stats-table th {
  font-size: 0.72rem;
  text-transform: uppercase;
  color: #7f8c8d;
  background: #0f0f25;
  letter-spacing: 0.04em;
}

.stats-table tr:hover { background: #1a1a35; }

.stats-table .stat-name { min-width: 160px; }
.stats-table .stat-num  { text-align: right; font-weight: 600; color: #e0e0f0; min-width: 60px; }

.driver-portrait {
  width: 32px;
  height: 40px;
  object-fit: cover;
  object-position: top;
  border-radius: 2px;
  vertical-align: middle;
  margin-right: 0.5rem;
}

.driver-portrait-placeholder {
  display: inline-block;
  width: 32px;
  height: 40px;
  background: #1e1e38;
  border-radius: 2px;
  vertical-align: middle;
  margin-right: 0.5rem;
}

/* Sortable table headers */
.sortable th {
  cursor: pointer;
  user-select: none;
  white-space: nowrap;
}
.sortable th::after { content: ' \2195'; opacity: 0.3; font-size: 0.7em; }
.sortable th.sort-asc::after  { content: ' \2191'; opacity: 1; }
.sortable th.sort-desc::after { content: ' \2193'; opacity: 1; }
.sortable th:hover { color: #bdc3c7; }
"#;

// ── Scripts ──────────────────────────────────────────────────────────────────

const JS: &str = r#"
(function () {
  // ── Tab switching ──────────────────────────────────────────────────────────
  document.querySelectorAll('.tab-btn').forEach(function (btn) {
    btn.addEventListener('click', function () {
      document.querySelectorAll('.tab-btn').forEach(function (b) {
        b.classList.remove('tab-active');
      });
      document.querySelectorAll('.tab-panel').forEach(function (p) {
        p.classList.add('tab-panel-hidden');
      });
      btn.classList.add('tab-active');
      document.getElementById('tab-' + btn.dataset.tab).classList.remove('tab-panel-hidden');
    });
  });

  // ── Sortable stats table ───────────────────────────────────────────────────
  var table = document.getElementById('stats-table');
  if (!table) return;
  var tbody = table.tBodies[0];
  var headers = table.tHead.rows[0].cells;
  var sortCol = 0, sortAsc = true;

  function cellVal(row, col, type) {
    var text = row.cells[col].textContent.trim();
    return type === 'num' ? (parseFloat(text) || 0) : text.toLowerCase();
  }

  function sort(col, type) {
    var rows = Array.from(tbody.rows);
    var asc = (col === sortCol) ? !sortAsc : (type === 'num' ? false : true);
    rows.sort(function (a, b) {
      var av = cellVal(a, col, type), bv = cellVal(b, col, type);
      if (av < bv) return asc ? -1 : 1;
      if (av > bv) return asc ? 1 : -1;
      return 0;
    });
    rows.forEach(function (r) { tbody.appendChild(r); });
    Array.from(headers).forEach(function (th) {
      th.classList.remove('sort-asc', 'sort-desc');
    });
    headers[col].classList.add(asc ? 'sort-asc' : 'sort-desc');
    sortCol = col; sortAsc = asc;
  }

  Array.from(headers).forEach(function (th) {
    th.addEventListener('click', function () {
      sort(+th.dataset.col, th.dataset.type);
    });
  });
}());
"#;
