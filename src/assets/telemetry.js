// ── Damage panel ──────────────────────────────────────────────────────────────
// Everything here is raw shared-memory state for the player's car, shown as it
// is read. No smoothing, no rolling buffer: the point of this panel is to say
// what AMS2 reports right now, so a wrong offset shows up as a wrong number.

var CRASH_STATES = [
  'None',
  'Off track',
  'Hit scenery',
  'Spinning',
  'Rolling'
];

var DMG_WHEELS = ['FL', 'FR', 'RL', 'RR'];

function dmgColor(v) {
  if (v <= 0.001) return '#27ae60';
  if (v < 0.25)   return '#f1c40f';
  if (v < 0.6)    return '#e67e22';
  return '#e74c3c';
}

// 0–1 as a percentage with a bar behind it.
function dmgBar(label, v) {
  var pct = Math.max(0, Math.min(1, v)) * 100;
  return '<div class="dmg-row">' +
    '<div class="dmg-row-lbl">' + esc(label) + '</div>' +
    '<div class="dmg-track"><div class="dmg-fill" style="width:' + pct.toFixed(1) + '%;background:' + dmgColor(v) + '"></div></div>' +
    '<div class="dmg-row-val" style="color:' + dmgColor(v) + '">' + pct.toFixed(1) + '%</div>' +
    '</div>';
}

function buildDamagePanel(tel) {
  var crash = tel.crash_state || 0;
  var crashTxt = CRASH_STATES[crash] || ('Unknown (' + crash + ')');
  var crashCls = crash === 0 ? 'dmg-state-ok' : 'dmg-state-bad';

  // How hard the last car-to-car contact was. This registers a tap that does no
  // damage at all, which none of the 0–1 values do. The index is a raw
  // participant slot — the live table is sorted by position, so it is shown as
  // the number the game gives rather than resolved to a name.
  var hitIdx = tel.last_collision_index;

  return '<div class="dmg-head">' +
      '<span class="dmg-state ' + crashCls + '">' + esc(crashTxt) + '</span>' +
      '<span class="dmg-head-meta">mCrashState = ' + crash + '</span>' +
    '</div>' +
    '<div class="dmg-cols">' +
      '<div class="dmg-block">' +
        '<h4>Car</h4>' +
        dmgBar('Aero / bodywork', tel.aero_damage) +
        dmgBar('Engine', tel.engine_damage) +
      '</div>' +
      '<div class="dmg-block">' +
        '<h4>Brakes</h4>' +
        DMG_WHEELS.map(function (w, i) { return dmgBar(w, tel.brake_damage[i]); }).join('') +
      '</div>' +
      '<div class="dmg-block">' +
        '<h4>Suspension</h4>' +
        DMG_WHEELS.map(function (w, i) { return dmgBar(w, tel.suspension_damage[i]); }).join('') +
      '</div>' +
    '</div>' +
    '<div class="dmg-contact">' +
      '<span class="dmg-contact-lbl">Last contact</span>' +
      '<span class="dmg-contact-val">' + (hitIdx >= 0 ? 'car #' + hitIdx : 'none') + '</span>' +
      '<span class="dmg-contact-lbl">magnitude</span>' +
      '<span class="dmg-contact-val">' + (tel.last_collision_magnitude || 0).toFixed(2) + '</span>' +
    '</div>';
}

function updateSetupPanel(d) {
  var panel = document.getElementById('setup-panel');
  if (!panel) return;

  var tel = d.player_telemetry;
  if (!d.connected || !tel) {
    panel.innerHTML = '<div class="setup-no-data">Connect to AMS2 to see damage.</div>';
    return;
  }

  // Skip DOM update when the sub-tab is hidden.
  var subPanel = document.getElementById('live-sub-setup');
  if (subPanel && subPanel.classList.contains('live-subpanel-hidden')) return;

  panel.innerHTML = buildDamagePanel(tel);
}
