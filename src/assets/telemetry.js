// ── Damage panel ──────────────────────────────────────────────────────────────
// Everything here is raw shared-memory state for the player's car, shown as it
// is read. No smoothing, no rolling average: the point of this panel is to say
// what AMS2 reports, so a wrong offset shows up as a wrong number. The one thing
// it does hold on to is the last on-track reading, kept while the car is in the
// box (see dmgLastOnTrack).

var CRASH_STATES = [
  'None',
  'Off track',
  'Hit scenery',
  'Spinning',
  'Rolling'
];

var DMG_WHEELS = ['FL', 'FR', 'RL', 'RR'];

// mPitMode, as a reason to hold the panel. Index 0 is only ever reached via
// in_pits, which flips at the pit entry a moment before mPitMode does.
var PIT_MODES = [
  'In pits',
  'Entering pits',
  'In the pit box',
  'Leaving the pits',
  'In the garage',
  'Leaving the garage'
];

// Last reading taken while the car was out on track. The panel holds this while
// the player is in the pit lane or the garage, so a damage figure cannot quietly
// change under a repair — and it is what the player wants to read in the box.
var dmgLastOnTrack = null;
// The held-reason currently painted, or null while the panel is live. A held
// panel has nothing new to say each poll, and rewriting innerHTML anyway would
// drop any text selection — but the reason itself changes on the way in, so it
// is the label rather than a flag that decides whether to repaint.
var dmgFrozenLabel = null;

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

// The extremes of four per-corner values, as {low: {v, i}, high: {v, i}}, or
// null when nothing is filled. Extremes rather than a mean: the lowest corner
// is the one that bottoms out and the pair together give the rake, both of
// which an average reads tidier by hiding. Corners at zero are skipped rather
// than winning the minimum — zero is unset, not a car resting on its floor,
// and whether AMS2 fills all four is not settled. With one corner filled, low
// and high are the same corner, which is the honest answer.
function rideExtremes(arr) {
  var out = null;
  for (var i = 0; i < 4; i++) {
    var v = arr[i];
    if (!(v > 0)) continue;
    if (!out) { out = { low: { v: v, i: i }, high: { v: v, i: i } }; continue; }
    if (v < out.low.v) out.low = { v: v, i: i };
    if (v > out.high.v) out.high = { v: v, i: i };
  }
  return out;
}

// ── Ride height over the run ──────────────────────────────────────────────────
// The instantaneous reading says what the car is doing now; the lowest value it
// ever reached is what says how much floor there was left, which is the number a
// setup is chosen against. So the extremes are accumulated per corner across the
// run — front and rear ride height are separate setup values, so an overall
// minimum alone would not say which end to lower.
//
// `min`/`max` hold 0 for "nothing recorded", the same unset convention the rest
// of the panel uses.
var rideSeen = null;
// The session the readings belong to. A new track is a new run, and carrying a
// figure across one would quietly answer a question about Monza with a lap of
// Bathurst.
var rideSeenKey = null;

function resetRideSeen() {
  rideSeen = null;
  rideSeenKey = null;
}

// Fold one on-track sample into the run's extremes. Called from the point where
// the last on-track reading is kept — deliberately before the panel's
// visibility check, because these accumulate while the driver is driving rather
// than while the tab happens to be open.
function recordRideHeight(d, tel) {
  var key = (d.track_location || '') + '|' + (d.track_variation || '');
  if (key !== rideSeenKey) {
    rideSeen = { min: [0, 0, 0, 0], max: [0, 0, 0, 0] };
    rideSeenKey = key;
  }
  for (var i = 0; i < 4; i++) {
    var v = tel.ride_height[i];
    if (!(v > 0)) continue;
    if (!(rideSeen.min[i] > 0) || v < rideSeen.min[i]) rideSeen.min[i] = v;
    if (v > rideSeen.max[i]) rideSeen.max[i] = v;
  }
}

// One label/value pair for a corner reading, with the corner name as a dimmed
// qualifier on the value rather than a pair of its own.
function rideHeightPair(label, c) {
  return '<span class="dmg-contact-lbl">' + esc(label) + '</span>' +
    '<span class="dmg-contact-val">' +
      (c ? c.v.toFixed(2) + ' cm <span class="dmg-corner">' + DMG_WHEELS[c.i] + '</span>' : '&mdash;') +
    '</span>';
}

function buildDamagePanel(tel, frozen) {
  var crash = tel.crash_state || 0;
  var crashTxt = CRASH_STATES[crash] || ('Unknown (' + crash + ')');
  var crashCls = crash === 0 ? 'dmg-state-ok' : 'dmg-state-bad';

  // How hard the last car-to-car contact was. This registers a tap that does no
  // damage at all, which none of the 0–1 values do. The index is a raw
  // participant slot — the live table is sorted by position, so it is shown as
  // the number the game gives rather than resolved to a name.
  var hitIdx = tel.last_collision_index;
  // Sits with the damage figures because it is read for the same reason they
  // are: how much car is left. The per-corner values stay on the tyre cards.
  var rh = rideExtremes(tel.ride_height);
  // The run's extremes: the lowest any corner has been, and the highest. Taken
  // from the two recorded arrays rather than one, so "lowest" is the deepest
  // any corner reached and "highest" the most any corner ever had.
  var seenLow = rideSeen ? (rideExtremes(rideSeen.min) || {}).low : null;
  var seenHigh = rideSeen ? (rideExtremes(rideSeen.max) || {}).high : null;

  return '<h3 class="tel-section">Damage</h3>' +
    '<div class="dmg-head">' +
      '<span class="dmg-state ' + crashCls + '">' + esc(crashTxt) + '</span>' +
      '<span class="dmg-head-meta">mCrashState = ' + crash + '</span>' +
      (frozen ? '<span class="dmg-frozen">' + esc(frozen) + ' — last on-track reading</span>' : '') +
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
      rideHeightPair('Ride height now', rh && rh.low) +
      rideHeightPair('to', rh && rh.high) +
    '</div>' +
    '<div class="dmg-contact">' +
      '<span class="dmg-contact-lbl">Lowest this run</span>' +
      '<span class="dmg-contact-val dmg-seen-low">' +
        (seenLow ? seenLow.v.toFixed(2) + ' cm <span class="dmg-corner">' + DMG_WHEELS[seenLow.i] + '</span>' : '&mdash;') +
      '</span>' +
      rideHeightPair('highest', seenHigh) +
      '<button type="button" class="dmg-reset" data-ride-reset="1">Reset</button>' +
    '</div>';
}

function updateSetupPanel(d) {
  var panel = document.getElementById('setup-panel');
  if (!panel) return;

  var tel = d.player_telemetry;
  if (!d.connected || !tel) {
    panel.innerHTML = '<div class="setup-no-data">Connect to AMS2 to see telemetry.</div>';
    dmgLastOnTrack = null;
    dmgFrozenLabel = null;
    // The run ended with the connection, and the next one is a different car on
    // a different setup.
    resetRideSeen();
    return;
  }

  // Freeze whenever the player is not out on track. Two signals, because
  // neither alone catches every way off it:
  //   pit_mode (mPitMode)  — 0 is the only value that means "out on track". This
  //     is what catches ESC -> "Return to pits", which teleports the car into
  //     the garage without it ever driving down a pit lane.
  //   in_pits (mCurrentSector < 0) — the participant-level view, kept because it
  //     flips as soon as the car crosses the pit entry.
  // A viewed driver that cannot be found at all is the garage too.
  var viewed = null;
  for (var pi = 0; pi < d.participants.length; pi++) {
    if (d.participants[pi].is_player) { viewed = d.participants[pi]; break; }
  }
  var onTrack = !!viewed && !viewed.in_pits && (d.pit_mode || 0) === 0;
  var frozenWhy = onTrack ? null : (PIT_MODES[d.pit_mode || 0] || 'In pits');
  if (onTrack) {
    dmgLastOnTrack = tel;
    // Before the visibility check below on purpose: the extremes are a record
    // of the run, and a lap driven with the Live tab closed is still a lap.
    recordRideHeight(d, tel);
  }

  // Nothing has been read on track yet, so there is no reading to hold.
  var shown = onTrack ? tel : dmgLastOnTrack;
  if (!shown) {
    panel.innerHTML = '<div class="setup-no-data">Waiting for the car to go out on track&hellip;</div>';
    return;
  }

  // Skip DOM update when the sub-tab is hidden, or when the held reading is
  // already on screen.
  var subPanel = document.getElementById('live-sub-setup');
  if (subPanel && subPanel.classList.contains('live-subpanel-hidden')) return;
  if (frozenWhy && frozenWhy === dmgFrozenLabel) return;

  panel.innerHTML = buildDamagePanel(shown, frozenWhy) + buildTyrePanel(shown);
  dmgFrozenLabel = frozenWhy;
}

// ── Tyres ─────────────────────────────────────────────────────────────────────

// mTerrain. The list is the header's, in order, so the index is the value.
var TERRAIN_NAMES = [
  'Road', 'Low-grip road', 'Bumpy road 1', 'Bumpy road 2', 'Bumpy road 3',
  'Marbles', 'Grassy berms', 'Grass', 'Gravel', 'Bumpy gravel',
  'Rumble strips', 'Drains', 'Tyre walls', 'Cement walls', 'Guard rails',
  'Sand', 'Bumpy sand', 'Dirt', 'Bumpy dirt', 'Dirt road',
  'Bumpy dirt road', 'Pavement', 'Dirt bank', 'Wood', 'Dry verge',
  'Exit rumble strips', 'Grasscrete', 'Long grass', 'Slope grass', 'Cobbles',
  'Sand road', 'Baked clay', 'Astroturf', 'Snow (half)', 'Snow (full)',
  'Damaged road', 'Train track road', 'Bumpy cobbles', 'Aries only', 'Orion only',
  'B1 rumbles', 'B2 rumbles', 'Rough sand (medium)', 'Rough sand (heavy)', 'Snow walls',
  'Ice road', 'Runoff road', 'Illegal strip', 'Painted concrete',
  'Painted concrete (illegal)', 'Rally tarmac'
];

// mTyreFlags is a bitfield, not an enum.
function tyreFlagText(f) {
  var on = [];
  if (f & 1) on.push('attached');
  if (f & 2) on.push('inflated');
  if (f & 4) on.push('on ground');
  return on.length ? on.join(', ') : 'none';
}

// A labelled value line. `unset` blanks the reading rather than printing a zero
// that reads as a measurement.
function telRow(label, txt, unset) {
  return '<div class="tel-row">' +
    '<span class="tel-row-lbl">' + esc(label) + '</span>' +
    '<span class="tel-row-val' + (unset ? ' tel-unset' : '') + '">' + (unset ? '—' : txt) + '</span>' +
    '</div>';
}

function degRow(label, v) {
  return telRow(label, Math.round(v) + '°C', !(v > 0));
}

function tyreCard(tel, i) {
  var compound = tel.tyre_compound[i];
  return '<div class="tel-card">' +
    '<div class="tel-card-head">' +
      '<span class="tel-card-name">' + DMG_WHEELS[i] + '</span>' +
      '<span class="tel-card-compound">' + (compound ? esc(compound) : '—') + '</span>' +
    '</div>' +

    '<div class="tel-group">Surface temperature</div>' +
    degRow('Left', tel.tyre_temp_left[i]) +
    degRow('Centre', tel.tyre_temp_center[i]) +
    degRow('Right', tel.tyre_temp_right[i]) +
    degRow('Overall', tel.tyre_temp[i]) +

    '<div class="tel-group">Structure temperature</div>' +
    degRow('Tread', tel.tyre_tread_temp[i]) +
    degRow('Layer', tel.tyre_layer_temp[i]) +
    degRow('Carcass', tel.tyre_carcass_temp[i]) +
    degRow('Rim', tel.tyre_rim_temp[i]) +
    degRow('Internal air', tel.tyre_internal_air_temp[i]) +

    '<div class="tel-group">Condition</div>' +
    telRow('Wear', (tel.tyre_wear[i] * 100).toFixed(1) + '%', false) +
    telRow('Pressure', tel.tyre_pressure[i].toFixed(2) + ' PSI', !(tel.tyre_pressure[i] > 0)) +
    telRow('Rotation', tel.tyre_rps[i].toFixed(2) + ' rev/s', false) +

    '<div class="tel-group">Contact</div>' +
    telRow('Terrain', esc(TERRAIN_NAMES[tel.terrain[i]] || ('#' + tel.terrain[i])), false) +
    telRow('State', tyreFlagText(tel.tyre_flags[i]), false) +
    telRow('Height above ground', tel.tyre_height_above_ground[i].toFixed(4), false) +
    telRow('Tyre Y', tel.tyre_y[i].toFixed(4), false) +

    '<div class="tel-group">Corner</div>' +
    telRow('Brake temp', Math.round(tel.brake_temp[i]) + '°C', !(tel.brake_temp[i] > 0)) +
    telRow('Ride height', (tel.ride_height[i]).toFixed(2) + ' cm', false) +
    // This corner's own extremes. The overall figures on the damage panel say
    // how much floor was left; these say at which end, which is what decides
    // whether it is the front or the rear ride height that can come down.
    telRow('Seen this run',
      rideSeen && rideSeen.min[i] > 0
        ? rideSeen.min[i].toFixed(2) + ' &ndash; ' + rideSeen.max[i].toFixed(2) + ' cm'
        : '', !(rideSeen && rideSeen.min[i] > 0)) +
    telRow('Susp. travel', (tel.suspension_travel[i] * 1000).toFixed(1) + ' mm', false) +
    telRow('Susp. velocity', tel.suspension_velocity[i].toFixed(3) + ' m/s', false) +
    telRow('Wheel Y', tel.wheel_local_position_y[i].toFixed(4), false) +
    '</div>';
}

function buildTyrePanel(tel) {
  var cards = [0, 1, 2, 3].map(function (i) { return tyreCard(tel, i); });
  // The header marks these three "kept for backward compatibility only". They are
  // shown so that the question of whether AMS2 still fills them is answered here
  // rather than re-investigated every time someone wants slip or grip data.
  var obsolete = ['tyre_slip_speed', 'tyre_grip', 'tyre_lateral_stiffness'].map(function (f) {
    return '<span class="tel-obs-item"><b>' + f.replace(/^tyre_/, '') + '</b> ' +
      tel[f].map(function (v) { return v.toFixed(3); }).join(' / ') + '</span>';
  }).join('');
  return '<h3 class="tel-section">Tyres</h3>' +
    '<div class="tel-grid">' +
      '<div class="tel-row-pair">' + cards[0] + cards[1] + '</div>' +
      '<div class="tel-row-pair">' + cards[2] + cards[3] + '</div>' +
    '</div>' +
    '<div class="tel-obsolete"><span class="tel-obs-lbl">Obsolete in the PCars2 header (FL / FR / RL / RR)</span>' +
      obsolete + '</div>';
}

// Reset the run's ride-height record. Delegated from document because the panel
// is rebuilt on every poll while the car is on track, so a listener bound to the
// button would be thrown away with it. Clearing dmgFrozenLabel forces the next
// poll to repaint: the button is most useful in the box between runs, and that
// is exactly when the panel is held and would otherwise not redraw.
document.addEventListener('click', function (e) {
  var btn = e.target.closest && e.target.closest('[data-ride-reset]');
  if (!btn) return;
  resetRideSeen();
  dmgFrozenLabel = null;
});
