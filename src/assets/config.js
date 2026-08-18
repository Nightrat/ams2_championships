// ── Config tab ────────────────────────────────────────────────────────────────

var _loadedConfig = null;  // last config fetched from server

function applyTrackMapConfig(cfg) {
  var canvas = document.getElementById('track-map');
  if (canvas) canvas.style.display = cfg.show_track_map ? '' : 'none';
  if (typeof TM_MAX !== 'undefined') TM_MAX = cfg.track_map_max_points;
}

function loadConfig() {
  fetch('/api/config').then(function (r) { return r.json(); })
    .then(function (cfg) {
      _loadedConfig = cfg;
      document.getElementById('cfg-port').value       = cfg.port;
      document.getElementById('cfg-host').value       = cfg.host;
      document.getElementById('cfg-saves-dir').value = cfg.saves_dir || '';
      document.getElementById('cfg-custom-ai-dir').value = cfg.custom_ai_dir || '';
      document.getElementById('cfg-poll-ms').value    = cfg.poll_ms;
      document.getElementById('cfg-record-practice').checked = cfg.record_practice;
      document.getElementById('cfg-record-qualify').checked  = cfg.record_qualify;
      document.getElementById('cfg-record-race').checked     = cfg.record_race;
      document.getElementById('cfg-enforce-team-eligibility').checked = cfg.enforce_team_eligibility;
      document.getElementById('cfg-show-track-map').checked        = cfg.show_track_map;
      document.getElementById('cfg-track-map-max-points').value    = cfg.track_map_max_points;
      applyTrackMapConfig(cfg);
      setConfigMsg('');
    })
    .catch(function () { setConfigMsg('Failed to load config.', true); });
}

// Apply track map visibility on page load
document.addEventListener('DOMContentLoaded', function () {
  fetch('/api/config').then(function (r) { return r.json(); })
    .then(function (cfg) { applyTrackMapConfig(cfg); })
    .catch(function () {});
});

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
    show_track_map:       document.getElementById('cfg-show-track-map').checked,
    track_map_max_points: parseInt(document.getElementById('cfg-track-map-max-points').value, 10),
  };
  saveConfig(newCfg);
});

document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () {
    if (btn.dataset.tab === 'config') loadConfig();
  });
});
