// ── Driver Performance tab ────────────────────────────────────────────────────

// Per-attribute bounds, filled from the server on load. Reiza documents every personality value
// as 0–1 inclusive and singles out vehicle_reliability as the one that may leave that range, so
// a single pair of limits for the whole table would be wrong for one column or fourteen.
var DRIVERPERF_RANGES = {};
var DRIVERPERF_FALLBACK = [0, 1];

function driverPerfSetLimits(data) {
  DRIVERPERF_RANGES = {};
  ((data && data.attr_ranges) || []).forEach(function (r) {
    DRIVERPERF_RANGES[r[0]] = [r[1], r[2]];
  });
}

function driverPerfRange(field) {
  return DRIVERPERF_RANGES[field] || DRIVERPERF_FALLBACK;
}

// Column headings. Keyed by the XML tag so the server's attribute list drives the columns and
// anything it adds still shows up, just under its raw tag name.
var DRIVERPERF_LABELS = {
  race_skill: 'Race',
  qualifying_skill: 'Qual',
  aggression: 'Aggr',
  defending: 'Def',
  stamina: 'Stam',
  consistency: 'Cons',
  start_reactions: 'Start',
  wet_skill: 'Wet',
  tyre_management: 'Tyres',
  fuel_management: 'Fuel',
  blue_flag_conceding: 'Blue',
  weather_tyre_changes: 'Wthr',
  avoidance_of_mistakes: 'Mist',
  avoidance_of_forced_mistakes: 'F.Mist',
  vehicle_reliability: 'Relia'
};

function driverPerfLabel(field) {
  return DRIVERPERF_LABELS[field] || field;
}

// Filled from the server's own weights so the tooltip can never drift from the maths.
var driverPerfRatingTitle = 'Weighted composite of the declared attributes.';

function driverPerfBuildRatingTitle(weights) {
  if (!weights || !weights.length) return;
  var parts = weights.map(function (w) { return driverPerfLabel(w[0]) + ' ' + w[1] + '%'; });
  driverPerfRatingTitle = 'Weighted composite of this entry’s declared attributes: ' +
    parts.join(', ') + '. Averaged over only the attributes the entry declares, so a sparse ' +
    'entry is rated on what it says rather than punished for silence. Aggression, blue flags, ' +
    'weather stops and reliability are excluded — higher is not better for those. Blank when ' +
    'the entry declares no race_skill.';
}

// 0–100, or blank when there is no race_skill to build on.
function driverPerfRatingCell(rating) {
  if (rating === null || rating === undefined) {
    return '<td class="stat-num driverperf-rating driverperf-rating-none" title="' +
      esc(driverPerfRatingTitle) + '">—</td>';
  }
  return '<td class="stat-num driverperf-rating" title="' + esc(driverPerfRatingTitle) + '">' +
    Math.round(rating) + '</td>';
}

function driverPerfStatus(msg, isError) {
  var el = document.getElementById('driverperf-status');
  if (!el) return;
  el.textContent = msg;
  el.className = 'carperf-status' + (isError ? ' carperf-status-error' : ' carperf-status-ok');
}

// Editable cell. Blank when the entry does not declare the attribute at all — AMS2 falls back to
// its own default there, which is not the same as an explicit 0.
function driverPerfCell(field, value) {
  var v = (value === undefined || value === null) ? '' : value.toFixed(2);
  var r = driverPerfRange(field);
  return '<td class="stat-num carperf-scalar-cell">' +
    '<input type="number" class="driverperf-attr" data-field="' + field + '"' +
    ' step="0.01" min="' + r[0] + '" max="' + r[1] + '"' +
    ' value="' + v + '" title="' + esc(field + ' (' + r[0].toFixed(2) + '–' + r[1].toFixed(2) + ')') + '">' +
    '</td>';
}

var DRIVERPERF_PHANTOM_TITLE = 'AMS2 has no livery matching this entry’s livery_name, ' +
  'so this driver can never appear on a grid. Editing the values changes nothing in game.';

// Count of entries the server could prove AMS2 will ignore. `phantom === null` means the check
// could not be made for this class, which must not be reported as a clean bill of health.
function driverPerfPhantomCount(drivers) {
  return drivers.reduce(function (n, d) { return d.phantom === true ? n + 1 : n; }, 0);
}

function driverPerfHeadingNote(drivers) {
  var checked = drivers.some(function (d) { return d.phantom !== null && d.phantom !== undefined; });
  if (!checked) {
    return '<span class="driverperf-unverified" title="No livery manifest covers this class, ' +
      'so its real liveries are still sealed in the game’s pak files and nothing can be ' +
      'proved either way.">liveries not verifiable</span>';
  }
  var n = driverPerfPhantomCount(drivers);
  if (!n) return '<span class="driverperf-allreal">all liveries present</span>';
  return '<span class="driverperf-phantom-count" title="' + esc(DRIVERPERF_PHANTOM_TITLE) + '">' +
    n + (n === 1 ? ' entry AMS2 ignores' : ' entries AMS2 ignores') + '</span>';
}

function renderDriverPerfClass(cls, idx, attrs) {
  var drivers = cls.drivers || [];
  var tableId = 'driverperf-table-' + idx;
  var col = 0;
  function th(cls2, type, label, title) {
    return '<th class="' + cls2 + '" data-col="' + (col++) + '" data-type="' + type + '"' +
      (title ? ' title="' + esc(title) + '"' : '') + '>' + esc(label) + '</th>';
  }
  var thead = '<tr>' +
    th('stat-name sort-asc', 'str', 'Driver') +
    th('stat-name', 'str', 'Team') +
    th('stat-name', 'str', 'Tracks', 'Blank means the entry applies everywhere') +
    th('stat-num', 'num', 'Rating', driverPerfRatingTitle) +
    attrs.map(function (f) { return th('stat-num', 'num', driverPerfLabel(f), f); }).join('') +
    '</tr>';
  var tbody = drivers.map(function (d) {
    var a = d.attrs || {};
    var phantom = d.phantom === true;
    var mark = phantom
      ? ' <span class="driverperf-phantom-tag" title="' + esc(DRIVERPERF_PHANTOM_TITLE) + '">no livery</span>'
      : '';
    return '<tr data-index="' + d.index + '" data-driver="' + esc(d.driver) + '"' +
      (phantom ? ' class="driverperf-phantom"' : '') + '>' +
      '<td class="stat-name" title="' + esc(d.livery || '') + '">' + esc(d.driver) + mark + '</td>' +
      '<td class="stat-name driverperf-team">' + esc(d.team) + '</td>' +
      '<td class="stat-name driverperf-tracks" title="' + esc(d.tracks || '') + '">' +
        esc(d.tracks || '') + '</td>' +
      driverPerfRatingCell(d.rating) +
      attrs.map(function (f) { return driverPerfCell(f, a[f]); }).join('') +
      '</tr>';
  }).join('');
  return '<section class="carperf-class" data-carperf-class="' + esc(cls.class) + '">' +
    '<h3 class="carperf-heading">' + carPerfClassLabel(cls) +
    ' <span class="carperf-year">(' + drivers.length + ' entries)</span> ' +
    driverPerfHeadingNote(drivers) + '</h3>' +
    '<div class="driverperf-scroll">' +
    '<table class="stats-table sortable driverperf-table" id="' + tableId + '">' +
    '<thead>' + thead + '</thead><tbody>' + tbody + '</tbody></table>' +
    '</div></section>';
}

function driverPerfFilterBarHtml(classes, preselect) {
  var options = classes.map(function (cls) {
    var on = !preselect || preselect[cls.class];
    return '<label class="carperf-filter-check">' +
      '<input type="checkbox" class="driverperf-filter-input" value="' + esc(cls.class) + '"' +
      (on ? ' checked' : '') + '> ' +
      carPerfClassLabel(cls) +
      '</label>';
  }).join('');
  return '<div class="carperf-filter-bar">' +
    '<div class="carperf-filter-actions">' +
      '<button type="button" class="manage-btn" id="driverperf-filter-all">All</button>' +
      '<button type="button" class="manage-btn" id="driverperf-filter-none">None</button>' +
    '</div>' +
    '<div class="carperf-filter-checks">' + options + '</div>' +
    '<span id="driverperf-status" class="carperf-status"></span>' +
    '</div>';
}

function driverPerfApplyFilter() {
  var checked = {};
  document.querySelectorAll('.driverperf-filter-input').forEach(function (input) {
    checked[input.value] = input.checked;
  });
  document.querySelectorAll('#driverperf-classes .carperf-class').forEach(function (section) {
    var show = checked[section.dataset.carperfClass] !== false;
    section.classList.toggle('carperf-class-hidden', !show);
  });
}

function driverPerfSave(input) {
  var tr = input.closest('tr');
  var section = tr && tr.closest('.carperf-class');
  if (!section) return;
  var raw = input.value.trim();
  if (raw === '') {
    // Clearing a cell would have to mean deleting the tag, which is not offered — put the
    // stored value back rather than guessing.
    input.value = input.dataset.stored || '';
    driverPerfStatus('Clearing an attribute is not supported — the value was restored.', true);
    return;
  }
  var field = input.dataset.field;
  var range = driverPerfRange(field);
  var v = parseFloat(raw);
  if (!isFinite(v) || v < range[0] || v > range[1]) {
    // Nothing was written, so the cell must not keep showing the rejected number.
    input.value = input.dataset.stored || '';
    driverPerfStatus(driverPerfLabel(field) + ' must be between ' + range[0].toFixed(2) +
      ' and ' + range[1].toFixed(2) + '.', true);
    return;
  }
  input.classList.add('driverperf-saving');
  driverPerfStatus('Saving ' + tr.dataset.driver + '…');
  fetch('/api/driver-performance', {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      class: section.dataset.carperfClass,
      index: +tr.dataset.index,
      driver: tr.dataset.driver,
      field: field,
      value: v
    })
  }).then(function (r) {
    return r.json().then(function (d) { return { ok: r.ok, data: d }; });
  }).then(function (res) {
    input.classList.remove('driverperf-saving');
    if (!res.ok) {
      input.value = input.dataset.stored || '';
      driverPerfStatus((res.data && res.data.error) || 'Save failed.', true);
      return;
    }
    var stored = res.data && res.data.attrs && res.data.attrs[field];
    if (stored !== undefined && stored !== null) {
      input.value = stored.toFixed(2);
      input.dataset.stored = input.value;
    }
    // The edit feeds the composite, so the Rating cell has to move with it.
    var cell = tr.querySelector('.driverperf-rating');
    if (cell) {
      var r = res.data && res.data.rating;
      cell.textContent = (r === null || r === undefined) ? '—' : Math.round(r);
      cell.className = 'stat-num driverperf-rating' +
        ((r === null || r === undefined) ? ' driverperf-rating-none' : '');
    }
    driverPerfStatus('Saved ' + driverPerfLabel(field) + ' for ' + tr.dataset.driver);
  }).catch(function () {
    input.classList.remove('driverperf-saving');
    driverPerfStatus('Save failed — is the server still running?', true);
  });
}

function renderDriverPerformance(data) {
  var classes = (data && data.classes) || [];
  var attrs = (data && data.attrs) || [];
  driverPerfSetLimits(data);
  driverPerfBuildRatingTitle(data && data.rating_weights);
  var container = document.getElementById('driverperf-container');
  if (!container) return;
  if (!classes.length) {
    container.innerHTML = '<div class="manage-placeholder" style="padding:2rem">' +
      'No Custom AI Driver files found. Set the Custom AI Drivers folder in the Config tab ' +
      '(e.g. <code>...\\Automobilista 2\\UserData\\CustomAIDrivers</code>).</div>';
    return;
  }
  var caption = '<p class="carperf-note">Every AI attribute AMS2 reads per driver, straight from ' +
    'that class’s Custom AI Drivers XML. Edit a cell and it is written back on the spot; the ' +
    'original file is kept once as <code>.xml.bak</code>, and AMS2 re-reads it when a session ' +
    'loads, so restart the session for an edit to take effect. Reiza documents every ' +
    'personality value as 0.00–1.00 inclusive, and <code>vehicle_reliability</code> as the one ' +
    'that may go outside it — hover a cell for the range it accepts. ' +
    'A blank cell means the entry does not declare that attribute, so AMS2 uses its own default — ' +
    'type a number to add it. A row with Tracks filled in only applies at those circuits, and may ' +
    'name a stand-in driver for that race; its values can be negative, as offsets from the ' +
    'driver’s usual figure. Hover a column heading for the raw XML tag, or a driver name for ' +
    'the livery_name it binds to. ' +
    'An entry marked <span class="driverperf-phantom-tag">no livery</span> has a ' +
    '<code>livery_name</code> AMS2 owns no car for, so it is silently ignored and that driver ' +
    'never reaches a grid — a roster can name more drivers than the class has cars. This is ' +
    'checked against the livery manifests your livery mods install; a class no mod covers reads ' +
    'as “liveries not verifiable” rather than guessing. The entries that do have a livery are ' +
    'the cars this class can field, so their count is the grid size to race it at — one fewer ' +
    'opponent than that, since one of the seats is yours. ' +
    'Rating is a 0–100 composite of the entry’s own attributes, weighted toward race pace — ' +
    'hover the column for the exact weights. It is a summary of what the file declares, not a ' +
    'measure of results, so it is not the same quantity as the driver ratings in the Car ' +
    'Performance tab even though both run 0–100.</p>';
  // Shares carPerfPreselected: both tabs filter the same class list the same way.
  var preselect = carPerfPreselected(classes, data && data.active_classes);
  container.innerHTML = caption + driverPerfFilterBarHtml(classes, preselect) +
    '<div id="driverperf-classes">' +
    classes.map(function (cls, idx) { return renderDriverPerfClass(cls, idx, attrs); }).join('') +
    '</div>';
  classes.forEach(function (cls, idx) {
    initSortableTableEl(document.getElementById('driverperf-table-' + idx));
  });
  driverPerfApplyFilter();
  if (preselect) {
    var note = document.getElementById('driverperf-status');
    if (note) {
      note.className = 'carperf-status carperf-status-note';
      note.textContent = 'Showing the class you are racing — All shows every class.';
    }
  }
  document.querySelectorAll('.driverperf-attr').forEach(function (input) {
    input.dataset.stored = input.value;
    input.addEventListener('change', function () { driverPerfSave(input); });
  });
  document.querySelectorAll('.driverperf-filter-input').forEach(function (input) {
    input.addEventListener('change', driverPerfApplyFilter);
  });
  var allBtn = document.getElementById('driverperf-filter-all');
  var noneBtn = document.getElementById('driverperf-filter-none');
  if (allBtn) allBtn.addEventListener('click', function () {
    document.querySelectorAll('.driverperf-filter-input').forEach(function (i) { i.checked = true; });
    driverPerfApplyFilter();
  });
  if (noneBtn) noneBtn.addEventListener('click', function () {
    document.querySelectorAll('.driverperf-filter-input').forEach(function (i) { i.checked = false; });
    driverPerfApplyFilter();
  });
}

function loadDriverPerformance() {
  fetch('/api/driver-performance').then(function (r) { return r.json(); })
    .then(function (data) {
      renderDriverPerformance(data || {});
    }).catch(function () {
      var el = document.getElementById('driverperf-container');
      if (el) el.innerHTML = '<div class="manage-placeholder" style="padding:2rem">Driver performance data requires the server binary.</div>';
    });
}

document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () {
    if (btn.dataset.tab === 'driverperf') loadDriverPerformance();
  });
});
