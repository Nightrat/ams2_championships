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
    '<thead><tr><th>Pos</th><th>Car</th><th>Pts</th><th>W</th></tr></thead>' +
    '<tbody>' + rows + '</tbody></table>';
}

function careerStandingsHtml(standings) {
  if (!standings.length) return '<p class="manage-empty">No results yet.</p>';
  var rows = standings.map(function (d, i) {
    return '<tr>' +
      '<td class="pos">' + (i + 1) + '</td>' +
      '<td>' + esc(d.name) + '</td>' +
      '<td class="pts">' + d.points + '</td>' +
      '<td class="pts">' + d.wins + '</td>' +
      '</tr>';
  }).join('');
  return '<table class="standings-table">' +
    '<thead><tr><th>Pos</th><th>Driver</th><th>Pts</th><th>W</th></tr></thead>' +
    '<tbody>' + rows + '</tbody></table>';
}

function lapChartHtml(s) {
  var chart = s.lap_chart || [];
  if (!chart.length) return '';

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

  return '<div class="lap-chart-wrap">' +
    '<div class="lap-chart-label">Lap Chart</div>' +
    '<div class="lap-chart-scroll">' +
    '<table class="lap-chart-table">' +
    '<thead><tr><th class="lc-driver-h">Driver</th>' + lapHeaders + '</tr></thead>' +
    '<tbody>' + bodyRows + '</tbody>' +
    '</table></div></div>';
}

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
    var winner = raceSess
      ? raceSess.results.find(function (r) { return r.race_position === 1; })
      : null;

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
      return '<div class="round-session">' +
        '<div class="round-session-label"><span class="session-type-badge">' + typeLabel + '</span> ' + typeName +
          ' <span class="session-track">' + fmtTrack(s) + '</span>' +
          ' <span class="session-date">' + fmtDate(s.recorded_at) + '</span>' +
          ' <span class="session-drivers">' + s.results.length + ' drivers</span>' +
        '</div>' +
        '<table class="standings-table"><thead><tr><th>Pos</th><th>Driver</th>' + ptsHeader + '<th>Laps</th><th>Best</th></tr></thead>' +
        '<tbody>' + resultRows + '</tbody></table>' +
        (isRace ? lapChartHtml(s) : '') +
        '</div>';
    }).join('');

    return '<details class="events-detail">' +
      '<summary>R' + (rIdx + 1) + ' \u2014 ' + esc(trackName) +
        (winner ? ' &nbsp;&#127942; ' + esc(winner.name) : '') +
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
  careerChamps = sortChamps(champs || []);
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
  var container = document.getElementById('career-stats-container');
  if (!container) return;
  var rows = (driverStats || []).map(function (d) {
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

var allTrackStats = [];

function aggregateTrackStats(rows) {
  var byKey = {};
  rows.forEach(function (t) {
    var key = t.track + '\x00' + t.track_variation;
    var a = byKey[key];
    if (!a) {
      byKey[key] = { track: t.track, track_variation: t.track_variation, car: '',
        races: t.races, qualifyings: t.qualifyings,
        best_lap: t.best_lap, best_lap_driver: t.best_lap_driver, best_lap_car: t.best_lap_car,
        second_lap: t.second_lap, second_lap_driver: t.second_lap_driver, second_lap_car: t.second_lap_car,
        third_lap: t.third_lap, third_lap_driver: t.third_lap_driver, third_lap_car: t.third_lap_car,
        last_visited: t.last_visited };
      return;
    }
    a.races += t.races;
    a.qualifyings += t.qualifyings;
    if (t.last_visited > a.last_visited) a.last_visited = t.last_visited;
    // Merge all per-driver bests across cars, keep top 3 unique drivers by lap time
    var allLaps = [
      { t: a.best_lap,   d: a.best_lap_driver,   c: a.best_lap_car },
      { t: a.second_lap, d: a.second_lap_driver, c: a.second_lap_car },
      { t: a.third_lap,  d: a.third_lap_driver,  c: a.third_lap_car },
      { t: t.best_lap,   d: t.best_lap_driver,   c: t.best_lap_car },
      { t: t.second_lap, d: t.second_lap_driver, c: t.second_lap_car },
      { t: t.third_lap,  d: t.third_lap_driver,  c: t.third_lap_car },
    ].filter(function (l) { return l.t > 0 && l.d; });
    // Keep best lap per driver, then sort
    var byDriver = {};
    allLaps.forEach(function (l) {
      if (!byDriver[l.d] || l.t < byDriver[l.d].t) byDriver[l.d] = l;
    });
    var top3 = Object.values(byDriver).sort(function (x, y) { return x.t - y.t; }).slice(0, 3);
    a.best_lap          = top3[0] ? top3[0].t : 0; a.best_lap_driver   = top3[0] ? top3[0].d : ''; a.best_lap_car    = top3[0] ? top3[0].c : '';
    a.second_lap        = top3[1] ? top3[1].t : 0; a.second_lap_driver = top3[1] ? top3[1].d : ''; a.second_lap_car  = top3[1] ? top3[1].c : '';
    a.third_lap         = top3[2] ? top3[2].t : 0; a.third_lap_driver  = top3[2] ? top3[2].d : ''; a.third_lap_car   = top3[2] ? top3[2].c : '';
  });
  return Object.values(byKey);
}

function buildTrackCarFilter(trackStats) {
  var cars = [];
  trackStats.forEach(function (t) { if (t.car && cars.indexOf(t.car) === -1) cars.push(t.car); });
  cars.sort();
  return cars;
}

function renderTrackStats(trackStats) {
  allTrackStats = trackStats || [];
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
  var rows = car ? allTrackStats.filter(function (t) { return t.car === car; }) : aggregateTrackStats(allTrackStats);
  rows.sort(function (a, b) { return b.last_visited - a.last_visited; });
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
      renderTrackStats(career.track_stats || []);
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

// ── PDF download ──────────────────────────────────────────────────────────────
var careerPdfBtn = document.getElementById('career-pdf-btn');
if (careerPdfBtn) {
  careerPdfBtn.addEventListener('click', function () {
    if (!careerChamps.length) { loadCareerChampionships(); return; }

    // Build a flat print view of all championships with all round details open
    var badgeMap = { Final: 'badge-final', Progress: 'badge-progress', Active: 'badge-active' };
    var printHtml = careerChamps.map(function (champ) {
      var badgeCls = badgeMap[champ.status] || 'badge-active';
      return '<div class="championship" style="break-inside:avoid-page">' +
        '<div class="champ-header" style="padding:0.6rem 1rem">' +
          '<div class="champ-title"><h2>' + esc(champ.name) + '</h2>' +
            '<span class="badge ' + badgeCls + '">' + esc(champ.status) + '</span></div>' +
        '</div>' +
        '<div class="champ-body">' +
          '<div class="standings-panel"><h3>Driver Standings</h3>' + careerStandingsHtml(champ.driver_standings) + '</div>' +
          (champ.constructor_standings.length ? '<div class="standings-panel"><h3>Constructor Standings</h3>' + careerConstructorsHtml(champ.constructor_standings) + '</div>' : '') +
          '<div class="results-panel"><h3>Rounds</h3>' + careerRoundsHtml(champ) + '</div>' +
        '</div></div>';
    }).join('');

    var subPanel = document.getElementById('career-sub-champs');
    var origHtml = subPanel.innerHTML;
    subPanel.innerHTML = '<div style="padding:1rem">' + printHtml + '</div>';

    // Expand all round <details>
    subPanel.querySelectorAll('details').forEach(function (d) { d.open = true; });

    window.onafterprint = function () {
      subPanel.innerHTML = origHtml;
      // Re-attach list click handlers
      renderCareerChampionships(careerChamps);
      window.onafterprint = null;
    };

    window.print();
  });
}
