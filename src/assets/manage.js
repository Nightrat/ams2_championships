// ── Manage tab ────────────────────────────────────────────────────────────────
var manageState = { champs: [], sessions: [], selectedId: null, currentRidx: 0 };

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
  var assignedIds = [];
  manageState.champs.forEach(function (c) {
    (c.rounds || []).forEach(function (round) {
      (round.session_ids || []).forEach(function (sid) { assignedIds.push(sid); });
    });
  });
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

document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () {
    if (btn.dataset.tab === 'manage') loadManage();
  });
});
