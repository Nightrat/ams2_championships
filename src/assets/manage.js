// ── Manage tab ────────────────────────────────────────────────────────────────
var manageState = { champs: [], sessions: [], customAiFiles: [], selectedId: null, currentRidx: 0 };

function loadManage() {
  Promise.all([
    fetch('/api/championships').then(function (r) { return r.json(); }),
    fetch('/api/sessions').then(function (r) { return r.json(); }),
    fetch('/api/custom-ai-files').then(function (r) { return r.json(); }).catch(function () { return []; })
  ]).then(function (results) {
    manageState.champs = sortChamps(results[0] || []);
    manageState.sessions = results[1] || [];
    manageState.customAiFiles = results[2] || [];
    var active = manageState.champs.find(function (c) { return c.status === 'Active'; });
    manageState.selectedId = active ? active.id : (manageState.champs[0] ? manageState.champs[0].id : null);
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
  function statusColourClass(status) {
    return status === 'Final' ? 'status-final' : status === 'Progress' ? 'status-progress' : 'status-active';
  }
  el.innerHTML = manageState.champs.map(function (c) {
    var sel = c.id === manageState.selectedId ? ' selected' : '';
    var opts = ['Active', 'Progress', 'Final'].map(function (s) {
      return '<option' + (s === c.status ? ' selected' : '') + '>' + s + '</option>';
    }).join('');
    return '<div class="champ-list-item' + sel + '" data-id="' + esc(c.id) + '">' +
      '<span class="champ-list-name">' + esc(c.name) + '</span>' +
      '<button class="champ-rename-btn" data-cid="' + esc(c.id) + '" title="Rename">&#9998;</button>' +
      '<select class="champ-list-status ' + statusColourClass(c.status) + '" data-cid="' + esc(c.id) + '">' + opts + '</select>' +
      '</div>';
  }).join('');
  el.querySelectorAll('.champ-list-item').forEach(function (item) {
    item.addEventListener('click', function () {
      manageState.selectedId = item.dataset.id;
      renderChampList();
      renderChampDetail(item.dataset.id);
    });
  });
  el.querySelectorAll('.champ-list-status').forEach(function (sel) {
    sel.addEventListener('click', function (e) { e.stopPropagation(); });
    sel.addEventListener('change', function (e) {
      e.stopPropagation();
      sel.className = 'champ-list-status ' + statusColourClass(sel.value);
      patchChamp(sel.dataset.cid, { status: sel.value });
    });
  });
  el.querySelectorAll('.champ-rename-btn').forEach(function (btn) {
    btn.addEventListener('click', function (e) {
      e.stopPropagation();
      var item = btn.closest('.champ-list-item');
      var nameSpan = item.querySelector('.champ-list-name');
      var original = nameSpan.textContent;
      var input = document.createElement('input');
      input.className = 'champ-list-rename-input';
      input.value = original;
      nameSpan.replaceWith(input);
      btn.style.display = 'none';
      input.focus();
      input.select();
      var committed = false;
      function commit() {
        if (committed) return;
        committed = true;
        var newName = input.value.trim();
        if (newName && newName !== original) {
          patchChamp(btn.dataset.cid, { name: newName });
        } else {
          input.replaceWith(nameSpan);
          btn.style.display = '';
        }
      }
      function cancel() {
        if (committed) return;
        committed = true;
        input.replaceWith(nameSpan);
        btn.style.display = '';
      }
      input.addEventListener('keydown', function (ev) {
        if (ev.key === 'Enter')  commit();
        if (ev.key === 'Escape') cancel();
      });
      input.addEventListener('blur', commit);
    });
  });
}

function renderChampDetail(id) {
  var champ = manageState.champs.find(function (c) { return c.id === id; });
  var right = document.getElementById('manage-right');
  if (!champ || !right) return;

  var rounds = champ.rounds || [];

  var aiOptions = '<option value="">None</option>' +
    manageState.customAiFiles.map(function (f) {
      return '<option value="' + esc(f) + '"' + (f === champ.custom_ai_file ? ' selected' : '') + '>' + esc(f) + '</option>';
    }).join('');
  var aiHint = manageState.customAiFiles.length
    ? 'Driver names matching a &lt;name&gt; entry in this file show its livery/team name instead of the AMS2 car class.'
    : 'No .xml files found. Set a Custom AI Drivers folder in the Config tab.';
  // A player team is only checkable against a Custom AI roster, so without a file assigned the
  // field is disabled and the server keeps player_team null — that is also what switches
  // session enforcement off.
  var hasAi = !!champ.custom_ai_file;
  // The team is committed once the championship has a session: changing it would rewrite the
  // meaning of results already scored, so the server refuses it too.
  var started = rounds.some(function (r) { return (r.session_ids || []).length > 0; });
  var teamLocked = !hasAi || started;
  var playerTeamHint = !hasAi
    ? 'Assign a Custom AI Drivers file first — its roster is what your seat is checked against.'
    : started
      ? 'Locked in: the championship has started. Remove its assigned sessions to change teams.'
      : 'AMS2 exposes no livery field. Your seat is inferred from the car you drove plus which roster drivers are on the grid, and sessions that contradict it are rejected.';

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
      '<label>Points&nbsp;<input class="manage-input champ-points-input" value="' + esc(champ.points_system.join(',')) + '" data-id="' + esc(champ.id) + '" size="32" title="Comma-separated points per finishing position"></label>' +
      '<label class="manage-checkbox-label"><input type="checkbox" class="champ-manufacturer-check"' + (champ.manufacturer_scoring ? ' checked' : '') + '> Constructor Scoring</label>' +
      '<label title="' + aiHint + '">Custom AI Drivers&nbsp;<select class="manage-select champ-custom-ai-select">' + aiOptions + '</select></label>' +
      // Teams are picked, never typed: a free-text seat could name a team that is not in the
      // roster at all, which the rating cannot judge and so would silently never be enforced.
      // Options arrive from loadPlayerTeamOptions; until then only the current value is listed.
      '<label title="' + esc(playerTeamHint) + '">My Team&nbsp;' +
        '<select class="manage-select champ-player-team-select"' + (teamLocked ? ' disabled' : '') + '>' +
          '<option value="">' + (hasAi ? '(none)' : 'needs a Custom AI file') + '</option>' +
          (champ.player_team
            ? '<option value="' + esc(champ.player_team) + '" selected>' + esc(champ.player_team) + '</option>'
            : '') +
        '</select>' +
      '</label>' +
      '<span class="config-hint" id="player-team-rating"></span>' +
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
  right.querySelector('.champ-points-input').addEventListener('blur', function () {
    var pts = this.value.split(',')
      .map(function (v) { return parseInt(v.trim(), 10); })
      .filter(function (n) { return !isNaN(n); });
    patchChamp(champ.id, { points_system: pts });
  });
  right.querySelector('.champ-manufacturer-check').addEventListener('change', function () {
    patchChamp(champ.id, { manufacturer_scoring: this.checked });
  });
  right.querySelector('.champ-custom-ai-select').addEventListener('change', function () {
    patchChamp(champ.id, { custom_ai_file: this.value || null });
  });
  right.querySelector('.champ-player-team-select').addEventListener('change', function () {
    var val = this.value;
    if (val === (champ.player_team || '')) return;
    var select = this;
    patchChamp(champ.id, { player_team: val || null }, function (err) {
      // 409 = the driver rating has not earned this seat.
      alert(err);
      select.value = champ.player_team || '';
    });
  });
  loadPlayerTeamOptions(champ.id);
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

function loadPlayerTeamOptions(champId) {
  fetch('/api/championships/' + champId + '/team-eligibility').then(function (r) { return r.json(); })
    .then(function (el) {
      if (manageState.selectedId !== champId) return; // user navigated away before this resolved
      var select = document.querySelector('.champ-player-team-select');
      var note = document.getElementById('player-team-rating');
      if (!select) return;
      var champ = manageState.champs.find(function (c) { return c.id === champId; });
      var current = (champ && champ.player_team) || '';
      if (!el || !el.rated) {
        if (note) note.textContent = '';
        return;
      }
      if (select.disabled) {
        // Committed for this championship — the option list is moot, but the rating still is not.
        if (note) {
          note.textContent = 'Driver rating ' + Math.round(el.reputation.value) + '/100' +
            ' — team locked in, this championship has already started.';
        }
        return;
      }
      // Offer only what the rating has earned; locked teams stay out of the list and are
      // refused by the server too, unless enforcement is switched off in Config.
      var teams = el.teams || [];
      var offerable = teams.filter(function (t) { return t.tier !== 'locked'; });
      var listed = el.enforced ? offerable : teams;
      var opts = '<option value="">(none)</option>';
      // An already-claimed seat stays selectable even if the rating has since dropped below it,
      // so opening the panel can never silently drop the team that is already saved.
      if (current && !listed.some(function (t) { return t.team === current; })) {
        opts += '<option value="' + esc(current) + '">' + esc(current) + ' (current)</option>';
      }
      opts += listed.map(function (t) {
        var label = t.team + ' — needs ' + Math.round(t.required) +
          (t.tier === 'offer_possible' ? ', within reach' : '');
        return '<option value="' + esc(t.team) + '">' + esc(label) + '</option>';
      }).join('');
      select.innerHTML = opts;
      select.value = current;
      if (note) {
        var locked = teams.filter(function (t) { return t.tier === 'locked'; }).length;
        var soon = teams.filter(function (t) { return t.tier === 'offer_possible'; })
          .map(function (t) { return t.team; });
        note.textContent = 'Driver rating ' + Math.round(el.reputation.value) + '/100' +
          (el.enforced ? '' : ' (enforcement off)') +
          ' — ' + offerable.length + ' of ' + teams.length + ' teams open' +
          (locked ? ', ' + locked + ' locked' : '') +
          (soon.length ? '. Within reach: ' + soon.join(', ') : '');
      }
    }).catch(function () {});
}

function renderAvailableSessions(champId) {
  // Eligibility is server-side; it only blocks anything when the championship has both a
  // Custom AI file and a selected team. Failing open keeps the picker usable if it errors.
  fetch('/api/championships/' + champId + '/session-eligibility')
    .then(function (r) { return r.json(); })
    .catch(function () { return null; })
    .then(function (elig) { renderAvailableSessionsWith(champId, elig || {}); });
}

function renderAvailableSessionsWith(champId, elig) {
  var assignedIds = [];
  manageState.champs.forEach(function (c) {
    (c.rounds || []).forEach(function (round) {
      (round.session_ids || []).forEach(function (sid) { assignedIds.push(sid); });
    });
  });
  var ridx = manageState.currentRidx || 0;
  var available = manageState.sessions.filter(function (s) { return !assignedIds.includes(s.id); });
  var blocked = elig.blocked || {};
  var hidden = 0;
  if (elig.enforced) {
    available = available.filter(function (s) {
      if (!blocked[s.id]) return true;
      hidden++;
      return false;
    });
  }
  var hiddenNote = hidden
    ? '<div class="manage-empty">' + hidden + ' session' + (hidden === 1 ? '' : 's') +
      ' hidden — they contradict your team for this championship.</div>'
    : '';
  var el = document.getElementById('available-sessions');
  if (!el) return;
  if (!available.length) {
    el.innerHTML = hiddenNote || '<div class="manage-empty">No unassigned sessions.</div>';
    return;
  }
  el.innerHTML = hiddenNote + available.map(function (s) {
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
        .then(function (r) {
          // 409 = the session contradicts the championship's declared player team.
          if (!r.ok) {
            return r.json().then(function (e) {
              alert('Session not added: ' + (e && e.error ? e.error : 'rejected by the server.'));
            });
          }
          loadManage();
          renderChampDetail(champId);
          renderAvailableSessions(champId);
        });
    });
  });
}

function patchChamp(id, patch, onError) {
  fetch('/api/championships/' + id, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(patch)
  }).then(function (r) {
    if (!r.ok && onError) {
      return r.json().then(function (e) { onError(e && e.error ? e.error : 'Rejected.'); });
    }
    loadManage();
  });
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
