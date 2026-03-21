(function () {
  // ── Tab switching ──────────────────────────────────────────────────────────
  document.querySelectorAll('.tab-btn').forEach(function (btn) {
    btn.addEventListener('click', function () {
      document.querySelectorAll('.tab-btn').forEach(function (b) {
        b.classList.remove('tab-active');
      });
      document.querySelectorAll('.tab-panel').forEach(function (p) {
        p.classList.add('tab-panel-hidden');
      });
      btn.classList.add('tab-active');
      document.getElementById('tab-' + btn.dataset.tab).classList.remove('tab-panel-hidden');
    });
  });

  // ── Sortable stats table ───────────────────────────────────────────────────
  var table = document.getElementById('stats-table');
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
      if (av < bv) return asc ? -1 : 1;
      if (av > bv) return asc ? 1 : -1;
      return 0;
    });
    rows.forEach(function (r) { tbody.appendChild(r); });
    Array.from(headers).forEach(function (th) {
      th.classList.remove('sort-asc', 'sort-desc');
    });
    headers[col].classList.add(asc ? 'sort-asc' : 'sort-desc');
    sortCol = col; sortAsc = asc;
  }

  Array.from(headers).forEach(function (th) {
    th.addEventListener('click', function () {
      sort(+th.dataset.col, th.dataset.type);
    });
  });
}());
