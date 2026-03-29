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
    var roundSessions = round.sessions || [];
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
        var pts = isRace ? '<td class="pts">' + (champ.points_system[r.race_position - 1] || 0) + '</td>' : '';
        var carLabel = r.car_name
          ? ' <span class="result-car">' + esc(r.car_name) + '</span>'
          : '';
        return '<tr><td class="pos">' + pos + '</td>' +
          '<td>' + esc(r.name) + carLabel + dnf + '</td>' +
          pts +
          '<td class="car">' + fl + '</td></tr>';
      }).join('');

      var ptsHeader = isRace ? '<th>Pts</th>' : '';
      return '<div class="round-session">' +
        '<div class="round-session-label"><span class="session-type-badge">' + typeLabel + '</span> ' + typeName +
          ' <span class="session-track">' + fmtTrack(s) + '</span>' +
          ' <span class="session-date">' + fmtDate(s.recorded_at) + '</span>' +
          ' <span class="session-drivers">' + s.results.length + ' drivers</span>' +
        '</div>' +
        '<table class="standings-table"><thead><tr><th>Pos</th><th>Driver</th>' + ptsHeader + '<th>Best</th></tr></thead>' +
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

function renderCareerChampionships(champs) {
  var container = document.getElementById('career-container');
  if (!container) return;
  if (!champs.length) {
    container.innerHTML = '<div class="manage-placeholder" style="padding:2rem">No championships yet \u2014 create one in the Manage tab.</div>';
    return;
  }
  container.innerHTML = champs.map(function (champ, idx) {
    var open = champ.status !== 'Finished' ? ' open' : '';
    var badgeCls = champ.status === 'Finished' ? 'badge-finished' :
                   champ.status === 'Active'   ? 'badge-active' : 'badge-pending';
    return '<details id="career-champ-' + idx + '" class="championship"' + open + '>' +
      '<summary class="champ-header">' +
        '<div class="champ-title">' +
          '<h2>' + esc(champ.name) + '</h2>' +
          '<span class="badge ' + badgeCls + '">' + esc(champ.status) + '</span>' +
        '</div>' +
        '<div class="champ-meta">' +
          '<span>Points: ' + esc(champ.points_system.slice(0, 5).join('\u2013') + (champ.points_system.length > 5 ? '\u2026' : '')) + '</span>' +
          '<span>Rounds: ' + (champ.rounds || []).length + '</span>' +
        '</div>' +
      '</summary>' +
      '<div class="champ-body">' +
        '<div class="standings-panel"><h3>Driver Standings</h3>' + careerStandingsHtml(champ.driver_standings) + '</div>' +
        (champ.constructor_standings.length ? '<div class="standings-panel"><h3>Constructor Standings</h3>' + careerConstructorsHtml(champ.constructor_standings) + '</div>' : '') +
        '<div class="results-panel"><h3>Rounds</h3>' + careerRoundsHtml(champ) + '</div>' +
      '</div>' +
      '</details>';
  }).join('');
}

function renderCareerStats(driverStats) {
  var container = document.getElementById('career-stats-container');
  if (!container) return;
  var rows = (driverStats || []).map(function (d) {
    return { name: d.name, races: d.races, wins: d.wins, top3: d.top3, top10: d.top10,
             dnf: d.dnf, champWins: d.champ_wins, avgPos: d.races ? d.avg_pos.toFixed(1) : '\u2014' };
  });
  var thead = '<tr>' +
    '<th class="stat-name sort-asc" data-col="0" data-type="str">Driver</th>' +
    '<th class="stat-num" data-col="1" data-type="num">Races</th>' +
    '<th class="stat-num" data-col="2" data-type="num">Wins</th>' +
    '<th class="stat-num" data-col="3" data-type="num">Top 3</th>' +
    '<th class="stat-num" data-col="4" data-type="num">Top 10</th>' +
    '<th class="stat-num" data-col="5" data-type="num">DNF</th>' +
    '<th class="stat-num" data-col="6" data-type="num">Champ Wins</th>' +
    '<th class="stat-num" data-col="7" data-type="num">Avg Pos</th>' +
    '</tr>';
  var tbody = rows.map(function (r) {
    return '<tr><td class="stat-name">' + esc(r.name) + '</td>' +
      '<td class="stat-num">' + r.races + '</td>' +
      '<td class="stat-num">' + r.wins + '</td>' +
      '<td class="stat-num">' + r.top3 + '</td>' +
      '<td class="stat-num">' + r.top10 + '</td>' +
      '<td class="stat-num">' + (r.dnf || 0) + '</td>' +
      '<td class="stat-num">' + r.champWins + '</td>' +
      '<td class="stat-num">' + r.avgPos + '</td></tr>';
  }).join('');
  container.innerHTML = '<table class="stats-table sortable" id="career-stats-table">' +
    '<thead>' + thead + '</thead><tbody>' + tbody + '</tbody></table>';
  initSortableTableEl(document.getElementById('career-stats-table'));
}

function loadCareerChampionships() {
  fetch('/api/career').then(function (r) { return r.json(); })
    .then(function (career) {
      renderCareerChampionships(career.championships || []);
      renderCareerStats(career.driver_stats || []);
    }).catch(function () {
      var el = document.getElementById('career-container');
      if (el) el.innerHTML = '<div class="manage-placeholder" style="padding:2rem">Career data requires the server binary.</div>';
    });
}

document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () {
    if (btn.dataset.tab === 'career') loadCareerChampionships();
  });
});

// ── PDF download ──────────────────────────────────────────────────────────────
var careerPdfBtn = document.getElementById('career-pdf-btn');
if (careerPdfBtn) {
  careerPdfBtn.addEventListener('click', function () {
    // Ensure data is loaded before printing
    loadCareerChampionships();

    // Expand all <details> so rounds are visible in the PDF
    var detailsEls = document.querySelectorAll('#tab-career details');
    var wasOpen = Array.from(detailsEls).map(function (d) { return d.open; });
    detailsEls.forEach(function (d) { d.open = true; });

    window.onafterprint = function () {
      detailsEls.forEach(function (d, i) { d.open = wasOpen[i]; });
      window.onafterprint = null;
    };

    window.print();
  });
}
