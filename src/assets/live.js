// ── Live session ──────────────────────────────────────────────────────────────
var liveSort = { col: -1, asc: true };

function liveSortVal(cell, type) {
  var text = cell.textContent.trim();
  if (text === '\u2014' || text === '') return type === 'str' ? '\uffff' : Infinity;
  if (type === 'num')  return parseFloat(text) || 0;
  if (type === 'time') {
    var c = text.indexOf(':');
    if (c >= 0) return parseInt(text, 10) * 60 + parseFloat(text.slice(c + 1));
    return parseFloat(text) || Infinity;
  }
  if (type === 'gap') {
    if (text === 'Fastest') return 0;
    return parseFloat(text.replace('+', '')) || Infinity;
  }
  return text.toLowerCase();
}

function applyLiveSort() {
  if (liveSort.col < 0) return;
  var table = document.getElementById('live-table');
  if (!table) return;
  var headers = table.tHead.rows[0].cells;
  var type = headers[liveSort.col].dataset.type;
  var tbody = table.tBodies[0];
  var rows = Array.from(tbody.rows);
  var asc = liveSort.asc;
  rows.sort(function (a, b) {
    var av = liveSortVal(a.cells[liveSort.col], type);
    var bv = liveSortVal(b.cells[liveSort.col], type);
    if (av < bv) return asc ? -1 : 1;
    if (av > bv) return asc ? 1 : -1;
    return 0;
  });
  rows.forEach(function (r) { tbody.appendChild(r); });
}

// ── Live sub-tabs (Timing / Telemetry) ────────────────────────────────────────
(function initLiveSubTabs() {
  document.querySelectorAll('.live-subtab').forEach(function (btn) {
    btn.addEventListener('click', function () {
      document.querySelectorAll('.live-subtab').forEach(function (b) { b.classList.remove('live-subtab-active'); });
      document.querySelectorAll('.live-subpanel').forEach(function (p) { p.classList.add('live-subpanel-hidden'); });
      btn.classList.add('live-subtab-active');
      var panel = document.getElementById('live-sub-' + btn.dataset.sub);
      if (panel) panel.classList.remove('live-subpanel-hidden');
    });
  });
}());

// ── Live table column sort ────────────────────────────────────────────────────
(function initLiveTableSort() {
  var table = document.getElementById('live-table');
  if (!table) return;
  var headers = table.tHead.rows[0].cells;
  Array.from(headers).forEach(function (th) {
    if (!th.dataset.col) return;
    th.style.cursor = 'pointer';
    th.addEventListener('click', function () {
      var col = +th.dataset.col;
      liveSort.asc = (col === liveSort.col) ? !liveSort.asc : (th.dataset.type === 'num' ? false : true);
      liveSort.col = col;
      Array.from(headers).forEach(function (h) { h.classList.remove('sort-asc', 'sort-desc'); });
      th.classList.add(liveSort.asc ? 'sort-asc' : 'sort-desc');
      applyLiveSort();
    });
  });
}());

var topSpeeds   = {};   // name → peak km/h this session
var lastPosPoll = {};   // name → {x, z, t}
var liveTrack   = null; // track name at last poll, used to reset speed data

var SESSION_NAMES = ['', 'Practice', 'Test', 'Qualify', 'Formation Lap', 'Race', 'Time Attack'];
var RACE_NAMES    = ['', 'Not Started', 'Racing', 'Finished', 'DSQ', 'Retired', 'DNF'];

function processLiveData(d) {
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
        liveBody.innerHTML = '<tr><td colspan="10" class="live-empty">Waiting for session data\u2026</td></tr>';
        return;
      }

      statusEl.className = 'live-status live-connected';
      statusTxt.textContent = 'Connected';
      sessType.textContent = SESSION_NAMES[d.session_state] || ('Session ' + d.session_state);
      // race_state is only meaningful for race sessions
      raceState.textContent = d.session_state === 5 ? (RACE_NAMES[d.race_state] || '') : '';
      if (trackEl) trackEl.textContent = d.track_location || '';
      if (rawStates) rawStates.textContent =
        'game:' + d.game_state + '  session:' + d.session_state + '  race:' + d.race_state;
      infoEl.style.visibility = '';

      if (!d.participants || d.participants.length === 0) {
        liveBody.innerHTML = '<tr><td colspan="10" class="live-empty">No active participants</td></tr>';
        return;
      }

      // ── Top speed tracking ────────────────────────────────────────────────
      if (liveTrack !== d.track_location) {
        liveTrack   = d.track_location;
        topSpeeds   = {};
        lastPosPoll = {};
      }
      var now = Date.now();
      d.participants.forEach(function (p) {
        var kmh;
        if (p.is_player && d.player_telemetry && d.player_telemetry.speed >= 0) {
          // Use AMS2's own speed sensor (m/s → km/h) for the player — more accurate than position deltas.
          kmh = d.player_telemetry.speed * 3.6;
        } else {
          var prev = lastPosPoll[p.name];
          if (prev) {
            var dt = (now - prev.t) / 1000;
            if (dt > 0) {
              var dx = p.world_pos_x - prev.x, dz = p.world_pos_z - prev.z;
              kmh = Math.sqrt(dx * dx + dz * dz) / dt * 3.6;
            }
          }
        }
        lastPosPoll[p.name] = { x: p.world_pos_x, z: p.world_pos_z, t: now };
        if (kmh !== undefined && kmh < 450 && (!(p.name in topSpeeds) || kmh > topSpeeds[p.name])) {
          topSpeeds[p.name] = kmh;
        }
      });

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
          '<td class="live-num">'  + (topSpeeds[p.name] ? Math.round(topSpeeds[p.name]) : '\u2014') + '</td>' +
          '</tr>';
      }).join('');
      applyLiveSort();
      tmUpdate(d);
      updateSetupPanel(d);
}

// ── WebSocket connection with auto-reconnect ──────────────────────────────────
var liveWs = null;
var liveWsRetry = null;

function connectLiveWs() {
  if (liveWs || !location.host) return;
  var ws = new WebSocket('ws://' + location.host + '/ws');
  liveWs = ws;

  ws.onmessage = function (e) {
    try { processLiveData(JSON.parse(e.data)); } catch (_) {}
  };

  ws.onclose = ws.onerror = function () {
    liveWs = null;
    if (!liveWsRetry) {
      liveWsRetry = setTimeout(function () {
        liveWsRetry = null;
        connectLiveWs();
      }, 2000);
    }
  };
}

connectLiveWs();
