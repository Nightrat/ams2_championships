// ── Career save files ─────────────────────────────────────────────────────────
// One save = one career. Switching reloads the page, because every tab caches its
// data and only refetches when its own tab button is clicked.

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
  if (sel) {
    sel.innerHTML = saves.map(function (s) {
      return '<option value="' + esc(s.name) + '"' + (s.active ? ' selected' : '') + '>' + esc(s.name) + '</option>';
    }).join('');
  }

  var list = document.getElementById('saves-list');
  if (!list) return;
  list.innerHTML = saves.map(function (s) {
    var name = esc(s.name);
    return '<li class="saves-item' + (s.active ? ' saves-item-active' : '') + '">' +
      '<span class="saves-name">' + name + (s.active ? ' <span class="saves-badge">active</span>' : '') + '</span>' +
      '<span class="saves-counts">' + s.championships + ' championship(s), ' + s.sessions + ' session(s)</span>' +
      '<span class="saves-actions">' +
        (s.active ? '' : '<button class="manage-btn" data-save-activate="' + name + '">Switch to</button>') +
        '<button class="manage-btn" data-save-duplicate="' + name + '">Duplicate</button>' +
        '<button class="manage-btn" data-save-rename="' + name + '">Rename</button>' +
        (s.active ? '' : '<button class="manage-btn" data-save-delete="' + name + '">Delete</button>') +
      '</span>' +
    '</li>';
  }).join('');
}

function loadSaves() {
  return fetch('/api/saves').then(function (r) { return r.json(); })
    .then(function (data) { renderSaves(data); setSavesMsg(''); })
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
    postSave('/api/saves', { name: name }, true);
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
