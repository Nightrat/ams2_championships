// ── Config tab ────────────────────────────────────────────────────────────────

var _loadedConfig = null;  // last config fetched from server

function applyTrackMapConfig(cfg) {
  var canvas = document.getElementById('track-map');
  if (canvas) canvas.style.display = cfg.show_track_map ? '' : 'none';
  if (typeof TM_MAX !== 'undefined') TM_MAX = cfg.track_map_max_points;
}

// ── Class season years ────────────────────────────────────────────────────────
// One box per class, each holding the *override* only: the built-in year sits in the
// placeholder, so an empty box reads as "whatever the app says" and clearing one is how you
// go back to it. The server decides what an override is worth keeping — a year equal to the
// built-in one is dropped there, not here.

var _classYearBounds = { min: 1900, max: 2100 };  // replaced by whatever /api/config reports

function renderClassYears(classes, yearMin, yearMax) {
  var host = document.getElementById('cfg-class-years');
  if (!host) return;
  if (typeof yearMin === 'number') _classYearBounds.min = yearMin;
  if (typeof yearMax === 'number') _classYearBounds.max = yearMax;
  if (!classes || !classes.length) {
    host.innerHTML = '<span class="config-hint">Set the Custom AI Drivers folder above and save to list your classes here.</span>';
    return;
  }
  host.innerHTML = classes.map(function (c, i) {
    var id = 'cfg-class-year-' + i;
    return '<div class="config-year-row">' +
      '<label class="config-year-name" for="' + id + '">' + esc(c.class) + '</label>' +
      '<input class="config-input config-input-year" type="number" step="1"' +
      ' min="' + _classYearBounds.min + '" max="' + _classYearBounds.max + '"' +
      ' id="' + id + '" data-class="' + esc(c.class) + '"' +
      ' value="' + (c.overridden && c.year != null ? c.year : '') + '"' +
      ' placeholder="' + (c.builtin != null ? c.builtin : '—') + '" />' +
      '</div>';
  }).join('');
}

// The overrides the boxes are currently showing. Blank boxes are simply absent, which is what
// makes clearing one remove it.
function collectClassYears() {
  var out = {};
  document.querySelectorAll('#cfg-class-years input[data-class]').forEach(function (el) {
    var v = parseInt(el.value, 10);
    if (!isNaN(v)) out[el.getAttribute('data-class')] = v;
  });
  return out;
}

// An empty Custom AI Drivers folder means this install has no rosters at all, and everything
// singleplayer is downstream of one: no rating, no team requirement, no offer, no money, and the
// live grid falls back to AMS2's car models. So it is flagged twice — beside the box that fixes
// it, and as a "!" on the Config tab itself, since someone who has never opened the tab is
// exactly the person who has not set it.
function flagMissingAiDir(dir) {
  var missing = !(dir && String(dir).trim());
  var notice = document.getElementById('cfg-custom-ai-dir-missing');
  var badge = document.getElementById('tab-config-warn');
  if (notice) notice.hidden = !missing;
  if (badge) badge.hidden = !missing;
}

function loadConfig() {
  fetch('/api/config').then(function (r) { return r.json(); })
    .then(function (cfg) {
      _loadedConfig = cfg;
      document.getElementById('cfg-port').value       = cfg.port;
      document.getElementById('cfg-host').value       = cfg.host;
      document.getElementById('cfg-saves-dir').value = cfg.saves_dir || '';
      document.getElementById('cfg-custom-ai-dir').value = cfg.custom_ai_dir || '';
      flagMissingAiDir(cfg.custom_ai_dir);
      document.getElementById('cfg-poll-ms').value    = cfg.poll_ms;
      document.getElementById('cfg-record-practice').checked = cfg.record_practice;
      document.getElementById('cfg-record-qualify').checked  = cfg.record_qualify;
      document.getElementById('cfg-record-race').checked     = cfg.record_race;
      document.getElementById('cfg-enforce-team-eligibility').checked = cfg.enforce_team_eligibility;
      document.getElementById('cfg-hide-locked-teams').checked = cfg.hide_locked_teams;
      document.getElementById('cfg-contract-top-salary').value = cfg.contract_top_salary;
      document.getElementById('cfg-contract-floor-salary').value = cfg.contract_floor_salary;
      document.getElementById('cfg-contract-buy-in').value = cfg.contract_buy_in_per_point;
      document.getElementById('cfg-champion-prize').value = cfg.champion_prize;
      document.getElementById('cfg-last-place-prize').value = cfg.last_place_prize;
      document.getElementById('cfg-starting-balance').value = cfg.starting_balance;
      document.getElementById('cfg-starting-rating').value   = cfg.starting_rating;
      document.getElementById('cfg-rating-strictness').value = cfg.rating_strictness;
      document.getElementById('cfg-offer-margin').value = cfg.offer_margin;
      document.getElementById('cfg-eligibility-gates').value = cfg.eligibility_gates;
      document.getElementById('cfg-rating-half-life').value  = cfg.rating_half_life;
      document.getElementById('cfg-count-retirements').checked = cfg.count_retirements;
      document.getElementById('cfg-retirement-min-laps-down').value = cfg.retirement_min_laps_down;
      document.getElementById('cfg-retirement-distance-pct').value = cfg.retirement_distance_pct;
      document.getElementById('cfg-show-track-map').checked        = cfg.show_track_map;
      document.getElementById('cfg-track-map-max-points').value    = cfg.track_map_max_points;
      applyTrackMapConfig(cfg);
      renderClassYears(cfg.classes, cfg.year_min, cfg.year_max);
      // The career runs on the tuning it was created with, so say so when the two have parted.
      var diverged = document.getElementById('cfg-rating-diverged');
      if (diverged) diverged.hidden = cfg.career_rating_matches !== false;
      setConfigMsg('');
    })
    .catch(function () { setConfigMsg('Failed to load config.', true); });
}

// Move the active career onto the rating settings now in config.json. Deliberate, and confirmed,
// because it re-judges every season the career has already raced.
function adoptRatingSettings() {
  var msg = document.getElementById('cfg-rating-adopt-msg');
  if (!confirm('Move this career onto the driver rating settings in Config?\n\n' +
      'Every rating and every team requirement is worked out afresh, including for seasons ' +
      'already raced, so a seat you earned may read differently afterwards.')) return;
  fetch('/api/career/rating/adopt', { method: 'POST' })
    .then(function (r) { return r.ok ? r.json() : r.text().then(function (t) { throw new Error(t); }); })
    .then(function () {
      if (msg) msg.textContent = 'This career now uses the settings above.';
      loadConfig();
    })
    .catch(function () { if (msg) msg.textContent = 'Could not apply the settings.'; });
}

// Apply track map visibility on page load
document.addEventListener('DOMContentLoaded', function () {
  fetch('/api/config').then(function (r) { return r.json(); })
    .then(function (cfg) { applyTrackMapConfig(cfg); })
    .catch(function () {});
});

// Numeric field value, falling back through the last loaded config to a literal default.
// parseFloat('') is NaN, which serde rejects outright — the form must never send one.
function numOr(id, loaded, fallback) {
  var el = document.getElementById(id);
  var v = el ? parseFloat(el.value) : NaN;
  if (!isNaN(v)) return v;
  return typeof loaded === 'number' ? loaded : fallback;
}

function setConfigMsg(msg, isError) {
  var el = document.getElementById('config-save-msg');
  if (!el) return;
  el.textContent = msg;
  el.className = 'config-save-msg' + (isError ? ' config-save-msg-error' : (msg ? ' config-save-msg-ok' : ''));
}

function saveConfig(newCfg) {
  fetch('/api/config', {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(newCfg),
  }).then(function (r) {
    return r.json().then(function (body) { return { ok: r.ok, body: body }; });
  })
    .then(function (r) {
      if (!r.ok) { setConfigMsg(r.body.error || 'Failed to save config.', true); return; }
      var res = r.body;
      _loadedConfig = res.config;
      applyTrackMapConfig(res.config);
      flagMissingAiDir(res.config.custom_ai_dir);
      // Redrawn from the response, not left as typed: the server drops an override that only
      // repeats the built-in year, and changing the Custom AI folder changes which classes there
      // are to answer for at all.
      renderClassYears(res.classes);
      var msgs = [];
      if (res.restart_required && res.restart_required.length) {
        msgs.push('Restart required for: ' + res.restart_required.join(', ') + '.');
      }
      setConfigMsg(msgs.length ? msgs.join(' ') : 'Saved.', false);
    })
    .catch(function () { setConfigMsg('Failed to save config.', true); });
}

document.getElementById('config-form').addEventListener('submit', function (e) {
  e.preventDefault();
  var newCfg = {
    port:           parseInt(document.getElementById('cfg-port').value, 10),
    host:           document.getElementById('cfg-host').value.trim(),
    saves_dir:      document.getElementById('cfg-saves-dir').value.trim() || null,
    custom_ai_dir:  document.getElementById('cfg-custom-ai-dir').value.trim() || null,
    poll_ms:        parseInt(document.getElementById('cfg-poll-ms').value, 10),
    record_practice:      document.getElementById('cfg-record-practice').checked,
    record_qualify:       document.getElementById('cfg-record-qualify').checked,
    record_race:          document.getElementById('cfg-record-race').checked,
    enforce_team_eligibility: document.getElementById('cfg-enforce-team-eligibility').checked,
    hide_locked_teams:    document.getElementById('cfg-hide-locked-teams').checked,
    // Money fields fall back the same way the rating ones do: a blank box must not wipe the
    // economy, and the server clamps whatever arrives.
    contract_top_salary:   Math.round(numOr('cfg-contract-top-salary', _loadedConfig && _loadedConfig.contract_top_salary, 4000000)),
    contract_floor_salary: Math.round(numOr('cfg-contract-floor-salary', _loadedConfig && _loadedConfig.contract_floor_salary, 200000)),
    contract_buy_in_per_point: Math.round(numOr('cfg-contract-buy-in', _loadedConfig && _loadedConfig.contract_buy_in_per_point, 150000)),
    champion_prize:        Math.round(numOr('cfg-champion-prize', _loadedConfig && _loadedConfig.champion_prize, 2000000)),
    last_place_prize:      Math.round(numOr('cfg-last-place-prize', _loadedConfig && _loadedConfig.last_place_prize, 50000)),
    starting_balance:      Math.round(numOr('cfg-starting-balance', _loadedConfig && _loadedConfig.starting_balance, 4050000)),
    // An empty or unparseable field falls back to the value the server last sent, so a blank
    // box cannot silently reset a rating to 0.
    starting_rating:      numOr('cfg-starting-rating', _loadedConfig && _loadedConfig.starting_rating, 50),
    rating_strictness:    numOr('cfg-rating-strictness', _loadedConfig && _loadedConfig.rating_strictness, 0),
    offer_margin:         numOr('cfg-offer-margin', _loadedConfig && _loadedConfig.offer_margin, 10),
    eligibility_gates:    document.getElementById('cfg-eligibility-gates').value,
    rating_half_life:     numOr('cfg-rating-half-life', _loadedConfig && _loadedConfig.rating_half_life, 10),
    count_retirements:    document.getElementById('cfg-count-retirements').checked,
    retirement_distance_pct: numOr('cfg-retirement-distance-pct', _loadedConfig && _loadedConfig.retirement_distance_pct, 90),
    // Rounded: this one is a u32 server-side, and serde rejects 3.5 outright with a 400.
    retirement_min_laps_down: Math.max(0, Math.round(
      numOr('cfg-retirement-min-laps-down', _loadedConfig && _loadedConfig.retirement_min_laps_down, 3))),
    show_track_map:       document.getElementById('cfg-show-track-map').checked,
    track_map_max_points: parseInt(document.getElementById('cfg-track-map-max-points').value, 10),
  };
  // Omitted entirely until the boxes have been filled from the server, so a form that has not
  // loaded cannot send an empty map and wipe overrides the user never saw.
  if (_loadedConfig) newCfg.class_years = collectClassYears();
  saveConfig(newCfg);
});

var _ratingAdoptBtn = document.getElementById('cfg-rating-adopt');
if (_ratingAdoptBtn) _ratingAdoptBtn.addEventListener('click', adoptRatingSettings);

document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () {
    if (btn.dataset.tab === 'config') loadConfig();
  });
});

// Read once at startup, only so the "!" on the Config tab is there for someone who has never
// opened it. Clicking the tab reloads anyway, so this costs one request and nothing else.
loadConfig();
