// ── Car Performance tab ───────────────────────────────────────────────────────

function carPerfPaceLabel(pct) {
  if (pct <= 0.0001) return '<span class="carperf-fastest">Fastest</span>';
  return '+' + pct.toFixed(1) + '%';
}

function carPerfClassLabel(cls) {
  return esc(cls.class) + (cls.year ? ' <span class="carperf-year">(' + cls.year + ')</span>' : '');
}

function renderCarPerformanceClass(cls, idx) {
  var cars = cls.cars || [];
  var tableId = 'carperf-table-' + idx;
  var col = 0;
  function th(cls2, type, label) { return '<th class="' + cls2 + '" data-col="' + (col++) + '" data-type="' + type + '">' + label + '</th>'; }
  var thead = '<tr>' +
    th('stat-name sort-asc', 'str', 'Team') +
    th('stat-num', 'num', 'Power') +
    th('stat-num', 'num', 'Weight') +
    th('stat-num', 'num', 'Drag') +
    th('stat-num', 'num', 'Est. Pace') +
    '</tr>';
  var tbody = cars.map(function (c) {
    return '<tr>' +
      '<td class="stat-name">' + esc(c.team) + '</td>' +
      '<td class="stat-num">' + c.power_scalar.toFixed(2) + '</td>' +
      '<td class="stat-num">' + c.weight_scalar.toFixed(2) + '</td>' +
      '<td class="stat-num">' + c.drag_scalar.toFixed(2) + '</td>' +
      '<td class="stat-num">' + carPerfPaceLabel(c.pace_delta_pct) + '</td>' +
      '</tr>';
  }).join('');
  return '<section class="carperf-class" data-carperf-class="' + esc(cls.class) + '">' +
    '<h3 class="carperf-heading">' + carPerfClassLabel(cls) + '</h3>' +
    '<table class="stats-table sortable" id="' + tableId + '">' +
    '<thead>' + thead + '</thead><tbody>' + tbody + '</tbody></table>' +
    '</section>';
}

function carPerfFilterBarHtml(classes) {
  var options = classes.map(function (cls) {
    return '<label class="carperf-filter-check">' +
      '<input type="checkbox" class="carperf-filter-input" value="' + esc(cls.class) + '" checked> ' +
      carPerfClassLabel(cls) +
      '</label>';
  }).join('');
  return '<div class="carperf-filter-bar">' +
    '<div class="carperf-filter-actions">' +
      '<button type="button" class="manage-btn" id="carperf-filter-all">All</button>' +
      '<button type="button" class="manage-btn" id="carperf-filter-none">None</button>' +
    '</div>' +
    '<div class="carperf-filter-checks">' + options + '</div>' +
    '</div>';
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

function renderCarPerformance(classes) {
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
    'Classes are always listed in chronological order by the season they model.</p>';
  container.innerHTML = caption + carPerfFilterBarHtml(classes) +
    '<div id="carperf-classes">' + classes.map(renderCarPerformanceClass).join('') + '</div>';
  classes.forEach(function (cls, idx) {
    initSortableTableEl(document.getElementById('carperf-table-' + idx));
  });
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
    .then(function (classes) {
      renderCarPerformance(classes || []);
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
