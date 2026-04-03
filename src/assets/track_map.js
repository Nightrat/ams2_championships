// ── Track map ─────────────────────────────────────────────────────────────────
var trackMap = { track: null, points: null, cells: {}, accumulated: [], saved: false, loading: false };
var TM_CELL = 5;   // metres per grid cell for deduplication
var TM_MIN  = 300; // minimum unique cells before saving

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
        trackMap.saved  = data.length >= TM_MIN;
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
  var oz = (H - rangeZ * scale) / 2 + maxZ * scale;
  function cx(x) { return x * scale + ox; }
  function cz(z) { return -z * scale + oz; }

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
