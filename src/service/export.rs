//! The career as one self-contained HTML file — every championship with its standings and
//! rounds, the driver statistics and the track records — for keeping or sharing outside the app.
//!
//! Built here rather than in the browser, which used to assemble it: there the lap charts were
//! fetched lazily, so they came out empty. The styling is inline and light, because the file is
//! opened on its own, long after the app that made it may have gone.

use std::fmt::Write as _;

use super::App;
use crate::career::data_store::{
    ChampionshipStatus, ChampionshipView, DriverStat, LapChartEntry, SessionView, StandingsEntry,
    TrackStat, SESSION_PRACTICE, SESSION_RACE,
};
use crate::format::{self, DASH};

const CSS: &str = concat!(
    "body{font-family:-apple-system,BlinkMacSystemFont,\"Segoe UI\",sans-serif;max-width:1200px;margin:0 auto;padding:1.5rem 2rem;color:#111;font-size:14px}",
    "h1{font-size:1.5rem;border-bottom:2px solid #ddd;padding-bottom:0.4rem;margin-bottom:0.3rem}",
    "h2{font-size:1.15rem;margin:2rem 0 0.8rem;border-bottom:1px solid #eee;padding-bottom:0.2rem}",
    "h3{font-size:0.95rem;margin:0.6rem 0 0.3rem;color:#333}",
    ".export-date{color:#888;font-size:0.82rem;margin:0 0 1rem}",
    ".champ-block{border:1px solid #ddd;border-radius:6px;margin-bottom:1.5rem;padding:1rem}",
    ".champ-title{display:flex;align-items:center;gap:0.6rem;margin-bottom:0.75rem}",
    ".champ-title h2{margin:0;border:none;padding:0;font-size:1.05rem}",
    ".badge{font-size:0.7rem;font-weight:700;padding:1px 5px;border-radius:3px;text-transform:uppercase;letter-spacing:0.04em;border:1px solid}",
    ".badge-active{border-color:#c0392b;color:#c0392b;background:#fdf0ef}",
    ".badge-progress{border-color:#27ae60;color:#27ae60;background:#edfaf1}",
    ".badge-final{border-color:#2980b9;color:#2980b9;background:#eaf4fb}",
    ".badge-dnf{border-color:#e67e22;color:#e67e22;background:#fef5ec}",
    ".champ-body{display:flex;flex-wrap:wrap;gap:1rem;align-items:flex-start}",
    ".standings-panel{min-width:200px;flex:0 0 auto}",
    ".results-panel{flex:2;min-width:300px}",
    "table{border-collapse:collapse;width:100%;margin-bottom:0.5rem;font-size:0.83rem}",
    "th,td{border:1px solid #ddd;padding:4px 8px;text-align:left}",
    "th{background:#f5f5f5;font-weight:600;color:#333}",
    ".pos,.pts,.stat-num{text-align:center}",
    ".stat-name{font-weight:600}",
    ".stat-group-start{border-left:2px solid #bbb}",
    ".result-car{color:#777;font-size:0.78rem}",
    ".manage-empty{color:#888;font-style:italic}",
    "details{margin-bottom:0.4rem}",
    "summary{font-weight:600;font-size:0.88rem;cursor:pointer;padding:0.2rem 0}",
    ".events-grid{margin-top:0.5rem}",
    ".round-session{margin-bottom:1rem}",
    ".round-session-label{font-size:0.79rem;color:#555;margin-bottom:0.3rem}",
    ".grid-note{font-size:0.78rem;color:#9a6700;margin-bottom:0.3rem}",
    ".session-type-badge{font-size:0.68rem;font-weight:700;border:1px solid #bbb;border-radius:2px;padding:0 3px}",
    ".session-track{font-weight:600}",
    ".session-date,.session-drivers{color:#888}",
    ".session-track-var{color:#777;font-size:0.85em}",
    ".lap-chart-label{font-size:0.79rem;font-weight:600;color:#555;margin:0.5rem 0 0.2rem}",
    ".lap-chart-scroll{overflow-x:auto}",
    ".lap-chart-table th,.lap-chart-table td{padding:2px 4px;font-size:0.72rem;text-align:center;min-width:22px}",
    ".lc-driver{text-align:left;white-space:nowrap;min-width:100px}",
    ".lap-p1{background:#8e44ad;color:#fff;font-weight:700}",
    ".lap-p2{background:#2980b9;color:#fff}",
    ".lap-p3{background:#16a085;color:#fff}",
    ".lap-top{background:#eaf6e8}",
    ".track-lap-driver{color:#444}",
    ".track-lap-cell{white-space:nowrap}",
);

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

fn status_badge(status: &ChampionshipStatus) -> String {
    let (class, label) = match status {
        ChampionshipStatus::Active => ("badge-active", "Active"),
        ChampionshipStatus::Progress => ("badge-progress", "Progress"),
        ChampionshipStatus::Final => ("badge-final", "Final"),
    };
    format!("<span class=\"badge {class}\">{label}</span>")
}

fn session_type_name(t: u32) -> (&'static str, &'static str) {
    match t {
        1 => ("P", "Practice"),
        3 => ("Q", "Qualify"),
        5 => ("R", "Race"),
        _ => ("?", "Session"),
    }
}

impl App {
    /// The file name an export is offered under: `ams2_career_<today>.html`.
    pub fn career_export_file_name(&self) -> String {
        format!("ams2_career_{}.html", format::today())
    }

    /// The whole career as one HTML document — see the module documentation.
    pub fn career_export_html(&self) -> String {
        let career = self.career_view();
        let mut h = String::new();
        let _ = write!(
            h,
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n\
             <title>AMS2 Career Championships</title>\n<style>{CSS}</style>\n</head>\n<body>\n\
             <h1>AMS2 Career Championships</h1>\n<p class=\"export-date\">Exported {}</p>\n",
            format::today()
        );
        h.push_str("<h2>Championships</h2>\n");
        if career.championships.is_empty() {
            h.push_str("<p class=\"manage-empty\">No championships yet.</p>\n");
        }
        for champ in &career.championships {
            self.champ_block(&mut h, champ);
        }
        h.push_str("<h2>Driver Statistics</h2>\n");
        driver_stats(&mut h, &career.driver_stats);
        h.push_str("<h2>Track Statistics</h2>\n");
        track_stats(&mut h, &career.track_stats_all);
        h.push_str("\n</body>\n</html>\n");
        h
    }

    fn champ_block(&self, h: &mut String, champ: &ChampionshipView) {
        let _ = write!(
            h,
            "<div class=\"champ-block\"><div class=\"champ-title\"><h2>{}</h2>{}</div>\
             <div class=\"champ-body\">",
            esc(&champ.name),
            status_badge(&champ.status)
        );
        h.push_str("<div class=\"standings-panel\"><h3>Driver Standings</h3>");
        standings(h, &champ.driver_standings, "Driver / Team");
        h.push_str("</div>");
        if !champ.constructor_standings.is_empty() {
            h.push_str("<div class=\"standings-panel\"><h3>Constructor Standings</h3>");
            standings(h, &champ.constructor_standings, "Car / Team");
            h.push_str("</div>");
        }
        h.push_str("<div class=\"results-panel\"><h3>Rounds</h3>");
        let mut any = false;
        for (i, round) in champ.rounds.iter().enumerate() {
            // Practice is never shown in a season's history: nothing is scored on it.
            let sessions: Vec<&SessionView> = round
                .sessions
                .iter()
                .filter(|s| s.session_type != SESSION_PRACTICE)
                .collect();
            if sessions.is_empty() {
                continue;
            }
            any = true;
            let race = sessions.iter().find(|s| s.session_type == SESSION_RACE);
            let track = race.or(sessions.first()).map(|s| s.track.as_str()).unwrap_or("Unknown");
            let winner = race
                .and_then(|s| s.winner.as_deref())
                .map(|w| format!(" &nbsp;&#127942; {}", esc(w)))
                .unwrap_or_default();
            let _ = write!(
                h,
                "<details open><summary>R{} {DASH} {}{winner}</summary><div class=\"events-grid\">",
                i + 1,
                esc(track)
            );
            for s in sessions {
                self.session(h, s);
            }
            h.push_str("</div></details>");
        }
        if !any {
            h.push_str("<p class=\"manage-empty\">No rounds assigned yet.</p>");
        }
        h.push_str("</div></div></div>\n");
    }

    fn session(&self, h: &mut String, s: &SessionView) {
        let (badge, name) = session_type_name(s.session_type);
        let is_race = s.session_type == SESSION_RACE;
        let _ = write!(
            h,
            "<div class=\"round-session\"><div class=\"round-session-label\">\
             <span class=\"session-type-badge\">{badge}</span> {name} \
             <span class=\"session-track\">{}</span> <span class=\"session-date\">{}</span> \
             <span class=\"session-drivers\">{} drivers</span></div>",
            esc(&format::track(&s.track, &s.track_variation)),
            format::date_time(s.recorded_at),
            s.results.len()
        );
        if let Some(note) = &s.grid_note {
            let _ = write!(h, "<div class=\"grid-note\">&#9888; {}</div>", esc(note));
        }
        let mut results: Vec<_> = s.results.iter().collect();
        results.sort_by_key(|r| r.race_position);
        h.push_str("<table><thead><tr><th>Pos</th><th>Driver</th>");
        if is_race {
            h.push_str("<th>Pts</th>");
        }
        h.push_str("<th>Laps</th><th>Best</th></tr></thead><tbody>");
        for (i, r) in results.iter().enumerate() {
            let pos = if r.race_position > 0 { r.race_position as usize } else { i + 1 };
            let car = if r.car_name.is_empty() {
                String::new()
            } else {
                format!(" <span class=\"result-car\">{}</span>", esc(&r.car_name))
            };
            let dnf = if is_race && r.dnf {
                " <span class=\"badge badge-dnf\">DNF</span>"
            } else {
                ""
            };
            let _ = write!(h, "<tr><td class=\"pos\">{pos}</td><td>{}{car}{dnf}</td>", esc(&r.name));
            if is_race {
                let _ = write!(h, "<td class=\"pts\">{}</td>", r.points_earned);
            }
            let _ = write!(
                h,
                "<td class=\"pts\">{}</td><td>{}</td></tr>",
                r.laps_completed,
                format::lap_time(r.fastest_lap)
            );
        }
        h.push_str("</tbody></table>");
        if is_race {
            let chart = self.lap_chart(&s.id);
            if !chart.is_empty() {
                h.push_str("<div class=\"lap-chart-label\">Lap Chart</div>");
                lap_chart(h, s, &chart);
            }
        }
        h.push_str("</div>");
    }
}

fn standings(h: &mut String, rows: &[StandingsEntry], heading: &str) {
    if rows.is_empty() {
        h.push_str("<p class=\"manage-empty\">No results yet.</p>");
        return;
    }
    let _ = write!(
        h,
        "<table><thead><tr><th>Pos</th><th>{heading}</th><th>Pts</th><th>W</th></tr></thead><tbody>"
    );
    for (i, d) in rows.iter().enumerate() {
        let team = d
            .team
            .as_deref()
            .map(|t| format!(" <span class=\"result-car\">{}</span>", esc(t)))
            .unwrap_or_default();
        let _ = write!(
            h,
            "<tr><td class=\"pos\">{}</td><td>{}{team}</td><td class=\"pts\">{}</td><td class=\"pts\">{}</td></tr>",
            i + 1,
            esc(&d.name),
            d.points,
            d.wins
        );
    }
    h.push_str("</tbody></table>");
}

/// Lap by lap, one row per driver in finishing order — the same table the Career tab draws.
fn lap_chart(h: &mut String, s: &SessionView, chart: &[LapChartEntry]) {
    let mut laps: Vec<u32> = chart.iter().map(|e| e.lap).collect();
    laps.sort_unstable();
    laps.dedup();
    let mut drivers: Vec<_> = s.results.iter().collect();
    drivers.sort_by_key(|r| r.race_position);
    let total = drivers.len() as u32;
    h.push_str("<div class=\"lap-chart-scroll\"><table class=\"lap-chart-table\"><thead><tr><th class=\"lc-driver\">Driver</th>");
    for l in &laps {
        let _ = write!(h, "<th>{l}</th>");
    }
    h.push_str("</tr></thead><tbody>");
    for d in drivers {
        let _ = write!(h, "<tr><td class=\"lc-driver\">{}</td>", esc(&d.name));
        for l in &laps {
            match chart.iter().find(|e| e.lap == *l && e.driver == d.name) {
                Some(e) => {
                    let class = match e.position {
                        1 => " class=\"lap-p1\"",
                        2 => " class=\"lap-p2\"",
                        3 => " class=\"lap-p3\"",
                        p if p <= total.div_ceil(2) => " class=\"lap-top\"",
                        _ => "",
                    };
                    let _ = write!(h, "<td{class}>{}</td>", e.position);
                }
                None => h.push_str("<td></td>"),
            }
        }
        h.push_str("</tr>");
    }
    h.push_str("</tbody></table></div>");
}

fn driver_stats(h: &mut String, rows: &[DriverStat]) {
    if rows.is_empty() {
        h.push_str("<p class=\"manage-empty\">No data.</p>\n");
        return;
    }
    h.push_str(
        "<table><thead><tr><th class=\"stat-name\">Driver</th><th class=\"stat-num\">Races</th>\
         <th class=\"stat-num\">1st</th><th class=\"stat-num\">2nd</th><th class=\"stat-num\">3rd</th>\
         <th class=\"stat-num\">Top 10</th><th class=\"stat-num\">Avg Pos</th><th class=\"stat-num\">DNF</th>\
         <th class=\"stat-num stat-group-start\">Q Pole</th><th class=\"stat-num\">Q 2nd</th>\
         <th class=\"stat-num\">Q 3rd</th><th class=\"stat-num\">Q Top 10</th>\
         <th class=\"stat-num stat-group-start\">C 1st</th><th class=\"stat-num\">C 2nd</th>\
         <th class=\"stat-num\">C 3rd</th></tr></thead><tbody>",
    );
    for d in rows {
        let avg = if d.races > 0 { format!("{:.1}", d.avg_pos) } else { DASH.into() };
        let _ = write!(
            h,
            "<tr><td class=\"stat-name\">{}</td><td class=\"stat-num\">{}</td><td class=\"stat-num\">{}</td>\
             <td class=\"stat-num\">{}</td><td class=\"stat-num\">{}</td><td class=\"stat-num\">{}</td>\
             <td class=\"stat-num\">{avg}</td><td class=\"stat-num\">{}</td>\
             <td class=\"stat-num stat-group-start\">{}</td><td class=\"stat-num\">{}</td>\
             <td class=\"stat-num\">{}</td><td class=\"stat-num\">{}</td>\
             <td class=\"stat-num stat-group-start\">{}</td><td class=\"stat-num\">{}</td>\
             <td class=\"stat-num\">{}</td></tr>",
            esc(&d.name),
            d.races,
            d.p1,
            d.p2,
            d.p3,
            d.top10,
            d.dnf,
            d.quali_p1,
            d.quali_p2,
            d.quali_p3,
            d.quali_top10,
            d.champ_wins,
            d.champ_p2,
            d.champ_p3
        );
    }
    h.push_str("</tbody></table>\n");
}

fn lap_holder(time: f32, driver: &str, car: &str) -> String {
    if time <= 0.0 {
        return DASH.into();
    }
    let mut out = format::lap_time(time);
    if !driver.is_empty() {
        let car = if car.is_empty() {
            String::new()
        } else {
            format!(" <span class=\"session-track-var\">({})</span>", esc(car))
        };
        let _ = write!(out, " <span class=\"track-lap-driver\">{}{car}</span>", esc(driver));
    }
    out
}

fn track_stats(h: &mut String, rows: &[TrackStat]) {
    if rows.is_empty() {
        h.push_str("<p class=\"manage-empty\">No data.</p>\n");
        return;
    }
    h.push_str(
        "<table><thead><tr><th class=\"stat-name\">Track</th><th class=\"stat-num\">Races</th>\
         <th class=\"stat-num\">Qualifyings</th><th class=\"stat-num\">Best Lap</th>\
         <th class=\"stat-num\">2nd Lap</th><th class=\"stat-num\">3rd Lap</th>\
         <th class=\"stat-num\">Last Visited</th></tr></thead><tbody>",
    );
    for t in rows {
        let name = if !t.track_variation.is_empty() && t.track_variation != t.track {
            format!(
                "{} <span class=\"session-track-var\">({})</span>",
                esc(&t.track),
                esc(&t.track_variation)
            )
        } else {
            esc(&t.track)
        };
        let _ = write!(
            h,
            "<tr><td class=\"stat-name\">{name}</td><td class=\"stat-num\">{}</td><td class=\"stat-num\">{}</td>\
             <td class=\"stat-num track-lap-cell\">{}</td><td class=\"stat-num track-lap-cell\">{}</td>\
             <td class=\"stat-num track-lap-cell\">{}</td><td class=\"stat-num\">{}</td></tr>",
            t.races,
            t.qualifyings,
            lap_holder(t.best_lap, &t.best_lap_driver, &t.best_lap_car),
            lap_holder(t.second_lap, &t.second_lap_driver, &t.second_lap_car),
            lap_holder(t.third_lap, &t.third_lap_driver, &t.third_lap_car),
            format::date_time(t.last_visited)
        );
    }
    h.push_str("</tbody></table>\n");
}
