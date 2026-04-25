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
    var text = row.cells[col].textContent.trim();
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
