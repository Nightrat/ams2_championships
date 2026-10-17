// ── Career Championships tab ──────────────────────────────────────────────────

function careerConstructorsHtml(constructors) {
  if (!constructors.length) return '<p class="manage-empty">No results yet.</p>';
  var rows = constructors.map(function (d, i) {
    return '<tr>' +
      '<td class="pos">' + (i + 1) + '</td>' +
      '<td>' + esc(d.name) + '</td>' +
      '<td class="pts">' + d.points + '</td>' +
      '<td class="pts">' + d.wins + '</td>' +
      '</tr>';
  }).join('');
  return '<table class="standings-table">' +
    '<thead><tr><th>Pos</th><th>Car / Team</th><th>Pts</th><th>W</th></tr></thead>' +
    '<tbody>' + rows + '</tbody></table>';
}

function careerStandingsHtml(standings) {
  if (!standings.length) return '<p class="manage-empty">No results yet.</p>';
  var rows = standings.map(function (d, i) {
    // Beside the name rather than in a column of its own: the standings panel is
    // narrow and this is the same shape a session result row already uses for the
    // very same value. `team` is absent when nothing named it, so no empty span.
    var team = d.team ? ' <span class="result-car">' + esc(d.team) + '</span>' : '';
    return '<tr>' +
      '<td class="pos">' + (i + 1) + '</td>' +
      '<td>' + esc(d.name) + team + '</td>' +
      '<td class="pts">' + d.points + '</td>' +
      '<td class="pts">' + d.wins + '</td>' +
      '</tr>';
  }).join('');
  return '<table class="standings-table">' +
    '<thead><tr><th>Pos</th><th>Driver / Team</th><th>Pts</th><th>W</th></tr></thead>' +
    '<tbody>' + rows + '</tbody></table>';
}

// A race session's chart, collapsed and not yet fetched. The chart is the bulk of a career and
// is read one session at a time, so it is no longer part of /api/career — opening this is what
// asks for it. `lapChartTableHtml` then draws it from the reply.
// Sessions by id, so a chart arriving later can order its drivers by their final position.
// Only the race sessions that have a chart section go in — this is a lookup, not a second copy
// of the career.
var _lapChartSessions = {};

function lapChartHtml(s) {
  _lapChartSessions[s.id] = s;
  return '<details class="lap-chart-wrap" data-session="' + esc(s.id) + '">' +
    '<summary class="lap-chart-label">Lap Chart</summary>' +
    '<div class="lap-chart-body"></div>' +
    '</details>';
}

function lapChartTableHtml(s, chart) {
  if (!chart.length) return '<div class="lap-chart-empty">No lap chart recorded.</div>';

  // Collect laps and build byDriverLap[driver][lap] = position
  var laps = [];
  var byDriverLap = {};
  chart.forEach(function (e) {
    if (laps.indexOf(e.lap) === -1) laps.push(e.lap);
    if (!byDriverLap[e.driver]) byDriverLap[e.driver] = {};
    byDriverLap[e.driver][e.lap] = e.position;
  });
  laps.sort(function (a, b) { return a - b; });

  // Order drivers by their final race position
  var drivers = s.results.slice()
    .sort(function (a, b) { return a.race_position - b.race_position; })
    .map(function (r) { return r.name; });

  function posClass(pos, total) {
    if (pos === 1) return ' lap-p1';
    if (pos === 2) return ' lap-p2';
    if (pos === 3) return ' lap-p3';
    if (pos <= Math.ceil(total / 2)) return ' lap-top';
    return '';
  }

  var n = drivers.length;
  var lapHeaders = laps.map(function (l) { return '<th class="lc-lap">' + l + '</th>'; }).join('');
  var bodyRows = drivers.map(function (d) {
    var cells = laps.map(function (l) {
      var pos = byDriverLap[d] && byDriverLap[d][l];
      if (!pos) return '<td class="lc-cell"></td>';
      return '<td class="lc-cell' + posClass(pos, n) + '">' + pos + '</td>';
    }).join('');
    return '<tr><td class="lc-driver">' + esc(d) + '</td>' + cells + '</tr>';
  }).join('');

  return '<div class="lap-chart-scroll">' +
    '<table class="lap-chart-table">' +
    '<thead><tr><th class="lc-driver-h">Driver</th>' + lapHeaders + '</tr></thead>' +
    '<tbody>' + bodyRows + '</tbody>' +
    '</table></div>';
}

// Fetch a chart the first time its section is opened, and once only. Delegated from the
// container because the career view rebuilds its HTML wholesale on every reload.
document.addEventListener('toggle', function (e) {
  var box = e.target;
  if (!box.classList || !box.classList.contains('lap-chart-wrap')) return;
  if (!box.open || box.dataset.loaded) return;
  box.dataset.loaded = '1';
  var body = box.querySelector('.lap-chart-body');
  var id = box.dataset.session;
  body.textContent = 'Loading…';
  fetch('/api/sessions/' + encodeURIComponent(id) + '/lap-chart')
    .then(function (r) { return r.json(); })
    .then(function (chart) {
      var s = _lapChartSessions[id];
      body.innerHTML = s ? lapChartTableHtml(s, chart || []) : '';
    })
    .catch(function () {
      box.dataset.loaded = '';
      body.textContent = 'Could not load the lap chart.';
    });
}, true);

function careerRoundsHtml(champ) {
  var rounds = champ.rounds || [];
  if (!rounds.length) return '<p class="manage-empty">No rounds assigned yet.</p>';

  var rows = rounds.map(function (round, rIdx) {
    var roundSessions = (round.sessions || []).filter(function (s) { return s.session_type !== 1; });
    if (!roundSessions.length) return '';

    var raceSess = roundSessions.find(function (s) { return s.session_type === 5; });
    var trackName = raceSess
      ? raceSess.track
      : (roundSessions[0] ? roundSessions[0].track : 'Unknown');
    var winner = raceSess ? raceSess.winner : null;

    var sessionsHtml = roundSessions.map(function (s) {
      var typeLabel = SESSION_TYPE_LABELS[s.session_type] || '?';
      var typeName  = SESSION_TYPE_NAMES[s.session_type]  || 'Session';
      var isRace    = s.session_type === 5;

      var sorted = s.results.slice().sort(function (a, b) { return a.race_position - b.race_position; });
      var resultRows = sorted.map(function (r, idx) {
        var pos = r.race_position > 0 ? r.race_position : (idx + 1);
        var fl  = r.fastest_lap > 0 ? fmtLapTime(r.fastest_lap) : '\u2014';
        var dnf = (isRace && r.dnf) ? ' <span class="badge badge-pending">DNF</span>' : '';
        var pts = isRace ? '<td class="pts">' + (r.points_earned || 0) + '</td>' : '';
        var carLabel = r.car_name
          ? ' <span class="result-car">' + esc(r.car_name) + '</span>'
          : '';
        return '<tr><td class="pos">' + pos + '</td>' +
          '<td>' + esc(r.name) + carLabel + dnf + '</td>' +
          pts +
          '<td class="pts">' + (r.laps_completed || 0) + '</td>' +
          '<td class="car">' + fl + '</td></tr>';
      }).join('');

      var ptsHeader = isRace ? '<th>Pts</th>' : '';
      // A season's history should show which races were run on the grid they are judged
      // against — the badge marks it, the sentence underneath is the server's own wording.
      var gridBadge = s.grid_note
        ? ' <span class="session-grid-badge" title="' + esc(s.grid_note) + '">&#9888; grid</span>'
        : '';
      var gridNote = s.grid_note
        ? '<div class="round-session-grid-note">' + esc(s.grid_note) + '</div>'
        : '';
      return '<div class="round-session' + (s.grid_note ? ' round-session-flagged' : '') + '">' +
        '<div class="round-session-label"><span class="session-type-badge">' + typeLabel + '</span> ' + typeName +
          ' <span class="session-track">' + fmtTrack(s) + '</span>' +
          ' <span class="session-date">' + fmtDate(s.recorded_at) + '</span>' +
          ' <span class="session-drivers">' + s.results.length + ' drivers</span>' +
          gridBadge +
        '</div>' +
        gridNote +
        '<table class="standings-table"><thead><tr><th>Pos</th><th>Driver</th>' + ptsHeader + '<th>Laps</th><th>Best</th></tr></thead>' +
        '<tbody>' + resultRows + '</tbody></table>' +
        (isRace ? lapChartHtml(s) : '') +
        '</div>';
    }).join('');

    return '<details class="events-detail">' +
      '<summary>R' + (rIdx + 1) + ' \u2014 ' + esc(trackName) +
        (winner ? ' &nbsp;&#127942; ' + esc(winner) : '') +
      '</summary>' +
      '<div class="events-grid">' + sessionsHtml + '</div>' +
      '</details>';
  }).filter(Boolean).join('');

  return rows
    ? '<div class="champ-sessions-list">' + rows + '</div>'
    : '<p class="manage-empty">No rounds assigned yet.</p>';
}

var careerChamps = [];
var selectedChampIdx = -1;

function champDetailHtml(champ) {
  var badgeCls = champ.status === 'Final'    ? 'badge-final' :
                 champ.status === 'Progress' ? 'badge-progress' : 'badge-active';
  return '<div class="champ-detail-header">' +
      '<h2>' + esc(champ.name) + '</h2>' +
      '<span class="badge ' + badgeCls + '">' + esc(champ.status) + '</span>' +
    '</div>' +
    '<div class="champ-body">' +
      '<div class="standings-panel"><h3>Driver Standings</h3>' + careerStandingsHtml(champ.driver_standings) + '</div>' +
      (champ.constructor_standings.length ? '<div class="standings-panel"><h3>Constructor Standings</h3>' + careerConstructorsHtml(champ.constructor_standings) + '</div>' : '') +
      '<div class="results-panel"><h3>Rounds</h3>' + careerRoundsHtml(champ) + '</div>' +
    '</div>';
}

function selectChamp(idx) {
  selectedChampIdx = idx;
  var list = document.getElementById('career-champ-list');
  if (list) list.querySelectorAll('.champ-list-item').forEach(function (el) {
    el.classList.toggle('champ-list-item-active', +el.dataset.idx === idx);
  });
  var detail = document.getElementById('career-champ-detail');
  if (!detail) return;
  if (idx < 0 || idx >= careerChamps.length) {
    detail.innerHTML = '<div class="champ-detail-empty">Select a championship.</div>';
  } else {
    detail.innerHTML = champDetailHtml(careerChamps[idx]);
  }
}

function renderCareerChampionships(champs) {
  careerChamps = champs || []; // already in display order
  var list = document.getElementById('career-champ-list');
  var detail = document.getElementById('career-champ-detail');
  if (!list || !detail) return;
  if (!careerChamps.length) {
    list.innerHTML = '<div class="manage-placeholder" style="padding:1.5rem">No championships yet.</div>';
    detail.innerHTML = '';
    return;
  }
  list.innerHTML = careerChamps.map(function (champ, idx) {
    var badgeCls = champ.status === 'Final'    ? 'badge-final' :
                   champ.status === 'Progress' ? 'badge-progress' : 'badge-active';
    return '<div class="champ-list-item" data-idx="' + idx + '">' +
      '<div class="champ-list-name">' + esc(champ.name) + '</div>' +
      '<div class="champ-list-meta">' +
        '<span class="badge ' + badgeCls + '">' + esc(champ.status) + '</span>' +
        '<span class="champ-list-rounds">' + (champ.rounds || []).length + ' rounds</span>' +
      '</div>' +
    '</div>';
  }).join('');
  list.querySelectorAll('.champ-list-item').forEach(function (el) {
    el.addEventListener('click', function () { selectChamp(+el.dataset.idx); });
  });
  // Auto-select Active championship, fall back to first
  var activeIdx = careerChamps.findIndex(function (c) { return c.status === 'Active'; });
  selectChamp(activeIdx >= 0 ? activeIdx : 0);
}

function renderCareerStats(driverStats) {
  careerDriverStats = driverStats || [];
  var container = document.getElementById('career-stats-container');
  if (!container) return;
  var rows = (careerDriverStats).map(function (d) {
    return { name: d.name, races: d.races, p1: d.p1, p2: d.p2, p3: d.p3, top10: d.top10,
             dnf: d.dnf, qualiP1: d.quali_p1, qualiP2: d.quali_p2, qualiP3: d.quali_p3, qualiTop10: d.quali_top10,
             champWins: d.champ_wins, champP2: d.champ_p2, champP3: d.champ_p3,
             avgPos: d.races ? d.avg_pos.toFixed(1) : '\u2014' };
  });
  var thead = '<tr>' +
    '<th class="stat-name sort-asc" data-col="0" data-type="str">Driver</th>' +
    '<th class="stat-num" data-col="1" data-type="num">Races</th>' +
    '<th class="stat-num" data-col="2" data-type="num">1st</th>' +
    '<th class="stat-num" data-col="3" data-type="num">2nd</th>' +
    '<th class="stat-num" data-col="4" data-type="num">3rd</th>' +
    '<th class="stat-num" data-col="5" data-type="num">Top 10</th>' +
    '<th class="stat-num" data-col="6" data-type="num">Avg Pos</th>' +
    '<th class="stat-num" data-col="7" data-type="num">DNF</th>' +
    '<th class="stat-num stat-group-start" data-col="8" data-type="num">Q Pole</th>' +
    '<th class="stat-num" data-col="9" data-type="num">Q 2nd</th>' +
    '<th class="stat-num" data-col="10" data-type="num">Q 3rd</th>' +
    '<th class="stat-num" data-col="11" data-type="num">Q Top 10</th>' +
    '<th class="stat-num stat-group-start" data-col="12" data-type="num">C 1st</th>' +
    '<th class="stat-num" data-col="13" data-type="num">C 2nd</th>' +
    '<th class="stat-num" data-col="14" data-type="num">C 3rd</th>' +
    '</tr>';
  var tbody = rows.map(function (r) {
    return '<tr><td class="stat-name">' + esc(r.name) + '</td>' +
      '<td class="stat-num">' + r.races + '</td>' +
      '<td class="stat-num">' + r.p1 + '</td>' +
      '<td class="stat-num">' + r.p2 + '</td>' +
      '<td class="stat-num">' + r.p3 + '</td>' +
      '<td class="stat-num">' + r.top10 + '</td>' +
      '<td class="stat-num">' + r.avgPos + '</td>' +
      '<td class="stat-num">' + (r.dnf || 0) + '</td>' +
      '<td class="stat-num stat-group-start">' + (r.qualiP1 || 0) + '</td>' +
      '<td class="stat-num">' + (r.qualiP2 || 0) + '</td>' +
      '<td class="stat-num">' + (r.qualiP3 || 0) + '</td>' +
      '<td class="stat-num">' + (r.qualiTop10 || 0) + '</td>' +
      '<td class="stat-num stat-group-start">' + (r.champWins || 0) + '</td>' +
      '<td class="stat-num">' + (r.champP2 || 0) + '</td>' +
      '<td class="stat-num">' + (r.champP3 || 0) + '</td></tr>';
  }).join('');
  container.innerHTML = '<table class="stats-table sortable" id="career-stats-table">' +
    '<thead>' + thead + '</thead><tbody>' + tbody + '</tbody></table>';
  initSortableTableEl(document.getElementById('career-stats-table'));
}

// Per (track, layout, car), and the same records merged across every car — both from the
// server (`track_stats` / `track_stats_all`), most recently visited first.
var allTrackStats = [];
var allCarsTrackStats = [];
var careerDriverStats = [];

function buildTrackCarFilter(trackStats) {
  var cars = [];
  trackStats.forEach(function (t) { if (t.car && cars.indexOf(t.car) === -1) cars.push(t.car); });
  cars.sort();
  return cars;
}

function renderTrackStats(trackStats, allCars) {
  allTrackStats = trackStats || [];
  allCarsTrackStats = allCars || [];
  var container = document.getElementById('career-tracks-container');
  if (!container) return;
  if (!allTrackStats.length) {
    container.innerHTML = '<div class="manage-placeholder" style="padding:2rem">No sessions recorded yet.</div>';
    return;
  }

  var cars = buildTrackCarFilter(allTrackStats);
  var filterHtml = '<div class="track-filter-bar">' +
    '<label class="track-filter-label">Car</label>' +
    '<select id="track-car-filter" class="track-car-filter">' +
    '<option value="">All Cars</option>' +
    cars.map(function (c) { return '<option value="' + esc(c) + '">' + esc(c) + '</option>'; }).join('') +
    '</select></div>';

  container.innerHTML = filterHtml + '<div id="career-tracks-table-wrap"></div>';
  document.getElementById('track-car-filter').addEventListener('change', function () {
    applyTrackCarFilter(this.value);
  });
  applyTrackCarFilter('');
}

function fmtLapHolder(time, driver, car, showCar) {
  if (!time || time <= 0) return '\u2014';
  var t = fmtLapTime(time);
  var d = driver ? ' <span class="track-lap-driver">' + esc(driver) + (showCar && car ? ' <span class="session-track-var">(' + esc(car) + ')</span>' : '') + '</span>' : '';
  return t + d;
}

function applyTrackCarFilter(car) {
  var rows = car ? allTrackStats.filter(function (t) { return t.car === car; }) : allCarsTrackStats;
  var showCar = !car;
  var col = 0;
  function th(cls, type, label) { return '<th class="' + cls + '" data-col="' + (col++) + '" data-type="' + type + '">' + label + '</th>'; }
  var thead = '<tr>' +
    th('stat-name sort-asc', 'str', 'Track') +
    th('stat-num', 'num', 'Races') +
    th('stat-num', 'num', 'Qualifyings') +
    th('stat-num', 'time', 'Best Lap') +
    th('stat-num', 'time', '2nd Lap') +
    th('stat-num', 'time', '3rd Lap') +
    th('stat-num', 'str', 'Last Visited') +
    '</tr>';
  var tbody = rows.map(function (t) {
    var name = t.track_variation && t.track_variation !== t.track
      ? esc(t.track) + ' <span class="session-track-var">' + esc(t.track_variation) + '</span>'
      : esc(t.track);
    return '<tr>' +
      '<td class="stat-name">' + name + '</td>' +
      '<td class="stat-num">' + t.races + '</td>' +
      '<td class="stat-num">' + t.qualifyings + '</td>' +
      '<td class="stat-num track-lap-cell">' + fmtLapHolder(t.best_lap,   t.best_lap_driver,   t.best_lap_car,   showCar) + '</td>' +
      '<td class="stat-num track-lap-cell">' + fmtLapHolder(t.second_lap, t.second_lap_driver, t.second_lap_car, showCar) + '</td>' +
      '<td class="stat-num track-lap-cell">' + fmtLapHolder(t.third_lap,  t.third_lap_driver,  t.third_lap_car,  showCar) + '</td>' +
      '<td class="stat-num">' + fmtDate(t.last_visited) + '</td>' +
    '</tr>';
  }).join('');
  var wrap = document.getElementById('career-tracks-table-wrap');
  if (!wrap) return;
  wrap.innerHTML = '<table class="stats-table sortable" id="career-tracks-table">' +
    '<thead>' + thead + '</thead><tbody>' + tbody + '</tbody></table>';
  initSortableTableEl(document.getElementById('career-tracks-table'));
}

function loadCareerChampionships() {
  fetch('/api/career').then(function (r) { return r.json(); })
    .then(function (career) {
      renderCareerChampionships(career.championships || []);
      renderCareerStats(career.driver_stats || []);
      renderTrackStats(career.track_stats || [], career.track_stats_all || []);
    }).catch(function () {
      var el = document.getElementById('career-champ-detail');
      if (el) el.innerHTML = '<div class="manage-placeholder" style="padding:2rem">Career data requires the server binary.</div>';
    });
}

document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () {
    if (btn.dataset.tab === 'career') loadCareerChampionships();
  });
});

// ── Champ list resize handle ──────────────────────────────────────────────────
(function () {
  var handle = document.getElementById('career-champ-list-resize');
  var list   = document.getElementById('career-champ-list');
  if (!handle || !list) return;
  var dragging = false, startX, startW;
  handle.addEventListener('mousedown', function (e) {
    dragging = true;
    startX = e.clientX;
    startW = list.offsetWidth;
    handle.classList.add('dragging');
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  });
  document.addEventListener('mousemove', function (e) {
    if (!dragging) return;
    var w = Math.min(480, Math.max(100, startW + (e.clientX - startX)));
    list.style.width = w + 'px';
  });
  document.addEventListener('mouseup', function () {
    if (!dragging) return;
    dragging = false;
    handle.classList.remove('dragging');
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  });
}());

// ── HTML export ───────────────────────────────────────────────────────────────
// The document is built by the server (`service::export`), so the browser and the native GUI
// export the same file — lap charts included, which the old browser-built export left empty.
var careerExportBtn = document.getElementById('career-export-btn');
if (careerExportBtn) {
  careerExportBtn.addEventListener('click', function () {
    location.href = '/api/career/export';
  });
}
