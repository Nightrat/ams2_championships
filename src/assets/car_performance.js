// ── Car Performance tab ───────────────────────────────────────────────────────

function carPerfPaceLabel(pct) {
  if (pct <= 0.0001) return '<span class="carperf-fastest">Fastest</span>';
  return '+' + pct.toFixed(1) + '%';
}

function carPerfClassLabel(cls) {
  return esc(cls.class) + (cls.year ? ' <span class="carperf-year">(' + cls.year + ')</span>' : '');
}

// Ratings span every class, so the highest one decides which seats anywhere are within reach.
function carPerfBestRating(players) {
  return (players || []).reduce(function (best, p) {
    return p.rating > best ? p.rating : best;
  }, -1);
}

function carPerfPlayersHtml(players) {
  if (!players || !players.length) {
    return '<p class="carperf-note carperf-players-empty">No rateable races recorded yet — ' +
      'a rating needs sessions in a class that has a Custom AI Drivers file.</p>';
  }
  var rows = players.slice().sort(function (a, b) { return b.rating - a.rating; })
    .map(function (p) {
      var mp = p.mp_races ? p.mp_wins + '–' + p.mp_losses + ' online' : '';
      var races = p.sp_races + (p.sp_races === 1 ? ' race' : ' races');
      return '<li class="carperf-player">' +
        '<span class="carperf-player-name">' + esc(p.name) + '</span> ' +
        '<span class="carperf-player-rating">' + Math.round(p.rating) + '/100</span> ' +
        '<span class="carperf-player-races">' + races + (mp ? ', ' + mp : '') + '</span>' +
        '</li>';
    }).join('');
  return '<section class="carperf-ratings">' +
    '<h3 class="carperf-heading">Driver ratings <span class="carperf-year">(all classes)</span></h3>' +
    '<ul class="carperf-players">' + rows + '</ul>' +
    '</section>';
}

// Overwritten from the server's own limits on every load, so the table and the writer can never
// disagree about what is allowed. These are only the fallback if the payload predates them.
var CARPERF_SCALAR_MIN = 0.5;
var CARPERF_SCALAR_MAX = 2;

function carPerfSetLimits(data) {
  if (data && typeof data.scalar_min === 'number') CARPERF_SCALAR_MIN = data.scalar_min;
  if (data && typeof data.scalar_max === 'number') CARPERF_SCALAR_MAX = data.scalar_max;
}

// Editable cell. The value lives in the input, not in the cell text — see cellVal in utils.js.
// `data-stored` keeps the last value the file is known to hold, so a rejected edit can be put
// back rather than leaving the table showing a number that was never written.
function carPerfScalarCell(field, value) {
  return '<td class="stat-num carperf-scalar-cell">' +
    '<input type="number" class="carperf-scalar" data-field="' + field + '"' +
    ' step="0.01" min="' + CARPERF_SCALAR_MIN + '" max="' + CARPERF_SCALAR_MAX + '"' +
    ' value="' + value.toFixed(2) + '" data-stored="' + value.toFixed(2) + '">' +
    '</td>';
}

function carPerfRangeMessage() {
  return 'Scalars must be between ' + CARPERF_SCALAR_MIN.toFixed(2) + ' and ' +
    CARPERF_SCALAR_MAX.toFixed(2) + '.';
}

/// Put every scalar in the row back to what the file last confirmed.
function carPerfRevertRow(tr) {
  tr.querySelectorAll('.carperf-scalar').forEach(function (input) {
    if (input.dataset.stored !== undefined) input.value = input.dataset.stored;
  });
}

var CARPERF_NO_SEAT_TITLE = 'Every entry for this team names a livery AMS2 does not own, so ' +
  'the team never reaches a grid and there is no seat to earn.';

function carPerfReqClass(req, best) {
  if (req === null || req === undefined) return 'stat-num carperf-req carperf-req-none';
  if (best < 0) return 'stat-num carperf-req';
  return 'stat-num carperf-req ' + (best >= req ? 'carperf-req-met' : 'carperf-req-unmet');
}

// A team with no real seat has no requirement at all, which is not the same as a requirement
// of zero — showing 0 would read as "open to anyone".
function carPerfReqLabel(req) {
  if (req === null || req === undefined) return '—';
  return Math.round(req);
}

function renderCarPerformanceClass(cls, idx, best) {
  var cars = cls.cars || [];
  var tableId = 'carperf-table-' + idx;
  var col = 0;
  function th(cls2, type, label) { return '<th class="' + cls2 + '" data-col="' + (col++) + '" data-type="' + type + '">' + label + '</th>'; }
  var thead = '<tr>' +
    th('stat-name sort-asc', 'str', 'Team') +
    th('stat-name', 'str', 'Drivers') +
    th('stat-num', 'num', 'Power') +
    th('stat-num', 'num', 'Weight') +
    th('stat-num', 'num', 'Drag') +
    th('stat-num', 'num', 'Est. Pace') +
    th('stat-num', 'num', 'Req. Rating') +
    '</tr>';
  var tbody = cars.map(function (c) {
    var mark = c.phantom
      ? ' <span class="driverperf-phantom-tag" title="' + esc(CARPERF_NO_SEAT_TITLE) + '">no livery</span>'
      : '';
    return '<tr data-team="' + esc(c.team) + '"' + (c.phantom ? ' class="driverperf-phantom"' : '') + '>' +
      '<td class="stat-name">' + liveryImg(c.preview, 'carperf-car') + esc(c.team) + mark + '</td>' +
      '<td class="stat-name carperf-drivers">' + esc((c.drivers || []).join(', ')) + '</td>' +
      carPerfScalarCell('power_scalar', c.power_scalar) +
      carPerfScalarCell('weight_scalar', c.weight_scalar) +
      carPerfScalarCell('drag_scalar', c.drag_scalar) +
      '<td class="stat-num carperf-pace">' + carPerfPaceLabel(c.pace_delta_pct) + '</td>' +
      '<td class="' + carPerfReqClass(c.required_rating, best) + '"' +
        (c.phantom ? ' title="' + esc(CARPERF_NO_SEAT_TITLE) + '"' : '') + '>' +
        carPerfReqLabel(c.required_rating) + '</td>' +
      '</tr>';
  }).join('');
  return '<section class="carperf-class" data-carperf-class="' + esc(cls.class) + '">' +
    '<h3 class="carperf-heading">' + carPerfClassLabel(cls) +
      carPerfBaselineHtml(cls) +
    '</h3>' +
    '<table class="stats-table sortable" id="' + tableId + '">' +
    '<thead>' + thead + '</thead><tbody>' + tbody + '</tbody></table>' +
    '</section>';
}

// Reset is offered only when there is a baseline to go back to — one is recorded on the first
// edit, so an untouched class has none. Adopt is always available: it is how a roster tuned by
// hand *after* the baseline was taken becomes the thing a reset returns to.
function carPerfBaselineHtml(cls) {
  return '<span class="carperf-baseline">' +
    (cls.has_baseline
      ? '<button class="manage-btn" data-carperf-reset="' + esc(cls.class) + '">Reset to baseline</button>'
      : '<span class="carperf-note">no baseline yet — the first edit records one</span>') +
    '<button class="manage-btn" data-carperf-baseline="' + esc(cls.class) + '">Set current as baseline</button>' +
    '</span>';
}

// The tab opens on whatever the user is actually racing; with nothing in progress the server
// sends an empty list and every class stays selected.
function carPerfPreselected(classes, active) {
  var wanted = {};
  (active || []).forEach(function (c) { wanted[c] = true; });
  var any = classes.some(function (cls) { return wanted[cls.class]; });
  return any ? wanted : null;
}

function carPerfFilterBarHtml(classes, preselect) {
  var options = classes.map(function (cls) {
    var on = !preselect || preselect[cls.class];
    return '<label class="carperf-filter-check">' +
      '<input type="checkbox" class="carperf-filter-input" value="' + esc(cls.class) + '"' +
      (on ? ' checked' : '') + '> ' +
      carPerfClassLabel(cls) +
      '</label>';
  }).join('');
  return '<div class="carperf-filter-bar">' +
    '<div class="carperf-filter-actions">' +
      '<button type="button" class="manage-btn" id="carperf-filter-all">All</button>' +
      '<button type="button" class="manage-btn" id="carperf-filter-none">None</button>' +
    '</div>' +
    '<div class="carperf-filter-checks">' + options + '</div>' +
    '<span id="carperf-status" class="carperf-status"></span>' +
    '</div>';
}

// ── Editing scalars ───────────────────────────────────────────────────────────

function carPerfStatus(msg, isError) {
  var el = document.getElementById('carperf-status');
  if (!el) return;
  el.textContent = msg;
  el.className = 'carperf-status' + (isError ? ' carperf-status-error' : ' carperf-status-ok');
}

// Refreshes the numbers a save changed without rebuilding the tables: an edit shifts the whole
// class's pace baseline and every rating derived from it, but re-rendering would also throw away
// the user's sort order, class filter, and keyboard focus mid-tune.
function carPerfApplyData(data) {
  var players = (data && data.players) || [];
  var best = carPerfBestRating(players);
  var byClass = {};
  ((data && data.classes) || []).forEach(function (cls) { byClass[cls.class] = cls; });
  document.querySelectorAll('.carperf-class').forEach(function (section) {
    var cls = byClass[section.dataset.carperfClass];
    if (!cls) return;
    var byTeam = {};
    (cls.cars || []).forEach(function (c) { byTeam[c.team] = c; });
    section.querySelectorAll('tbody tr').forEach(function (tr) {
      var c = byTeam[tr.dataset.team];
      if (!c) return;
      tr.querySelectorAll('.carperf-scalar').forEach(function (input) {
        var written = c[input.dataset.field].toFixed(2);
        // The file now holds this, so it becomes the value a later rejection reverts to.
        input.dataset.stored = written;
        // Never overwrite the field the user is still typing in.
        if (document.activeElement !== input) input.value = written;
      });
      var drivers = tr.querySelector('.carperf-drivers');
      if (drivers) drivers.textContent = (c.drivers || []).join(', ');
      var pace = tr.querySelector('.carperf-pace');
      if (pace) pace.innerHTML = carPerfPaceLabel(c.pace_delta_pct);
      var req = tr.querySelector('.carperf-req');
      if (req) {
        req.textContent = carPerfReqLabel(c.required_rating);
        req.className = carPerfReqClass(c.required_rating, best);
      }
    });
    // The first edit of a class creates its baseline, so the Reset button has to appear without
    // a reload. Safe to replace wholesale because the click handler is delegated.
    var baseline = section.querySelector('.carperf-baseline');
    if (baseline) baseline.outerHTML = carPerfBaselineHtml(cls);
  });
  var slot = document.getElementById('carperf-ratings-slot');
  if (slot) slot.innerHTML = carPerfPlayersHtml(players);
}

// Both baseline actions rewrite a whole file, so both confirm first — and they discard different
// things, which is why the wording is not shared.
function carPerfBaselineAction(cls, reset) {
  var ask = reset
    ? 'Reset ' + cls + ' to its baseline?\n\n' +
      'Every scalar edit made to this class since the baseline was recorded is discarded.'
    : 'Make the current ' + cls + ' file the baseline?\n\n' +
      'The file it replaces cannot be recovered, and "Reset to baseline" will come back to ' +
      'this one from now on.';
  if (!confirm(ask)) return;
  carPerfStatus(reset ? 'Resetting ' + cls + '…' : 'Recording baseline for ' + cls + '…');
  fetch('/api/car-performance/' + (reset ? 'reset' : 'baseline'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ class: cls })
  }).then(function (r) {
    return r.json().then(function (d) { return { ok: r.ok, data: d }; });
  }).then(function (res) {
    if (!res.ok) {
      carPerfStatus((res.data && res.data.error) || 'Failed.', true);
      return;
    }
    carPerfApplyData(res.data);
    carPerfStatus(reset
      ? cls + '.xml restored from its baseline'
      : cls + '.xml.bak now holds the current file');
  }).catch(function () {
    carPerfStatus('Failed — is the server still running?', true);
  });
}

function carPerfSaveRow(tr) {
  var section = tr.closest('.carperf-class');
  if (!section) return;
  var body = { class: section.dataset.carperfClass, team: tr.dataset.team };
  var bad = null;
  tr.querySelectorAll('.carperf-scalar').forEach(function (input) {
    var v = parseFloat(input.value);
    if (!isFinite(v) || v < CARPERF_SCALAR_MIN || v > CARPERF_SCALAR_MAX) bad = input;
    body[input.dataset.field] = v;
  });
  if (bad) {
    // Nothing was written, so the cell must not keep showing the rejected number.
    carPerfRevertRow(tr);
    carPerfStatus(carPerfRangeMessage(), true);
    return;
  }
  tr.classList.add('carperf-row-saving');
  carPerfStatus('Saving ' + tr.dataset.team + '…');
  fetch('/api/car-performance', {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  }).then(function (r) {
    return r.json().then(function (d) { return { ok: r.ok, data: d }; });
  }).then(function (res) {
    tr.classList.remove('carperf-row-saving');
    if (!res.ok) {
      // The server is the authority on the range; if it refused, nothing reached the file.
      carPerfRevertRow(tr);
      carPerfStatus((res.data && res.data.error) || 'Save failed.', true);
      return;
    }
    carPerfApplyData(res.data);
    carPerfStatus('Saved ' + tr.dataset.team + ' to ' + body['class'] + '.xml');
  }).catch(function () {
    tr.classList.remove('carperf-row-saving');
    carPerfRevertRow(tr);
    carPerfStatus('Save failed — is the server still running?', true);
  });
}

function carPerfApplyFilter() {
  var checked = {};
  document.querySelectorAll('.carperf-filter-input').forEach(function (input) {
    checked[input.value] = input.checked;
  });
  document.querySelectorAll('.carperf-class').forEach(function (section) {
    var show = checked[section.dataset.carperfClass] !== false;
    section.classList.toggle('carperf-class-hidden', !show);
  });
}

function renderCarPerformance(data) {
  var classes = (data && data.classes) || [];
  var players = (data && data.players) || [];
  carPerfSetLimits(data);
  var container = document.getElementById('carperf-container');
  if (!container) return;
  if (!classes.length) {
    container.innerHTML = '<div class="manage-placeholder" style="padding:2rem">' +
      'No Custom AI Driver files found. Set the Custom AI Drivers folder in the Config tab ' +
      '(e.g. <code>...\\Automobilista 2\\UserData\\CustomAIDrivers</code>).</div>';
    return;
  }
  var caption = '<p class="carperf-note">Est. Pace is a rough estimate from each car’s power/weight/drag ' +
    'scalars (not an AMS2-measured figure) — the fastest car in each class is the baseline. ' +
    'Classes are always listed in chronological order by the season they model. ' +
    'Req. Rating is the driver rating needed to claim that seat: the higher of how far up the grid ' +
    'the rating reaches and the skill of the team’s weaker driver, so a midfield car with a strong ' +
    'line-up can ask more than a quicker one. Ratings shown are performance at the AI difficulty ' +
    'actually raced, not an absolute skill measure. ' +
    'Power, Weight and Drag are editable: change one and it is written straight back into that ' +
    'class’s Custom AI Drivers XML, for every driver on the team. The first edit of a class keeps ' +
    'the file as it was then as <code>.xml.bak</code> — its baseline — and Reset to baseline ' +
    'restores it. Tuned a roster by hand after that? Set current as baseline adopts it, so a ' +
    'later reset comes back to your version instead of the one the app happened to catch. ' +
    'AMS2 reads the file when a session loads, so restart the ' +
    'session for an edit to take effect. Reiza documents the scalars as 0.900–1.100, where ' +
    '1.000 means no change, and edits outside that are refused.</p>';
  var best = carPerfBestRating(players);
  var preselect = carPerfPreselected(classes, data && data.active_classes);
  container.innerHTML = caption +
    '<div id="carperf-ratings-slot">' + carPerfPlayersHtml(players) + '</div>' +
    carPerfFilterBarHtml(classes, preselect) +
    '<div id="carperf-classes">' +
    classes.map(function (cls, idx) { return renderCarPerformanceClass(cls, idx, best); }).join('') +
    '</div>';
  classes.forEach(function (cls, idx) {
    initSortableTableEl(document.getElementById('carperf-table-' + idx));
  });
  carPerfApplyFilter();
  if (preselect) {
    var note = document.getElementById('carperf-status');
    if (note) {
      note.className = 'carperf-status carperf-status-note';
      note.textContent = 'Showing the class you are racing — All shows every class.';
    }
  }
  document.querySelectorAll('.carperf-scalar').forEach(function (input) {
    // 'change' rather than 'input': one save per committed edit, not one per keystroke.
    input.addEventListener('change', function () { carPerfSaveRow(input.closest('tr')); });
  });
  // Delegated, so the buttons survive `carPerfApplyData` replacing them after an edit.
  var classesEl = document.getElementById('carperf-classes');
  if (classesEl) {
    classesEl.addEventListener('click', function (ev) {
      var reset = ev.target.getAttribute && ev.target.getAttribute('data-carperf-reset');
      var adopt = ev.target.getAttribute && ev.target.getAttribute('data-carperf-baseline');
      if (reset) carPerfBaselineAction(reset, true);
      else if (adopt) carPerfBaselineAction(adopt, false);
    });
  }
  document.querySelectorAll('.carperf-filter-input').forEach(function (input) {
    input.addEventListener('change', carPerfApplyFilter);
  });
  var allBtn = document.getElementById('carperf-filter-all');
  var noneBtn = document.getElementById('carperf-filter-none');
  if (allBtn) allBtn.addEventListener('click', function () {
    document.querySelectorAll('.carperf-filter-input').forEach(function (i) { i.checked = true; });
    carPerfApplyFilter();
  });
  if (noneBtn) noneBtn.addEventListener('click', function () {
    document.querySelectorAll('.carperf-filter-input').forEach(function (i) { i.checked = false; });
    carPerfApplyFilter();
  });
}

function loadCarPerformance() {
  fetch('/api/car-performance').then(function (r) { return r.json(); })
    .then(function (data) {
      renderCarPerformance(data || {});
    }).catch(function () {
      var el = document.getElementById('carperf-container');
      if (el) el.innerHTML = '<div class="manage-placeholder" style="padding:2rem">Car performance data requires the server binary.</div>';
    });
}

document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () {
    if (btn.dataset.tab === 'carperf') loadCarPerformance();
  });
});
