// ── Contracts: seat offers and the career ledger ──────────────────────────────
//
// Two views over the same feature. The Manage tab shows what this season's grid will offer and
// takes the signing; the Career tab shows what every signed season paid. Both are read straight
// from the server on each open — offers and money are derived there, never cached here.

// A badge only where there is something to say. Most seats are an ordinary paid deal, and a
// column reading "PAID" on every row is decoration rather than information — it was exactly what
// made the old four-kind table hard to read. Only the two exceptions are marked: a seat bought
// with sponsorship, and a team re-signing its own driver.
function offerLabel(o) {
  return o.renewal ? 'Renewal' : o.kind === 'pay' ? 'Pay driver' : '';
}

function offerClass(o) {
  return o.renewal ? 'renewal' : o.kind;
}

function fmtCredits(n) {
  var neg = n < 0;
  var s = String(Math.abs(Math.round(n)));
  var out = '';
  while (s.length > 3) {
    out = ',' + s.slice(-3) + out;
    s = s.slice(0, -3);
  }
  return (neg ? '-' : '') + s + out;
}

function fmtTarget(pos) {
  return pos ? 'finish P' + pos + ' or better' : 'no target';
}

function ordinal(n) {
  var teens = n % 100;
  if (teens >= 11 && teens <= 13) return n + 'th';
  var last = n % 10;
  return n + (last === 1 ? 'st' : last === 2 ? 'nd' : last === 3 ? 'rd' : 'th');
}

/// Why this offer reads the way it does, in plain words.
///
/// Every part comes from a field already in the payload. Nothing here recomputes a rate or a
/// threshold — those are the server's, and a second copy in the browser would drift from them.
/// What it can say without duplicating anything is *which* rule applied.
function offerWhy(o, data) {
  var bits = [];
  // `rank` is a position among *teams*, not cars — which is why a 9th-place team expects to
  // finish somewhere around P18 on a grid running two cars each. Saying "team" rather than
  // "car" is what makes those two numbers agree instead of looking contradictory.
  if (data.teams) {
    bits.push(ordinal(o.rank + 1) + '-fastest of ' + data.teams + ' teams');
  }
  bits.push('its car should finish around P' + Math.round(o.expected_position));

  var asks = Math.round(o.required);
  var you = Math.round(data.reputation);
  if (o.kind === 'pay') {
    bits.push('wants a rating of ' + asks + ' and you have ' + you +
      ' — they will take sponsorship instead, which only teams this far back do');
  } else if (o.renewal) {
    var st = data.standing || {};
    bits.push('your own team: they re-sign you whatever the rating says' +
      (st.delivered === true ? ', and pay more for the seasons you have served' : ''));
  } else if (you > asks) {
    bits.push('wants ' + asks + ' and you have ' + you +
      ' — clear of their bar, so they pay over their rate');
  } else if (you < asks) {
    // Without this the row looks wrong: a team asking 57 is offering a seat to a 50. It is the
    // rating's offer margin, and it used to be signalled by a "trial" badge that no longer
    // exists — so the sentence has to carry it.
    bits.push('wants ' + asks + ' and you have ' + you +
      ' — short of their bar but within reach, so they will take you at their rate');
  } else {
    bits.push('exactly on their bar of ' + asks + ', so they pay their rate and no more');
  }

  if (!o.objective) bits.push('too far back for a finishing target to mean anything');
  return bits.join(' &middot; ');
}

// ── Manage tab: this season's offers ──────────────────────────────────────────

function loadOffers(champId) {
  var panel = document.getElementById('champ-contract-panel');
  if (!panel) return;
  fetch('/api/championships/' + champId + '/offers')
    .then(function (r) { return r.json(); })
    .then(function (data) {
      // The user may have selected another championship while this was in flight.
      if (manageState.selectedId !== champId) return;
      renderOffers(champId, data);
    })
    .catch(function () {});
}

function renderOffers(champId, data) {
  var panel = document.getElementById('champ-contract-panel');
  if (!panel) return;
  // Switched off, or no roster to rate against: the team picker above is the whole story.
  if (!data || !data.enabled) { panel.innerHTML = ''; return; }
  if (!data.rated) {
    panel.innerHTML = '<div class="contract-box"><div class="contract-note">' +
      'Assign a Custom AI Drivers file to see what the grid will offer.</div></div>';
    return;
  }

  if (data.signed) { renderSignedDeal(champId, data); return; }
  if (!data.open) {
    panel.innerHTML = '<div class="contract-box"><div class="contract-note">' +
      'This season has started, so its seat is settled.</div></div>';
    return;
  }

  var rows = (data.offers || []).map(function (o) {
    var afford = !o.buy_in || data.balance >= o.buy_in;
    var terms = fmtCredits(o.salary) + ' &middot; ' + fmtTarget(o.objective);
    return '<tr class="contract-row contract-row-' + esc(offerClass(o)) + '">' +
      '<td class="contract-team">' + esc(o.team) + '</td>' +
      '<td>' + (offerLabel(o)
        ? '<span class="contract-kind contract-kind-' + esc(offerClass(o)) + '">' +
          esc(offerLabel(o)) + '</span>'
        : '') + '</td>' +
      '<td class="contract-terms">' + terms + '</td>' +
      '<td class="contract-price">' +
        (o.buy_in ? (afford ? '' : '<span class="contract-unaffordable">') +
                    fmtCredits(o.buy_in) + ' in sponsorship' + (afford ? '' : '</span>') : '') +
      '</td>' +
      '<td><button class="manage-btn manage-btn-primary contract-sign-btn"' +
        ' data-team="' + esc(o.team) + '"' + (afford ? '' : ' disabled') + '>Sign</button></td>' +
      '</tr>' +
      // A second row rather than more columns: the explanation is a sentence, and a sentence in
      // a column makes every other column narrow.
      '<tr class="contract-why-row"><td colspan="5" class="contract-why">' +
        offerWhy(o, data) +
        (afford ? '' : ' &middot; <b>more than the career is worth</b>') +
      '</td></tr>';
  }).join('');

  panel.innerHTML = '<div class="contract-box">' +
    '<div class="contract-header">' +
      '<span class="contract-title">Seats on offer</span>' +
      '<span class="contract-note">' + contractSummary(data) + '</span>' +
    '</div>' +
    (rows
      ? '<table class="contract-table"><tbody>' + rows + '</tbody></table>'
      : '<div class="contract-note">No team on this grid will offer a seat yet.</div>') +
    '</div>';

  panel.querySelectorAll('.contract-sign-btn').forEach(function (btn) {
    btn.addEventListener('click', function () { signSeat(champId, btn.dataset.team); });
  });
}

function contractSummary(data) {
  var bits = ['Rating ' + Math.round(data.reputation) + '/100',
              'Balance ' + fmtCredits(data.balance)];
  var st = data.standing || {};
  if (st.incumbent) {
    // A seat is only held inside the series it was held in, so say when it was another one —
    // otherwise the missing renewal looks like a bug rather than a rule.
    var elsewhere = st.class !== data.class;
    bits.push('Last seat: ' + st.incumbent +
      (elsewhere ? ' (' + st.class + ' — different series, no renewal)'
        : st.delivered === true ? ' (target met)'
        : st.delivered === false ? ' (target missed)' : ''));
  }
  return esc(bits.join(' · '));
}

function renderSignedDeal(champId, data) {
  var panel = document.getElementById('champ-contract-panel');
  var c = data.signed;
  // Releasing is only possible while the season is unraced — the same rule that locks the team.
  var canRelease = data.open || !data.offers;
  panel.innerHTML = '<div class="contract-box contract-box-signed">' +
    '<div class="contract-header">' +
      '<span class="contract-title">Signed &mdash; ' + esc(c.team) + '</span>' +
      '<span class="contract-note">' +
        fmtCredits(c.salary) + ' · ' + fmtTarget(c.objective) +
        (c.bought_for ? ' · ' + fmtCredits(c.bought_for) + ' in sponsorship' : '') +
      '</span>' +
      (canRelease
        ? '<button class="manage-btn manage-btn-danger contract-release-btn">Tear up</button>'
        : '') +
    '</div></div>';
  var btn = panel.querySelector('.contract-release-btn');
  if (btn) btn.addEventListener('click', function () { releaseSeat(champId, c.team); });
}

function signSeat(champId, team) {
  fetch('/api/championships/' + champId + '/sign', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ team: team }),
  }).then(function (r) {
    return r.json().then(function (body) { return { ok: r.ok, body: body }; });
  }).then(function (r) {
    // 409 carries the reason the seat is not available — the requirement, or the price.
    if (!r.ok) { alert(r.body.error || 'Could not sign.'); return; }
    loadManage();
  }).catch(function () { alert('Could not sign.'); });
}

function releaseSeat(champId, team) {
  if (!confirm('Tear up the contract with ' + team + '? The seat goes with it.')) return;
  fetch('/api/championships/' + champId + '/sign', { method: 'DELETE' })
    .then(function () { loadManage(); })
    .catch(function () {});
}

// ── Career tab: the ledger ────────────────────────────────────────────────────

function loadFinances() {
  var box = document.getElementById('career-contracts-container');
  if (!box) return;
  fetch('/api/career/finances').then(function (r) { return r.json(); })
    .then(function (f) { renderFinances(f); })
    .catch(function () {});
}

function renderFinances(f) {
  var box = document.getElementById('career-contracts-container');
  if (!box) return;
  if (!f || !f.enabled) {
    box.innerHTML = '<div class="career-empty">Contracts belong to a singleplayer career. ' +
      'Create one under <em>Config &rarr; Career Save Files</em> to sign for a team and track earnings.</div>';
    return;
  }
  if (!f.seasons || !f.seasons.length) {
    box.innerHTML = '<div class="career-empty">No seasons under contract yet. ' +
      'Sign for a team on the Manage tab.</div>';
    return;
  }

  var rows = f.seasons.map(function (s) {
    var result = s.position ? 'P' + s.position + ' of ' + s.field : '—';
    var target = s.objective
      ? 'P' + s.objective +
        (s.objective_met === true ? ' ✓' : s.objective_met === false ? ' ✗' : '')
      : '—';
    var cls = s.objective_met === true ? ' ledger-met'
      : s.objective_met === false ? ' ledger-missed' : '';
    return '<tr class="' + cls + '">' +
      '<td>' + esc(s.name || '(deleted)') + '</td>' +
      '<td>' + esc(s.team) + '</td>' +
      '<td>' + (s.complete ? 'Complete' : 'Racing') + '</td>' +
      '<td class="num">' + result + '</td>' +
      '<td class="num">' + target + '</td>' +
      '<td class="num">' + fmtCredits(s.salary) + '</td>' +
      '<td class="num">' + fmtCredits(s.prize) + '</td>' +
      '<td class="num">' + (s.bought_for ? '-' + fmtCredits(s.bought_for) : '') + '</td>' +
      '</tr>';
  }).join('');

  box.innerHTML =
    '<div class="ledger-totals">' +
      '<span class="ledger-total"><b>' + fmtCredits(f.balance) + '</b> balance</span>' +
      (f.starting ? '<span class="ledger-total">' + fmtCredits(f.starting) + ' started with</span>' : '') +
      '<span class="ledger-total">' + fmtCredits(f.earned) + ' earned</span>' +
      '<span class="ledger-total">' + fmtCredits(f.spent) + ' spent</span>' +
    '</div>' +
    '<p class="config-note">A season pays out when its championship is marked <em>Final</em>. ' +
      'Everything here is worked out from results as they stand, so reopening a season takes its ' +
      'payout back.</p>' +
    '<table class="ledger-table"><thead><tr>' +
      '<th>Season</th><th>Team</th><th>State</th><th class="num">Result</th>' +
      '<th class="num">Target</th><th class="num">Salary</th><th class="num">Prize</th>' +
      '<th class="num">Sponsorship</th>' +
    '</tr></thead><tbody>' + rows + '</tbody></table>';
}

document.querySelectorAll('[data-career-sub="contracts"]').forEach(function (btn) {
  btn.addEventListener('click', loadFinances);
});
