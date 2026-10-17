// ── Championship sort: Active first, then Progress, then Final; alpha within group ──
var CHAMP_STATUS_ORDER = { Active: 0, Progress: 1, Final: 2 };
function sortChamps(champs) {
  return champs.slice().sort(function (a, b) {
    var ao = CHAMP_STATUS_ORDER[a.status] ?? 3;
    var bo = CHAMP_STATUS_ORDER[b.status] ?? 3;
    return ao !== bo ? ao - bo : a.name.localeCompare(b.name);
  });
}

// ── Shared constants ──────────────────────────────────────────────────────────
var SESSION_TYPE_LABELS = { 1: 'P', 3: 'Q', 5: 'R' };
var SESSION_TYPE_NAMES  = { 1: 'Practice', 3: 'Qualify', 5: 'Race' };

// ── Livery pictures ───────────────────────────────────────────────────────────
// The livery mod's own picture of a car, decoded from its .dds by /api/livery-preview. `path`
// is whatever the server handed out for that team — `preview` on a Car Performance row, or the
// `previews` map beside a season's offers. Nothing is drawn without one: a class with no livery
// mod, and an entry declaring no PREVIEWIMAGE, are both ordinary rather than errors.
//
// `loading="lazy"` is load-bearing. A grid is two dozen pictures the server decodes on demand,
// and the ones below the fold are usually never asked for at all.
function liveryImg(path, cls) {
  if (!path) return '';
  return '<img class="' + cls + '" loading="lazy" decoding="async" alt=""' +
    ' src="/api/livery-preview/' + encodeURIComponent(path) + '">';
}

// ── Entries AMS2 will never field ─────────────────────────────────────────────
// A `livery_name` the game owns no car for is silently ignored, so the entry exists in the
// roster file and nowhere else. Both performance tabs mark those rows "no livery", and both
// hide them by default: they are noise in a table about how the grid performs, and the person
// who wants to see them is the one repairing the roster.
//
// One preference asked twice, not two preferences: the checkbox appears in each tab's filter
// bar and they are kept in step, because both tabs are showing the same entries.
//
// Hiding is done with a class on <body> rather than per row, so one rule covers both tabs and
// a table rebuilt after an edit comes back in whatever state was already chosen.
var SHOW_PHANTOM_ENTRIES = false;

var PHANTOM_TOGGLE_TITLE = 'These entries name a livery AMS2 does not own, so the game ignores ' +
  'them: they never reach a grid, and nothing about them affects a result. Show them to fix ' +
  'the roster.';

// `any` only decides whether the control is worth drawing — a checkbox that would hide nothing
// is worse than no checkbox. The count belongs on each class heading, not here: this tab-wide
// control would otherwise quote a total dominated by classes the class filter has hidden.
function phantomToggleHtml(label, any) {
  if (!any) return '';
  return '<label class="carperf-filter-check" title="' + esc(PHANTOM_TOGGLE_TITLE) + '">' +
    '<input type="checkbox" class="phantom-toggle-input"' +
    (SHOW_PHANTOM_ENTRIES ? ' checked' : '') + '> ' + esc(label) + '</label>';
}

function applyPhantomFilter() {
  document.body.classList.toggle('hide-phantom-entries', !SHOW_PHANTOM_ENTRIES);
}

function initPhantomToggles() {
  document.querySelectorAll('.phantom-toggle-input').forEach(function (input) {
    input.addEventListener('change', function () {
      SHOW_PHANTOM_ENTRIES = input.checked;
      document.querySelectorAll('.phantom-toggle-input').forEach(function (other) {
        other.checked = SHOW_PHANTOM_ENTRIES;
      });
      applyPhantomFilter();
    });
  });
  applyPhantomFilter();
}

// ── Shared utilities ──────────────────────────────────────────────────────────
function esc(str) {
  return String(str)
    .replace(/&/g, '&amp;').replace(/</g, '&lt;')
    .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function fmtTrack(s) {
  return s.track_variation ? esc(s.track) + ' \u2013 ' + esc(s.track_variation) : esc(s.track);
}

function fmtDate(ts) {
  if (!ts) return '';
  var d = new Date(ts * 1000);
  return d.getFullYear() + '-' +
    String(d.getMonth() + 1).padStart(2, '0') + '-' +
    String(d.getDate()).padStart(2, '0') + ' ' +
    String(d.getHours()).padStart(2, '0') + ':' +
    String(d.getMinutes()).padStart(2, '0');
}

function fmtLapTime(t) {
  if (!t || t <= 0) return '<span class="no-time">\u2014</span>';
  var m = Math.floor(t / 60);
  var s = t % 60;
  var ss = s.toFixed(3);
  if (parseFloat(ss) < 10) ss = '0' + ss;
  return m > 0 ? m + ':' + ss : ss;
}

function sessionWinner(session) {
  if (!session.results || !session.results.length) return '\u2014';
  var w = session.results.find(function (r) { return r.race_position === 1; });
  return w ? w.name : '\u2014';
}

// ── Sortable tables ───────────────────────────────────────────────────────────
function initSortableTableEl(table) {
  if (!table) return;
  var tbody = table.tBodies[0];
  var headers = table.tHead.rows[0].cells;
  var sortCol = 0, sortAsc = true;
  function cellVal(row, col, type) {
    // An editable cell holds its value in an input, where textContent is empty.
    var cell = row.cells[col];
    var input = cell.querySelector('input');
    var text = (input ? input.value : cell.textContent).trim();
    return type === 'num' ? (parseFloat(text) || 0) : text.toLowerCase();
  }
  function sort(col, type) {
    var rows = Array.from(tbody.rows);
    var asc = (col === sortCol) ? !sortAsc : (type === 'num' ? false : true);
    rows.sort(function (a, b) {
      var av = cellVal(a, col, type), bv = cellVal(b, col, type);
      return av < bv ? (asc ? -1 : 1) : av > bv ? (asc ? 1 : -1) : 0;
    });
    rows.forEach(function (r) { tbody.appendChild(r); });
    Array.from(headers).forEach(function (th) { th.classList.remove('sort-asc', 'sort-desc'); });
    headers[col].classList.add(asc ? 'sort-asc' : 'sort-desc');
    sortCol = col; sortAsc = asc;
  }
  Array.from(headers).forEach(function (th) {
    th.style.cursor = 'pointer';
    th.addEventListener('click', function () { sort(+th.dataset.col, th.dataset.type); });
  });
}

function initSortableTable() { initSortableTableEl(document.getElementById('stats-table')); }

// ── Sub-tab switching (scoped per parent tab) ─────────────────────────────────
function initSubTabs(parentId, btnAttr, panelPrefix) {
  var parent = document.getElementById(parentId);
  if (!parent) return;
  parent.querySelectorAll('.sub-tab-btn').forEach(function (btn) {
    btn.addEventListener('click', function () {
      parent.querySelectorAll('.sub-tab-btn').forEach(function (b) { b.classList.remove('sub-tab-active'); });
      parent.querySelectorAll('.sub-tab-panel').forEach(function (p) { p.classList.add('sub-tab-panel-hidden'); });
      btn.classList.add('sub-tab-active');
      document.getElementById(panelPrefix + btn.dataset[btnAttr]).classList.remove('sub-tab-panel-hidden');
    });
  });
}
