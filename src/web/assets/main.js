// ── Tab switching ─────────────────────────────────────────────────────────────
function showTab(name) {
  var panel = document.getElementById('tab-' + name);
  if (!panel) return;
  document.querySelectorAll('.tab-btn').forEach(function (b) { b.classList.remove('tab-active'); });
  document.querySelectorAll('.tab-panel').forEach(function (p) { p.classList.add('tab-panel-hidden'); });
  var btn = document.querySelector('.tab-btn[data-tab="' + name + '"]');
  if (btn) btn.classList.add('tab-active');
  panel.classList.remove('tab-panel-hidden');
  // A roster edited in another tab may now differ from the one the season was signed on, and
  // the live banner only re-asks when the grid changes — so ask again on the way in.
  if (name === 'live') loadLiveTeams();
}

document.querySelectorAll('.tab-btn').forEach(function (btn) {
  btn.addEventListener('click', function () { showTab(btn.dataset.tab); });
});

// ── Sub-tab init ──────────────────────────────────────────────────────────────
initSubTabs('tab-career', 'careerSub', 'career-sub-');
