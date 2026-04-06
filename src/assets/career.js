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
        '<tbody>' + resultRows + '</tbody></table></div>';
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
  careerChamps = champs || [];
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

function renderTrackStats(trackStats) {
  var container = document.getElementById('career-tracks-container');
  if (!container) return;
  if (!(trackStats || []).length) {
    container.innerHTML = '<div class="manage-placeholder" style="padding:2rem">No sessions recorded yet.</div>';
    return;
  }
  var thead = '<tr>' +
    '<th class="stat-name sort-asc" data-col="0" data-type="str">Track</th>' +
    '<th class="stat-num" data-col="1" data-type="num">Races</th>' +
    '<th class="stat-num" data-col="2" data-type="num">Qualifyings</th>' +
    '<th class="stat-num" data-col="3" data-type="time">Best Lap</th>' +
    '<th class="stat-name" data-col="4" data-type="str">Record Holder</th>' +
    '<th class="stat-name" data-col="5" data-type="str">Car</th>' +
    '<th class="stat-num" data-col="6" data-type="str">Last Visited</th>' +
    '</tr>';
  var tbody = trackStats.map(function (t) {
    var name = t.track_variation && t.track_variation !== t.track
      ? esc(t.track) + ' <span class="session-track-var">' + esc(t.track_variation) + '</span>'
      : esc(t.track);
    return '<tr>' +
      '<td class="stat-name">' + name + '</td>' +
      '<td class="stat-num">' + t.races + '</td>' +
      '<td class="stat-num">' + t.qualifyings + '</td>' +
      '<td class="stat-num">' + (t.best_lap > 0 ? fmtLapTime(t.best_lap) : '\u2014') + '</td>' +
      '<td class="stat-name">' + (t.best_lap_driver ? esc(t.best_lap_driver) : '\u2014') + '</td>' +
      '<td class="stat-name">' + (t.best_lap_car ? esc(t.best_lap_car) : '\u2014') + '</td>' +
      '<td class="stat-num">' + fmtDate(t.last_visited) + '</td>' +
    '</tr>';
  }).join('');
  container.innerHTML = '<table class="stats-table sortable" id="career-tracks-table">' +
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
