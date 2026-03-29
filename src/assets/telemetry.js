// ── Setup telemetry panel ─────────────────────────────────────────────────────
var telBuf            = [];   // rolling samples for the viewed driver
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

  telBuf.push(tel);
  if (telBuf.length > TEL_BUF_SIZE) telBuf.shift();

  // Skip DOM update when tab hidden
  var subPanel = document.getElementById('live-sub-setup');
  if (subPanel && subPanel.classList.contains('live-subpanel-hidden')) return;

  var label = driverName ? '<div class="setup-driver-name">' + esc(driverName) + '</div>' : '';
  panel.innerHTML = label + buildDriverPanel(tel, computeAvgTel(telBuf));
}
