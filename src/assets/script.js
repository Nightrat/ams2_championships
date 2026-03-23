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

  // ── SecondMonitor XML import ───────────────────────────────────────────────
  var smInput = document.getElementById('sm-xml-input');
  if (smInput) {
    smInput.addEventListener('change', function (e) {
      var file = e.target.files[0];
      if (!file) return;
      document.getElementById('sm-xml-filename').textContent = file.name;
      var statusEl = document.getElementById('sm-xml-status');
      statusEl.textContent = 'Processing\u2026';
      statusEl.className = 'import-status';
      var reader = new FileReader();
      reader.onload = function (ev) {
        fetch('/api/import', {
          method: 'POST',
          headers: { 'Content-Type': 'text/xml' },
          body: ev.target.result
        })
          .then(function (r) { return r.json(); })
          .then(function (data) {
            document.getElementById('subtab-championships').innerHTML = data.championships_html;
            document.getElementById('subtab-driver-stats').innerHTML  = data.stats_html;
            initSortableTable();
            statusEl.textContent = 'Imported \u2713';
            statusEl.className = 'import-status import-status-ok';
          })
          .catch(function () {
            statusEl.textContent = 'Import failed';
            statusEl.className = 'import-status import-status-err';
          });
      };
      reader.readAsText(file);
    });
  }

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
  function initSortableTable() {
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
  }
  initSortableTable();

  // ── Live table sort ────────────────────────────────────────────────────────
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

  // ── Live sub-tabs (Timing / Telemetry) ────────────────────────────────────
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

  // ── Track map ──────────────────────────────────────────────────────────────
  var trackMap = { track: null, points: null, cells: {}, accumulated: [], saved: false, loading: false };
  var TM_CELL = 5;     // metres per grid cell for deduplication
  var TM_MIN  = 300;   // minimum unique cells before saving

  function tmCellKey(x, z) { return Math.floor(x / TM_CELL) + ',' + Math.floor(z / TM_CELL); }

  function tmAddPoints(participants) {
    participants.forEach(function (p) {
      var key = tmCellKey(p.world_pos_x, p.world_pos_z);
      if (!trackMap.cells[key]) {
        trackMap.cells[key] = true;
        trackMap.accumulated.push([p.world_pos_x, p.world_pos_z]);
      }
    });
  }

  function tmLoad(track) {
    if (trackMap.loading) return;
    trackMap.loading = true;
    fetch('/api/track-layout/' + encodeURIComponent(track))
      .then(function (r) { return r.json(); })
      .then(function (data) {
        trackMap.loading = false;
        if (Array.isArray(data) && data.length > 50) {
          trackMap.points = data;
          trackMap.saved  = true;
        }
      })
      .catch(function () { trackMap.loading = false; });
  }

  function tmSave(track) {
    fetch('/api/track-layout/' + encodeURIComponent(track), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(trackMap.accumulated)
    }).catch(function () {});
  }

  function tmRender(canvas, points, cars) {
    var ctx = canvas.getContext('2d');
    var W = canvas.width, H = canvas.height;
    ctx.fillStyle = '#0a0a14';
    ctx.fillRect(0, 0, W, H);

    if (!points || points.length < 20) {
      ctx.fillStyle = '#2a2a3e';
      ctx.font = '11px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('Collecting track data\u2026', W / 2, H / 2);
      ctx.textAlign = 'left';
      return;
    }

    var PAD = 16;
    var minX = Infinity, maxX = -Infinity, minZ = Infinity, maxZ = -Infinity;
    points.forEach(function (p) {
      if (p[0] < minX) minX = p[0]; if (p[0] > maxX) maxX = p[0];
      if (p[1] < minZ) minZ = p[1]; if (p[1] > maxZ) maxZ = p[1];
    });
    var rangeX = maxX - minX || 1, rangeZ = maxZ - minZ || 1;
    var scale = Math.min((W - 2 * PAD) / rangeX, (H - 2 * PAD) / rangeZ);
    var ox = (W - rangeX * scale) / 2 - minX * scale;
    var oz = (H - rangeZ * scale) / 2 - minZ * scale;
    function cx(x) { return x * scale + ox; }
    function cz(z) { return z * scale + oz; }

    // Track dots
    ctx.fillStyle = '#3a3a5e';
    points.forEach(function (p) { ctx.fillRect(cx(p[0]) - 2, cz(p[1]) - 2, 4, 4); });

    // Car positions
    cars.forEach(function (c) {
      var x = cx(c.x), z = cz(c.z);
      ctx.beginPath();
      ctx.arc(x, z, c.isPlayer ? 5 : 3.5, 0, 2 * Math.PI);
      ctx.fillStyle = c.isPlayer ? '#f1c40f' : '#e74c3c';
      ctx.fill();
      if (c.isPlayer && c.pos > 0) {
        ctx.fillStyle = '#fff';
        ctx.font = 'bold 9px sans-serif';
        ctx.fillText('P' + c.pos, x + 7, z + 4);
      }
    });
  }

  function tmUpdate(d) {
    var canvas = document.getElementById('track-map');
    if (!canvas) return;

    if (!d.connected || !d.track_location) {
      var ctx = canvas.getContext('2d');
      ctx.fillStyle = '#0a0a14';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      return;
    }

    // Reset when track changes
    if (trackMap.track !== d.track_location) {
      trackMap.track       = d.track_location;
      trackMap.points      = null;
      trackMap.cells       = {};
      trackMap.accumulated = [];
      trackMap.saved       = false;
      tmLoad(d.track_location);
    }

    if (d.participants && d.participants.length > 0) {
      tmAddPoints(d.participants);
    }

    if (!trackMap.saved && Object.keys(trackMap.cells).length >= TM_MIN) {
      trackMap.saved = true;
      tmSave(d.track_location);
    }

    var renderPoints = trackMap.points || (trackMap.accumulated.length >= 20 ? trackMap.accumulated : null);
    var cars = (d.participants || []).map(function (p) {
      return { x: p.world_pos_x, z: p.world_pos_z, isPlayer: p.is_player, pos: p.race_position };
    });
    tmRender(canvas, renderPoints, cars);
  }

  // ── Setup telemetry panel ──────────────────────────────────────────────────
  var telBuf           = [];   // rolling samples for the viewed driver
  var telLastViewedName = null; // persists through garage (mViewedParticipantIndex = -1)
  var TEL_BUF_SIZE = 20;

  function computeAvgTel(buf) {
    function avg(field, idx) {
      if (!buf.length) return 0;
      var sum = 0;
      buf.forEach(function (t) { sum += (idx !== undefined ? t[field][idx] : t[field]); });
      return sum / buf.length;
    }
    return {
      tyre_temp_left:    [0,1,2,3].map(function (i) { return avg('tyre_temp_left', i); }),
      tyre_temp_center:  [0,1,2,3].map(function (i) { return avg('tyre_temp_center', i); }),
      tyre_temp_right:   [0,1,2,3].map(function (i) { return avg('tyre_temp_right', i); }),
      tyre_wear:         [0,1,2,3].map(function (i) { return avg('tyre_wear', i); }),
      tyre_pressure:     [0,1,2,3].map(function (i) { return avg('tyre_pressure', i); }),
      brake_temp:        [0,1,2,3].map(function (i) { return avg('brake_temp', i); }),
      suspension_travel: [0,1,2,3].map(function (i) { return avg('suspension_travel', i); }),
    };
  }

  function lerpColor(a, b, t) {
    function h2r(h) { var n = parseInt(h.slice(1), 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; }
    var ca = h2r(a), cb = h2r(b);
    return 'rgb(' + Math.round(ca[0] + (cb[0] - ca[0]) * t) + ',' +
                    Math.round(ca[1] + (cb[1] - ca[1]) * t) + ',' +
                    Math.round(ca[2] + (cb[2] - ca[2]) * t) + ')';
  }

  function tyreColor(t) {
    if (t <= 5)   return '#12121e';
    if (t < 60)   return '#1a5276';
    if (t < 75)   return lerpColor('#1a5276', '#1e8449', (t - 60) / 15);
    if (t <= 100) return '#1e8449';
    if (t < 115)  return lerpColor('#1e8449', '#b7950b', (t - 100) / 15);
    if (t < 130)  return lerpColor('#b7950b', '#7b241c', (t - 115) / 15);
    return '#7b241c';
  }

  function brakeColor(t) {
    if (t <= 5)   return '#12121e';
    if (t < 200)  return '#1a5276';
    if (t < 350)  return lerpColor('#1a5276', '#1e8449', (t - 200) / 150);
    if (t <= 700) return '#1e8449';
    if (t < 850)  return lerpColor('#1e8449', '#b7950b', (t - 700) / 150);
    if (t < 1000) return lerpColor('#b7950b', '#7b241c', (t - 850) / 150);
    return '#7b241c';
  }

  // Returns [inner, mid, outer] temps for wheel i.
  // FL(0), RL(2) are left wheels: left-edge = outer, right-edge = inner.
  // FR(1), RR(3) are right wheels: left-edge = inner, right-edge = outer.
  function tyreIMO(tel, i) {
    var L = tel.tyre_temp_left[i], M = tel.tyre_temp_center[i], R = tel.tyre_temp_right[i];
    return (i === 0 || i === 2) ? [R, M, L] : [L, M, R];
  }

  function setupRecommendations(avg) {
    var recs = [], WN = ['FL', 'FR', 'RL', 'RR'];
    for (var i = 0; i < 4; i++) {
      var imo = tyreIMO(avg, i);
      var inn = imo[0], mid = imo[1], out = imo[2];
      if (inn < 5 && mid < 5 && out < 5) continue;
      var avgT = (inn + mid + out) / 3, name = WN[i];
      var camberDiff = inn - out;
      if (camberDiff > 20)        recs.push({ warn: true,  txt: name + ': inner ' + Math.round(camberDiff) + '\u00b0C hotter \u2014 reduce negative camber' });
      else if (camberDiff < -10)  recs.push({ warn: false, txt: name + ': outer ' + Math.round(-camberDiff) + '\u00b0C hotter \u2014 increase negative camber' });
      var pressDiff = mid - (inn + out) / 2;
      if (pressDiff > 15)         recs.push({ warn: true,  txt: name + ': centre tread ' + Math.round(pressDiff) + '\u00b0C hotter than edges \u2014 reduce tyre pressure' });
      else if (pressDiff < -12)   recs.push({ warn: false, txt: name + ': edges hotter than centre \u2014 increase tyre pressure' });
      if (avgT > 115)             recs.push({ warn: true,  txt: name + ': overheating (' + Math.round(avgT) + '\u00b0C) \u2014 consider harder compound' });
      else if (avgT > 5 && avgT < 65) recs.push({ warn: false, txt: name + ': undertemp (' + Math.round(avgT) + '\u00b0C) \u2014 softer compound or more aggressive warm-up' });
    }
    var bf = (avg.brake_temp[0] + avg.brake_temp[1]) / 2;
    var br = (avg.brake_temp[2] + avg.brake_temp[3]) / 2;
    for (var i = 0; i < 4; i++) {
      var bt = avg.brake_temp[i];
      if (bt <= 5) continue;
      if (bt > 900) recs.push({ warn: true,  txt: WN[i] + ' brakes: ' + Math.round(bt) + '\u00b0C \u2014 open ducts or reduce bias to this axle' });
      else if (bt < 150) recs.push({ warn: false, txt: WN[i] + ' brakes: ' + Math.round(bt) + '\u00b0C \u2014 close ducts or increase bias to this axle' });
    }
    if (bf > 50 && br > 50) {
      var bDiff = bf - br;
      if (bDiff > 250)        recs.push({ warn: false, txt: 'Brake bias: fronts ' + Math.round(bDiff) + '\u00b0C hotter \u2014 move bias rearward' });
      else if (bDiff < -200)  recs.push({ warn: false, txt: 'Brake bias: rears ' + Math.round(-bDiff) + '\u00b0C hotter \u2014 move bias forward' });
    }
    for (var i = 0; i < 4; i++) {
      var st = avg.suspension_travel[i];
      if (st > 0.075) recs.push({ warn: true, txt: WN[i] + ': high susp. travel (' + (st * 1000).toFixed(0) + ' mm) \u2014 raise ride height or stiffen springs' });
    }
    var fs = (avg.suspension_travel[0] + avg.suspension_travel[1]) / 2;
    var rs = (avg.suspension_travel[2] + avg.suspension_travel[3]) / 2;
    if (fs > 0.005 && rs > 0.005) {
      var sDiff = fs - rs;
      if (sDiff > 0.025)        recs.push({ warn: false, txt: 'Front travels ' + (sDiff * 1000).toFixed(0) + ' mm more than rear \u2014 stiffen front or soften rear springs' });
      else if (sDiff < -0.025)  recs.push({ warn: false, txt: 'Rear travels ' + (-sDiff * 1000).toFixed(0) + ' mm more than front \u2014 stiffen rear or soften front springs' });
    }
    return recs;
  }

  function buildDriverPanel(tel, avgTel) {
    var WN = ['FL', 'FR', 'RL', 'RR'];
    function tyreCard(i) {
      var imo = tyreIMO(tel, i);
      var temps = ['In', 'Mid', 'Out'].map(function (lbl, j) {
        var t = imo[j], ok = t > 5;
        return '<div class="tc-cell" style="background:' + tyreColor(t) + '">' +
          '<div class="tc-cell-lbl">' + lbl + '</div>' +
          '<div class="tc-cell-val">' + (ok ? Math.round(t) + '\u00b0' : '\u2014') + '</div>' +
          '</div>';
      }).join('');
      var psi = tel.tyre_pressure[i], wear = tel.tyre_wear[i];
      var bt = tel.brake_temp[i], st = tel.suspension_travel[i];
      var wClr = wear < 0.5 ? '#27ae60' : wear < 0.75 ? '#f39c12' : '#e74c3c';
      return '<div class="tc-card">' +
        '<div class="tc-label">' + WN[i] + '</div>' +
        '<div class="tc-temps">' + temps + '</div>' +
        '<div class="tc-meta">' +
          '<span class="tc-psi">' + (psi > 0 ? psi.toFixed(1) + ' PSI' : '\u2014') + '</span>' +
          '<span class="tc-wear" style="color:' + wClr + '">' + (wear > 0 ? Math.round(wear * 100) + '% wear' : '') + '</span>' +
        '</div>' +
        '<div class="tc-chassis">' +
          '<span class="tc-brake" style="background:' + brakeColor(bt) + '">' + (bt > 5 ? Math.round(bt) + '\u00b0C' : '\u2014') + '</span>' +
          '<span class="tc-susp">' + (st > 0 ? (st * 1000).toFixed(0) + ' mm' : '\u2014') + '</span>' +
        '</div></div>';
    }
    var grid = '<div class="tc-grid">' +
      '<div class="tc-row">' + tyreCard(0) + tyreCard(1) + '</div>' +
      '<div class="tc-row">' + tyreCard(2) + tyreCard(3) + '</div>' +
    '</div>';
    var recs = setupRecommendations(avgTel);
    var recHtml = recs.length
      ? recs.map(function (r) { return '<li class="' + (r.warn ? 'rec-warn' : 'rec-info') + '">' + esc(r.txt) + '</li>'; }).join('')
      : '<li class="rec-ok">\u2713 No significant issues detected</li>';
    return '<div class="setup-legend"><span>Tyres &amp; Brakes</span>' +
        '<span class="leg-cold">Cold</span><span class="leg-ok">OK</span><span class="leg-hot">Hot</span>' +
      '</div>' + grid +
      '<div class="setup-recs"><h4>Recommendations</h4>' +
        '<ul class="recs-list">' + recHtml + '</ul></div>';
  }

  function updateSetupPanel(d) {
    var panel = document.getElementById('setup-panel');
    if (!panel) return;

    var tel = d.player_telemetry;
    if (!d.connected || !tel) {
      panel.innerHTML = '<div class="setup-no-data">Connect to AMS2 to see telemetry.</div>';
      telBuf = []; telLastViewedName = null;
      return;
    }

    // Identify viewed driver; fall back to last known when in garage (viewed_idx = -1)
    var viewed = null;
    for (var pi = 0; pi < d.participants.length; pi++) { if (d.participants[pi].is_player) { viewed = d.participants[pi]; break; } }
    var viewedName = viewed ? viewed.name : null;
    if (viewedName) telLastViewedName = viewedName;
    var driverName = viewedName || telLastViewedName;

    // Accumulate
    telBuf.push(tel);
    if (telBuf.length > TEL_BUF_SIZE) telBuf.shift();

    // Skip DOM update when tab hidden
    var subPanel = document.getElementById('live-sub-setup');
    if (subPanel && subPanel.classList.contains('live-subpanel-hidden')) return;

    var label = driverName ? '<div class="setup-driver-name">' + esc(driverName) + '</div>' : '';
    panel.innerHTML = label + buildDriverPanel(tel, computeAvgTel(telBuf));
  }

  // ── Live Session polling ───────────────────────────────────────────────────
  var liveTimer = null;
  var topSpeeds   = {};   // name → peak km/h this session
  var lastPosPoll = {};   // name → {x, z, t}
  var liveTrack   = null; // track name at last poll, used to reset speed data

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
          liveBody.innerHTML = '<tr><td colspan="10" class="live-empty">Waiting for session data\u2026</td></tr>';
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
          var prev = lastPosPoll[p.name];
          if (prev) {
            var dt = (now - prev.t) / 1000;
            if (dt > 0) {
              var dx = p.world_pos_x - prev.x, dz = p.world_pos_z - prev.z;
              var kmh = Math.sqrt(dx * dx + dz * dz) / dt * 3.6;
              if (!(p.name in topSpeeds) || kmh > topSpeeds[p.name]) topSpeeds[p.name] = kmh;
            }
          }
          lastPosPoll[p.name] = { x: p.world_pos_x, z: p.world_pos_z, t: now };
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
            '<td class="live-num">'  + (topSpeeds[p.name] ? Math.round(topSpeeds[p.name]) : '\u2014') + '</td>' +
            '</tr>';
        }).join('');
        applyLiveSort();
        tmUpdate(d);
        updateSetupPanel(d);
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

  function careerComputeConstructors(champ, sessions) {
    var pts = {}, wins = {};
    (champ.rounds || []).forEach(function (round) {
      (round.session_ids || []).forEach(function (sid) {
        var s = sessions.find(function (s) { return s.id === sid; });
        if (!s || s.session_type !== 5) return;
        s.results.forEach(function (r) {
          var key = r.car_name || r.car_class;
          if (!key) return;
          if (!pts[key]) { pts[key] = 0; wins[key] = 0; }
          if (!r.dnf) {
            pts[key] += champ.points_system[r.race_position - 1] || 0;
            if (r.race_position === 1) wins[key]++;
          }
        });
      });
    });
    return Object.keys(pts).map(function (name) {
      return { name: name, points: pts[name], wins: wins[name] };
    }).sort(function (a, b) { return b.points - a.points || b.wins - a.wins; });
  }

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

  function renderCareerChampionships(champs, sessions) {
    var container = document.getElementById('career-container');
    if (!container) return;
    if (!champs.length) {
      container.innerHTML = '<div class="manage-placeholder" style="padding:2rem">No championships yet \u2014 create one in the Manage tab.</div>';
      return;
    }
    container.innerHTML = champs.map(function (champ, idx) {
      var standings     = careerComputeStandings(champ, sessions);
      var constructors  = careerComputeConstructors(champ, sessions);
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
          '<div class="standings-panel"><h3>Driver Standings</h3>' + careerStandingsHtml(standings) + '</div>' +
          (constructors.length ? '<div class="standings-panel"><h3>Constructor Standings</h3>' + careerConstructorsHtml(constructors) + '</div>' : '') +
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

  function fmtTrack(s) {
    return s.track_variation ? esc(s.track) + ' \u2013 ' + esc(s.track_variation) : esc(s.track);
  }

  function fmtDate(ts) {
    if (!ts) return '';
    var d = new Date(ts * 1000);
    return d.getFullYear() + '-' +
      String(d.getMonth() + 1).padStart(2, '0') + '-' +
      String(d.getDate()).padStart(2, '0') + ' ' +
      String(d.getHours()).padStart(2, '0') + ':' +
      String(d.getMinutes()).padStart(2, '0');
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
                '<span class="session-track">' + fmtTrack(s) + '</span>' +
                '<span class="session-date">' + fmtDate(s.recorded_at) + '</span>' +
                (s.car_name ? '<span class="session-car">' + esc(s.car_name) + '</span>' : '') +
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
          '<span class="session-track">' + fmtTrack(s) + '</span>' +
          '<span class="session-date">' + fmtDate(s.recorded_at) + '</span>' +
          (s.car_name ? '<span class="session-car">' + esc(s.car_name) + '</span>' : '') +
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
        .then(function () { loadManage(); });
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
