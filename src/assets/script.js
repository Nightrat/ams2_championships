(function () {
  // ── Tab switching ──────────────────────────────────────────────────────────
  document.querySelectorAll('.tab-btn').forEach(function (btn) {
    btn.addEventListener('click', function () {
      document.querySelectorAll('.tab-btn').forEach(function (b) { b.classList.remove('tab-active'); });
      document.querySelectorAll('.tab-panel').forEach(function (p) { p.classList.add('tab-panel-hidden'); });
      btn.classList.add('tab-active');
      document.getElementById('tab-' + btn.dataset.tab).classList.remove('tab-panel-hidden');
    });
  });

  // ── Sub-tab switching (SecondMonitor Import) ───────────────────────────────
  document.querySelectorAll('.sub-tab-btn').forEach(function (btn) {
    btn.addEventListener('click', function () {
      document.querySelectorAll('.sub-tab-btn').forEach(function (b) { b.classList.remove('sub-tab-active'); });
      document.querySelectorAll('.sub-tab-panel').forEach(function (p) { p.classList.add('sub-tab-panel-hidden'); });
      btn.classList.add('sub-tab-active');
      document.getElementById('subtab-' + btn.dataset.subtab).classList.remove('sub-tab-panel-hidden');
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

  // ── Live Session polling ───────────────────────────────────────────────────
  var liveTimer = null;

  function fmtLapTime(t) {
    if (!t || t <= 0) return '<span class="no-time">\u2014</span>';
    var m = Math.floor(t / 60);
    var s = t % 60;
    var ss = s.toFixed(3);
    if (parseFloat(ss) < 10) ss = '0' + ss;
    return m > 0 ? m + ':' + ss : ss;
  }

  var SESSION_NAMES = ['', 'Practice', 'Test', 'Qualify', 'Formation Lap', 'Race', 'Time Attack'];
  var RACE_NAMES    = ['', 'Not Started', 'Racing', 'Finished', 'DSQ', 'Retired', 'DNF'];

  function updateLive() {
    fetch('/live')
      .then(function (r) { return r.json(); })
      .then(function (d) {
        var statusEl   = document.getElementById('live-status');
        var statusTxt  = document.getElementById('live-status-text');
        var infoEl     = document.getElementById('live-info');
        var liveBody   = document.getElementById('live-tbody');
        var sessType   = document.getElementById('live-session-type');
        var raceState  = document.getElementById('live-race-state');
        var trackEl    = document.getElementById('live-track');
        var rawStates  = document.getElementById('live-raw-states');
        if (!statusEl || !liveBody) return;

        if (!d.connected || d.game_state < 2) {
          statusEl.className = 'live-status live-disconnected';
          statusTxt.textContent = 'Not connected \u2014 start AMS2 to see live data';
          infoEl.style.visibility = 'hidden';
          liveBody.innerHTML = '<tr><td colspan="9" class="live-empty">Waiting for session data\u2026</td></tr>';
          return;
        }

        statusEl.className = 'live-status live-connected';
        statusTxt.textContent = 'Connected';
        sessType.textContent = SESSION_NAMES[d.session_state] || ('Session ' + d.session_state);
        // race_state is only meaningful for race sessions; hide it otherwise to avoid "Not Started" showing during practice.
        raceState.textContent = d.session_state === 5 ? (RACE_NAMES[d.race_state] || '') : '';
        if (trackEl) trackEl.textContent = d.track_location || '';
        if (rawStates) rawStates.textContent =
          'game:' + d.game_state + '  session:' + d.session_state + '  race:' + d.race_state;
        infoEl.style.visibility = '';

        if (!d.participants || d.participants.length === 0) {
          liveBody.innerHTML = '<tr><td colspan="9" class="live-empty">No active participants</td></tr>';
          return;
        }

        // ── Gap to fastest lap ────────────────────────────────────────────────
        var bestLap = 0;
        d.participants.forEach(function (p) {
          if (p.fastest_lap_time > 0 && (bestLap === 0 || p.fastest_lap_time < bestLap)) {
            bestLap = p.fastest_lap_time;
          }
        });

        // Best sector times across all participants (for purple highlight)
        var bestS1 = 0, bestS2 = 0, bestS3 = 0;
        d.participants.forEach(function (p) {
          if (p.best_s1 > 0 && (bestS1 === 0 || p.best_s1 < bestS1)) bestS1 = p.best_s1;
          if (p.best_s2 > 0 && (bestS2 === 0 || p.best_s2 < bestS2)) bestS2 = p.best_s2;
          if (p.best_s3 > 0 && (bestS3 === 0 || p.best_s3 < bestS3)) bestS3 = p.best_s3;
        });

        function fmtGap(p) {
          if (bestLap <= 0 || p.fastest_lap_time <= 0) return '<span class="no-time">\u2014</span>';
          var delta = p.fastest_lap_time - bestLap;
          if (delta < 0.001) return '<span class="live-gap-leader">Fastest</span>';
          return '+' + delta.toFixed(3);
        }

        // Sector cell: show current-lap sector time, highlight if it matches
        // the overall best sector (purple) or the driver's own best (green).
        function fmtSector(cur, best, overallBest) {
          // cur: current lap's sector time; best: driver's personal best; overallBest: field best
          var display = cur > 0 ? cur : best;
          if (display <= 0) return '<span class="no-time">\u2014</span>';
          var cls = 'live-sector';
          if (overallBest > 0 && display <= overallBest + 0.001) cls = 'live-sector-best';
          else if (best > 0 && cur > 0 && cur <= best + 0.001)   cls = 'live-sector-pb';
          return '<span class="' + cls + '">' + fmtLapTime(display) + '</span>';
        }

        liveBody.innerHTML = d.participants.map(function (p) {
          var pos = p.race_position > 0 ? p.race_position : '\u2014';
          var rowCls = p.is_player ? ' class="player-row"' : '';
          var nameSuffix = p.is_player ? ' <span class="player-tag">YOU</span>' : '';
          return '<tr' + rowCls + '>' +
            '<td class="live-pos">'  + pos + '</td>' +
            '<td class="live-name">' + p.name + nameSuffix + '</td>' +
            '<td class="live-num">'  + p.current_lap + '</td>' +
            '<td class="live-gap">'  + fmtGap(p) + '</td>' +
            '<td class="live-time">' + fmtSector(p.cur_s1, p.best_s1, bestS1) + '</td>' +
            '<td class="live-time">' + fmtSector(p.cur_s2, p.best_s2, bestS2) + '</td>' +
            '<td class="live-time">' + fmtSector(p.cur_s3, p.best_s3, bestS3) + '</td>' +
            '<td class="live-time">' + fmtLapTime(p.fastest_lap_time) + '</td>' +
            '<td class="live-time">' + fmtLapTime(p.last_lap_time) + '</td>' +
            '</tr>';
        }).join('');
      })
      .catch(function () { /* server unavailable (static file) — ignore */ });
  }

  // Live Session is the default tab — start polling immediately.
  updateLive();
  liveTimer = setInterval(updateLive, 2000);

  document.querySelectorAll('.tab-btn').forEach(function (btn) {
    btn.addEventListener('click', function () {
      if (btn.dataset.tab === 'live') {
        updateLive();
        if (!liveTimer) liveTimer = setInterval(updateLive, 2000);
      } else {
        if (liveTimer) { clearInterval(liveTimer); liveTimer = null; }
      }
    });
  });

  // ── Career Championships tab ───────────────────────────────────────────────

  var SESSION_TYPE_LABELS = { 1: 'P', 3: 'Q', 5: 'R' };
  var SESSION_TYPE_NAMES  = { 1: 'Practice', 3: 'Qualify', 5: 'Race' };

  function careerComputeStandings(champ, sessions) {
    var pts = {}, wins = {};
    (champ.rounds || []).forEach(function (round) {
      (round.session_ids || []).forEach(function (sid) {
        var s = sessions.find(function (s) { return s.id === sid; });
        if (!s || s.session_type !== 5) return; // only race sessions score points
        s.results.forEach(function (r) {
          if (!pts[r.name]) { pts[r.name] = 0; wins[r.name] = 0; }
          if (!r.dnf) {
            pts[r.name] += champ.points_system[r.race_position - 1] || 0;
            if (r.race_position === 1) wins[r.name]++;
          }
        });
      });
    });
    return Object.keys(pts).map(function (name) {
      return { name: name, points: pts[name], wins: wins[name] };
    }).sort(function (a, b) { return b.points - a.points || b.wins - a.wins; });
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

  function careerRoundsHtml(champ, sessions) {
    var rounds = champ.rounds || [];
    if (!rounds.length) return '<p class="manage-empty">No rounds assigned yet.</p>';

    var rows = rounds.map(function (round, rIdx) {
      var roundSessions = (round.session_ids || [])
        .map(function (sid) { return sessions.find(function (s) { return s.id === sid; }); })
        .filter(Boolean)
        .sort(function (a, b) { return a.session_type - b.session_type; });
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
          var dnf = r.dnf ? ' <span class="badge badge-pending">DNF</span>' : '';
          var pts = isRace ? '<td class="pts">' + (champ.points_system[r.race_position - 1] || 0) + '</td>' : '';
          return '<tr><td class="pos">' + pos + '</td>' +
            '<td>' + esc(r.name) + dnf + '</td>' +
            pts +
            '<td class="car">' + fl + '</td></tr>';
        }).join('');

        var ptsHeader = isRace ? '<th>Pts</th>' : '';
        return '<div class="round-session">' +
          '<div class="round-session-label"><span class="session-type-badge">' + typeLabel + '</span> ' + typeName +
            ' <span class="session-date">' + fmtDate(s.recorded_at) + '</span>' +
            ' <span class="session-drivers">' + s.results.length + ' drivers</span>' +
          '</div>' +
          '<table class="standings-table"><thead><tr><th>Pos</th><th>Driver</th>' + ptsHeader + '<th>Best Lap</th></tr></thead>' +
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

  function renderCareerChampionships(champs, sessions) {
    var container = document.getElementById('career-container');
    if (!container) return;
    if (!champs.length) {
      container.innerHTML = '<div class="manage-placeholder" style="padding:2rem">No championships yet \u2014 create one in the Manage tab.</div>';
      return;
    }
    container.innerHTML = champs.map(function (champ, idx) {
      var standings = careerComputeStandings(champ, sessions);
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
          '<div class="standings-panel"><h3>Standings</h3>' + careerStandingsHtml(standings) + '</div>' +
          '<div class="results-panel"><h3>Rounds</h3>' + careerRoundsHtml(champ, sessions) + '</div>' +
        '</div>' +
        '</details>';
    }).join('');
  }

  function loadCareerChampionships() {
    Promise.all([
      fetch('/api/championships').then(function (r) { return r.json(); }),
      fetch('/api/sessions').then(function (r) { return r.json(); })
    ]).then(function (results) {
      renderCareerChampionships(results[0] || [], results[1] || []);
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

  // ── Manage tab ─────────────────────────────────────────────────────────────
  var manageState = { champs: [], sessions: [], selectedId: null, currentRidx: 0 };

  function esc(str) {
    return String(str)
      .replace(/&/g, '&amp;').replace(/</g, '&lt;')
      .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  function fmtDate(ts) {
    if (!ts) return '';
    var d = new Date(ts * 1000);
    return d.getFullYear() + '-' +
      String(d.getMonth() + 1).padStart(2, '0') + '-' +
      String(d.getDate()).padStart(2, '0');
  }

  function sessionWinner(session) {
    if (!session.results || !session.results.length) return '\u2014';
    var w = session.results.find(function (r) { return r.race_position === 1; });
    return w ? w.name : '\u2014';
  }

  function loadManage() {
    Promise.all([
      fetch('/api/championships').then(function (r) { return r.json(); }),
      fetch('/api/sessions').then(function (r) { return r.json(); })
    ]).then(function (results) {
      manageState.champs = results[0] || [];
      manageState.sessions = results[1] || [];
      renderChampList();
      if (manageState.selectedId) renderChampDetail(manageState.selectedId);
    }).catch(function () {
      var right = document.getElementById('manage-right');
      if (right) right.innerHTML = '<div class="manage-placeholder">Management requires the server binary \u2014 open this page via <code>ams2_championship_server</code>.</div>';
    });
  }

  function renderChampList() {
    var el = document.getElementById('champ-list');
    if (!el) return;
    if (!manageState.champs.length) {
      el.innerHTML = '<div class="manage-empty">No championships yet.</div>';
      return;
    }
    el.innerHTML = manageState.champs.map(function (c) {
      var sel = c.id === manageState.selectedId ? ' selected' : '';
      var statusCls = c.status === 'Active' ? 'status-active' :
                      c.status === 'Finished' ? 'status-finished' : 'status-pending';
      return '<div class="champ-list-item' + sel + '" data-id="' + esc(c.id) + '">' +
        '<span class="champ-list-name">' + esc(c.name) + '</span>' +
        '<span class="champ-status ' + statusCls + '">' + esc(c.status) + '</span>' +
        '</div>';
    }).join('');
    el.querySelectorAll('.champ-list-item').forEach(function (item) {
      item.addEventListener('click', function () {
        manageState.selectedId = item.dataset.id;
        renderChampList();
        renderChampDetail(item.dataset.id);
      });
    });
  }

  function renderChampDetail(id) {
    var champ = manageState.champs.find(function (c) { return c.id === id; });
    var right = document.getElementById('manage-right');
    if (!champ || !right) return;

    var rounds = champ.rounds || [];

    var roundsHtml = rounds.length === 0
      ? '<div class="manage-empty">No rounds yet. Click \u201c+ Add Round\u201d to create one.</div>'
      : rounds.map(function (round, rIdx) {
          var roundSessions = (round.session_ids || [])
            .map(function (sid) { return manageState.sessions.find(function (s) { return s.id === sid; }); })
            .filter(Boolean);

          var sessionCards = roundSessions.map(function (s) {
            var typeLabel = SESSION_TYPE_LABELS[s.session_type] || '?';
            return '<div class="session-card">' +
              '<div class="session-card-info">' +
                '<span class="session-type-badge">' + typeLabel + '</span>' +
                '<span class="session-track">' + esc(s.track) + '</span>' +
                '<span class="session-date">' + fmtDate(s.recorded_at) + '</span>' +
                '<span class="session-drivers">' + s.results.length + ' drivers</span>' +
                '<span class="session-winner">\u{1f3c6} ' + esc(sessionWinner(s)) + '</span>' +
              '</div>' +
              '<button class="manage-btn manage-btn-danger session-remove-btn"' +
                ' data-cid="' + esc(champ.id) + '" data-ridx="' + rIdx + '" data-sid="' + esc(s.id) + '">Remove</button>' +
              '</div>';
          }).join('') || '<div class="manage-empty">No sessions in this round.</div>';

          return '<div class="round-block">' +
            '<div class="round-block-header">' +
              '<span class="round-block-title">Round ' + (rIdx + 1) + '</span>' +
              '<button class="manage-btn manage-btn-primary show-sessions-btn" data-ridx="' + rIdx + '">+ Add Session</button>' +
              '<button class="manage-btn manage-btn-danger round-remove-btn" data-cid="' + esc(champ.id) + '" data-ridx="' + rIdx + '">Remove Round</button>' +
            '</div>' +
            '<div class="round-block-sessions">' + sessionCards + '</div>' +
            '</div>';
        }).join('');

    right.innerHTML =
      '<div class="champ-detail">' +
      '<div class="champ-detail-header">' +
        '<input class="manage-input champ-name-input" value="' + esc(champ.name) + '" data-id="' + esc(champ.id) + '">' +
        '<button class="manage-btn manage-btn-danger champ-delete-btn" data-id="' + esc(champ.id) + '">Delete</button>' +
      '</div>' +
      '<div class="champ-detail-meta">' +
        '<label>Status&nbsp;<select class="manage-select champ-status-select" data-id="' + esc(champ.id) + '">' +
          ['Pending', 'Active', 'Finished'].map(function (s) {
            return '<option' + (s === champ.status ? ' selected' : '') + '>' + s + '</option>';
          }).join('') +
        '</select></label>' +
        '<label>Points&nbsp;<input class="manage-input champ-points-input" value="' + esc(champ.points_system.join(',')) + '" data-id="' + esc(champ.id) + '" size="32" title="Comma-separated points per finishing position"></label>' +
        '<label class="manage-checkbox-label"><input type="checkbox" class="champ-manufacturer-check"' + (champ.manufacturer_scoring ? ' checked' : '') + '> Constructor Scoring</label>' +
      '</div>' +
      '<div class="champ-rounds-header">' +
        '<span>Rounds&nbsp;(' + rounds.length + ')</span>' +
        '<button class="manage-btn manage-btn-primary add-round-btn" data-cid="' + esc(champ.id) + '">+ Add Round</button>' +
      '</div>' +
      '<div class="champ-rounds-list">' + roundsHtml + '</div>' +
      '</div>';

    right.querySelector('.champ-name-input').addEventListener('blur', function () {
      patchChamp(champ.id, { name: this.value });
    });
    right.querySelector('.champ-status-select').addEventListener('change', function () {
      patchChamp(champ.id, { status: this.value });
    });
    right.querySelector('.champ-points-input').addEventListener('blur', function () {
      var pts = this.value.split(',')
        .map(function (v) { return parseInt(v.trim(), 10); })
        .filter(function (n) { return !isNaN(n); });
      patchChamp(champ.id, { points_system: pts });
    });
    right.querySelector('.champ-manufacturer-check').addEventListener('change', function () {
      patchChamp(champ.id, { manufacturer_scoring: this.checked });
    });
    right.querySelector('.champ-delete-btn').addEventListener('click', function () {
      if (!confirm('Delete "' + champ.name + '"?')) return;
      fetch('/api/championships/' + champ.id, { method: 'DELETE' }).then(function () {
        manageState.selectedId = null;
        loadManage();
        var right = document.getElementById('manage-right');
        if (right) right.innerHTML = '<div class="manage-placeholder">Select a championship or create a new one.</div>';
      });
    });
    right.querySelector('.add-round-btn').addEventListener('click', function () {
      fetch('/api/championships/' + champ.id + '/rounds', { method: 'POST' })
        .then(function () { loadManage(); });
    });
    right.querySelectorAll('.round-remove-btn').forEach(function (btn) {
      btn.addEventListener('click', function () {
        if (!confirm('Remove round ' + (+btn.dataset.ridx + 1) + ' and all its sessions?')) return;
        fetch('/api/championships/' + btn.dataset.cid + '/rounds/' + btn.dataset.ridx,
              { method: 'DELETE' })
          .then(function () { loadManage(); });
      });
    });
    right.querySelectorAll('.session-remove-btn').forEach(function (btn) {
      btn.addEventListener('click', function () {
        fetch('/api/championships/' + btn.dataset.cid + '/rounds/' + btn.dataset.ridx + '/sessions/' + btn.dataset.sid,
              { method: 'DELETE' })
          .then(function () { loadManage(); });
      });
    });
    right.querySelectorAll('.show-sessions-btn').forEach(function (btn) {
      btn.addEventListener('click', function () {
        manageState.currentRidx = parseInt(btn.dataset.ridx, 10);
        renderAvailableSessions(champ.id);
        var panel = document.getElementById('manage-sessions-panel');
        if (panel) panel.style.display = '';
      });
    });
  }

  function renderAvailableSessions(champId) {
    var champ = manageState.champs.find(function (c) { return c.id === champId; });
    var assignedIds = [];
    if (champ) {
      (champ.rounds || []).forEach(function (round) {
        (round.session_ids || []).forEach(function (sid) { assignedIds.push(sid); });
      });
    }
    var ridx = manageState.currentRidx || 0;
    var available = manageState.sessions.filter(function (s) { return !assignedIds.includes(s.id); });
    var el = document.getElementById('available-sessions');
    if (!el) return;
    if (!available.length) {
      el.innerHTML = '<div class="manage-empty">No unassigned sessions.</div>';
      return;
    }
    el.innerHTML = available.map(function (s) {
      var typeLabel = SESSION_TYPE_LABELS[s.session_type] || '?';
      return '<div class="session-card">' +
        '<div class="session-card-info">' +
          '<span class="session-type-badge">' + typeLabel + '</span>' +
          '<span class="session-track">' + esc(s.track) + '</span>' +
          '<span class="session-date">' + fmtDate(s.recorded_at) + '</span>' +
          '<span class="session-drivers">' + s.results.length + ' drivers</span>' +
          '<span class="session-winner">\u{1f3c6} ' + esc(sessionWinner(s)) + '</span>' +
        '</div>' +
        '<button class="manage-btn manage-btn-primary session-add-btn"' +
          ' data-cid="' + esc(champId) + '" data-ridx="' + ridx + '" data-sid="' + esc(s.id) + '">+ Add to Round ' + (ridx + 1) + '</button>' +
        '</div>';
    }).join('');
    el.querySelectorAll('.session-add-btn').forEach(function (btn) {
      btn.addEventListener('click', function () {
        fetch('/api/championships/' + btn.dataset.cid + '/rounds/' + btn.dataset.ridx + '/sessions/' + btn.dataset.sid,
              { method: 'POST' })
          .then(function () {
            loadManage();
            renderChampDetail(champId);
            renderAvailableSessions(champId);
          });
      });
    });
  }

  function patchChamp(id, patch) {
    fetch('/api/championships/' + id, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(patch)
    }).then(function () { loadManage(); });
  }

  // New championship form wiring
  var addChampBtn = document.getElementById('add-champ-btn');
  var newForm = document.getElementById('manage-new-form');
  if (addChampBtn && newForm) {
    addChampBtn.addEventListener('click', function () {
      newForm.style.display = '';
      document.getElementById('new-champ-name').focus();
    });
    document.getElementById('new-champ-cancel').addEventListener('click', function () {
      newForm.style.display = 'none';
    });
    document.getElementById('new-champ-points').addEventListener('change', function () {
      var custom = document.getElementById('new-champ-custom');
      custom.style.display = this.value === 'custom' ? '' : 'none';
    });
    document.getElementById('new-champ-save').addEventListener('click', function () {
      var name = document.getElementById('new-champ-name').value.trim();
      if (!name) { alert('Enter a championship name.'); return; }
      var ptsEl = document.getElementById('new-champ-points');
      var ptsVal = ptsEl.value === 'custom'
        ? document.getElementById('new-champ-custom').value
        : ptsEl.value;
      var pts = ptsVal.split(',')
        .map(function (v) { return parseInt(v.trim(), 10); })
        .filter(function (n) { return !isNaN(n); });
      var manufacturerScoring = document.getElementById('new-champ-manufacturer').checked;
      fetch('/api/championships', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: name, points_system: pts, manufacturer_scoring: manufacturerScoring })
      }).then(function () {
        newForm.style.display = 'none';
        document.getElementById('new-champ-name').value = '';
        loadManage();
      });
    });
  }

  var closeSessionsBtn = document.getElementById('close-sessions-btn');
  if (closeSessionsBtn) {
    closeSessionsBtn.addEventListener('click', function () {
      var panel = document.getElementById('manage-sessions-panel');
      if (panel) panel.style.display = 'none';
    });
  }

  var purgeBtn = document.getElementById('purge-sessions-btn');
  if (purgeBtn) {
    purgeBtn.addEventListener('click', function () {
      var unassigned = manageState.sessions.filter(function (s) {
        return !manageState.champs.some(function (c) {
          return (c.rounds || []).some(function (r) {
            return (r.session_ids || []).includes(s.id);
          });
        });
      });
      if (!unassigned.length) { alert('No unassigned sessions.'); return; }
      if (!confirm('Delete ' + unassigned.length + ' unassigned session(s)? This cannot be undone.')) return;
      fetch('/api/sessions/unassigned', { method: 'DELETE' })
        .then(function (r) { return r.json(); })
        .then(function (d) { loadManage(); });
    });
  }

  // Load manage data whenever the tab is activated
  document.querySelectorAll('.tab-btn').forEach(function (btn) {
    btn.addEventListener('click', function () {
      if (btn.dataset.tab === 'manage') loadManage();
    });
  });

  // ── Download button ────────────────────────────────────────────────────────
  var dlBtn = document.getElementById('download-btn');
  if (dlBtn) {
    dlBtn.addEventListener('click', function () {
      var html = '<!DOCTYPE html>\n' + document.documentElement.outerHTML;
      var blob = new Blob([html], { type: 'text/html' });
      var a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = 'championships.html';
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(a.href);
    });
  }
}());
