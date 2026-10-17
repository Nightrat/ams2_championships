// ── Career save files ─────────────────────────────────────────────────────────
// One save = one career. Switching reloads the page, because every tab caches its
// data and only refetches when its own tab button is clicked.

var MODE_LABEL = {
  unset: 'Kind not set',
  singleplayer: 'Singleplayer',
  multiplayer: 'Multiplayer',
};

/// The active career's mode, so other tabs can tell what is allowed without refetching.
var careerMode = 'unset';

/// Everything a multiplayer career has no use for. Contracts, car performance and driver
/// performance all describe a Custom AI roster, and a multiplayer career does not have one —
/// leaving them in the tab bar offers the user pages that can only ever be empty.
var ROSTER_TABS = ['carperf', 'driverperf'];

function applyCareerMode() {
  var hide = careerMode === 'multiplayer';
  ROSTER_TABS.forEach(function (name) {
    var btn = document.querySelector('.tab-btn[data-tab="' + name + '"]');
    if (!btn) return;
    btn.style.display = hide ? 'none' : '';
    // Never leave the user looking at a tab that has just disappeared.
    if (hide && btn.classList.contains('tab-active')) showTab('live');
  });
  var contracts = document.querySelector('.sub-tab-btn[data-career-sub="finances"]');
  if (contracts) {
    contracts.style.display = hide ? 'none' : '';
    if (hide && contracts.classList.contains('sub-tab-active')) {
      var champs = document.querySelector('.sub-tab-btn[data-career-sub="champs"]');
      if (champs) champs.click();
    }
  }
}

/// A career written before careers had a kind has to be asked once — every rule turns on it.
/// The answer is final, so it is a confirm rather than a silent default.
function askCareerMode(name) {
  var sp = confirm(
    'The career "' + name + '" was created before careers had a kind.\n\n' +
    'OK  — Singleplayer: race the AI. Pick a Custom AI roster per season, sign for a team, ' +
    'one season at a time.\n' +
    'Cancel — Multiplayer: race people. No roster, no team, no contracts, as many seasons ' +
    'at once as you like.\n\n' +
    'This is permanent.');
  saveAction('/api/career/mode', {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ mode: sp ? 'singleplayer' : 'multiplayer' }),
  }, true);
}

function setSavesMsg(msg, isError) {
  var el = document.getElementById('saves-msg');
  if (!el) return;
  el.textContent = msg;
  el.className = 'config-save-msg' + (isError ? ' config-save-msg-error' : (msg ? ' config-save-msg-ok' : ''));
}

function renderSaves(data) {
  var saves = data.saves || [];

  var dirLabel = document.getElementById('saves-dir-label');
  if (dirLabel) dirLabel.textContent = data.dir || '';

  var sel = document.getElementById('save-select');
  if (sel && !saves.length) {
    // No careers at all — the app does not invent one, so say so rather than showing an empty
    // dropdown that looks broken.
    sel.innerHTML = '<option disabled selected>No careers yet</option>';
  } else if (sel) {
    sel.innerHTML = saves.map(function (s) {
      // A save that will not parse stays listed but cannot be switched to — the server refuses
      // it anyway, and an unselectable row is a clearer answer than a silently missing one.
      return '<option value="' + esc(s.name) + '"' + (s.active ? ' selected' : '') +
        (s.error ? ' disabled' : '') + '>' + esc(s.name) +
        (s.error ? ' (unreadable)' : '') + '</option>';
    }).join('');
  }

  var list = document.getElementById('saves-list');
  if (!list) return;
  if (!saves.length) {
    list.innerHTML = '<li class="saves-item"><span class="saves-counts">' +
      'No careers yet — create one below. Sessions are not recorded until you do.' +
      '</span></li>';
    return;
  }
  list.innerHTML = saves.map(function (s) {
    var name = esc(s.name);
    return '<li class="saves-item' + (s.active ? ' saves-item-active' : '') + '">' +
      '<span class="saves-name">' + name + (s.active ? ' <span class="saves-badge">active</span>' : '') + '</span>' +
      (s.error
        ? '<span class="saves-error" title="' + esc(s.error) +
          '">⚠ Cannot be read — it will not be written to, so nothing in it is lost</span>'
        : '<span class="saves-counts">' + MODE_LABEL[s.mode] + ' &middot; ' +
          s.championships + ' championship(s), ' + s.sessions + ' session(s)</span>') +
      '<span class="saves-actions">' +
        (s.active || s.error ? '' : '<button class="manage-btn" data-save-activate="' + name + '">Switch to</button>') +
        '<button class="manage-btn" data-save-duplicate="' + name + '">Duplicate</button>' +
        '<button class="manage-btn" data-save-rename="' + name + '">Rename</button>' +
        (s.active ? '' : '<button class="manage-btn" data-save-delete="' + name + '">Delete</button>') +
      '</span>' +
    '</li>';
  }).join('');
}

function loadSaves() {
  return fetch('/api/saves').then(function (r) { return r.json(); })
    .then(function (data) {
      renderSaves(data);
      setSavesMsg('');
      var active = (data.saves || []).find(function (s) { return s.active; });
      careerMode = (active && active.mode) || 'unset';
      applyCareerMode();
      // Ask once, and only for a career that has never been asked.
      if (active && !active.error && active.mode === 'unset') askCareerMode(active.name);
    })
    .catch(function () { setSavesMsg('Failed to load save files.', true); });
}

/// Send a save request; on success either reload (switch) or re-render the list.
function saveAction(url, opts, reload) {
  return fetch(url, opts).then(function (r) {
    return r.json().then(function (body) { return { ok: r.ok, body: body }; });
  }).then(function (res) {
    if (!res.ok) { setSavesMsg(res.body.error || 'Request failed.', true); return; }
    if (reload) { location.reload(); return; }
    renderSaves(res.body);
    setSavesMsg('Saved.');
  }).catch(function () { setSavesMsg('Request failed.', true); });
}

function postSave(path, payload, reload) {
  return saveAction(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  }, reload);
}

function activateSave(name) {
  postSave('/api/saves/activate', { name: name }, true);
}

document.addEventListener('DOMContentLoaded', function () {
  loadSaves();

  var sel = document.getElementById('save-select');
  if (sel) sel.addEventListener('change', function () { activateSave(sel.value); });

  var newBtn = document.getElementById('save-new-btn');
  if (newBtn) newBtn.addEventListener('click', function () {
    var input = document.getElementById('save-new-name');
    var name = input.value.trim();
    if (!name) { setSavesMsg('Enter a name for the new career.', true); return; }
    var modeEl = document.getElementById('save-new-mode');
    postSave('/api/saves', { name: name, mode: modeEl ? modeEl.value : 'singleplayer' }, true);
  });

  var list = document.getElementById('saves-list');
  if (list) list.addEventListener('click', function (e) {
    var btn = e.target.closest('button');
    if (!btn) return;
    var d = btn.dataset;

    if (d.saveActivate) {
      activateSave(d.saveActivate);
    } else if (d.saveDuplicate) {
      var copy = prompt('Name for the copy of "' + d.saveDuplicate + '":', d.saveDuplicate + ' copy');
      if (copy) postSave('/api/saves/duplicate', { name: d.saveDuplicate, new_name: copy }, false);
    } else if (d.saveRename) {
      var renamed = prompt('New name for "' + d.saveRename + '":', d.saveRename);
      if (renamed && renamed !== d.saveRename) {
        saveAction('/api/saves/' + encodeURIComponent(d.saveRename), {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ new_name: renamed }),
        }, false);
      }
    } else if (d.saveDelete) {
      if (confirm('Delete the career "' + d.saveDelete + '"? This cannot be undone.')) {
        saveAction('/api/saves/' + encodeURIComponent(d.saveDelete), { method: 'DELETE' }, false);
      }
    }
  });
});

document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () {
    if (btn.dataset.tab === 'config') loadSaves();
  });
});
