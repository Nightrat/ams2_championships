// ── Tab switching ─────────────────────────────────────────────────────────────
document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () {
    document.querySelectorAll('.tab-btn').forEach(function (b) { b.classList.remove('tab-active'); });
    document.querySelectorAll('.tab-panel').forEach(function (p) { p.classList.add('tab-panel-hidden'); });
    btn.classList.add('tab-active');
    document.getElementById('tab-' + btn.dataset.tab).classList.remove('tab-panel-hidden');
  });
});

// ── Sub-tab init ──────────────────────────────────────────────────────────────
initSubTabs('tab-career', 'careerSub', 'career-sub-');

