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
        var statusEl  = document.getElementById('live-status');
        var statusTxt = document.getElementById('live-status-text');
        var infoEl    = document.getElementById('live-info');
        var liveBody  = document.getElementById('live-tbody');
        var sessType  = document.getElementById('live-session-type');
        var raceState = document.getElementById('live-race-state');
        var trackEl   = document.getElementById('live-track');
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
        raceState.textContent = RACE_NAMES[d.race_state] || '';
        if (trackEl) trackEl.textContent = d.track_location || '';
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
          return '<tr>' +
            '<td class="live-pos">'  + pos + '</td>' +
            '<td class="live-name">' + p.name + '</td>' +
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
