// Career → Paddock: a season's story, as GET /api/championships/:id/story tells it. Everything
// here is worked out server-side (service::story); this file only draws it.

// The season the Paddock is showing, so a story arriving after the career was switched is dropped.
var paddockChampId = null;

// "3–1", with the leader's figure picked out. `lowerWins` is for averages, where less is better.
function storyScore(pair, fmt, lowerWins) {
  fmt = fmt || function (v) { return v; };
  var a = pair.player, b = pair.teammate;
  var mine = a !== null && (b === null || (lowerWins ? a < b : a > b));
  var theirs = b !== null && (a === null || (lowerWins ? b < a : b > a));
  function cell(v, lead) {
    var text = v === null ? '—' : esc(String(fmt(v)));
    return lead ? '<strong class="story-lead">' + text + '</strong>' : text;
  }
  return cell(a, mine) + '–' + cell(b, theirs);
}

// The teammate card.
function teammateCardHtml(view) {
  var title = '<div class="contract-title">Teammate' + (view.team ? ' — ' + esc(view.team) : '') + '</div>';
  if (view.reason) {
    return '<div class="contract-box story-box">' + title +
      '<div class="contract-note">' + esc(view.reason) + '</div></div>';
  }
  var oneDp = function (v) { return v.toFixed(1); };
  var rows = view.head_to_head.map(function (h) {
    var shared = h.races + h.qualifyings > 0;
    return '<tr>' +
      '<td class="contract-team">' + esc(h.teammate) + '</td>' +
      (shared
        ? '<td>' + storyScore(h.qualifying) + ' <span class="contract-note">of ' + h.qualifyings + '</span></td>' +
          '<td>' + storyScore(h.race) + ' <span class="contract-note">of ' + h.races + '</span></td>' +
          '<td>' + storyScore(h.points) + '</td>' +
          '<td>' + storyScore(h.average_finish, oneDp, true) + '</td>' +
          '<td>' + storyScore(h.retirements, null, true) + '</td>'
        : '<td colspan="5" class="contract-note">No session together yet.</td>') +
      '</tr>';
  }).join('');
  return '<div class="contract-box story-box">' + title +
    (view.summary ? '<div class="story-summary">' + esc(view.summary) + '</div>' : '') +
    '<table class="contract-table story-table">' +
      '<thead><tr><th></th><th>Qualifying</th><th>Races</th><th>Points</th><th>Avg finish</th><th>Retired</th></tr></thead>' +
      '<tbody>' + rows + '</tbody>' +
    '</table>' +
    '<div class="contract-note">You first, then your teammate. Only sessions you both drove count.</div>' +
    '</div>';
}

// Opens the Paddock on the season being raced — the `Active` one, whose story is still being
// written. There is no picker. Between seasons it shows the review of the season just finished,
// so the page is never empty after the flag.
function loadPaddock() {
  var box = document.getElementById('career-paddock-container');
  if (!box) return;
  fetch('/api/championships').then(function (r) { return r.json(); })
    .then(function (champs) {
      var active = (champs || []).find(function (c) { return c.status === 'Active'; });
      paddockChampId = active ? active.id : null;
      if (active) {
        box.innerHTML = '<h2 class="paddock-season">' + esc(active.name) + '</h2>' +
          '<div id="paddock-story"></div>';
        loadPaddockStory(active.id);
        return;
      }
      return fetch('/api/career/story').then(function (r) { return r.json(); }).then(function (career) {
        var last = career.latest_finished;
        var name = last && (champs || []).find(function (c) { return c.id === last; });
        if (!name) {
          box.innerHTML = '<div class="career-empty">No season is being raced. Start one on the Manage tab.</div>';
          return;
        }
        paddockChampId = last;
        box.innerHTML = '<h2 class="paddock-season">' + esc(name.name) + ' — finished</h2>' +
          '<div class="contract-note story-line">No season is being raced. Start the next one on the Manage tab.</div>' +
          '<div id="paddock-story"></div>';
        fetch('/api/championships/' + encodeURIComponent(last) + '/story')
          .then(function (r) { return r.ok ? r.json() : null; })
          .then(function (story) {
            var target = document.getElementById('paddock-story');
            if (!story || !target || paddockChampId !== last) return;
            target.innerHTML =
              '<div class="paddock-side">' + reviewCardHtml(story.review) + careerCardHtml(career) + '</div>' +
              '<div class="paddock-news">' + newsCardHtml(story.news) + '</div>';
          });
      });
    })
    .catch(function () {});
}

// A finished season in sentences, and the headlines it is remembered for.
function reviewCardHtml(view) {
  var title = '<div class="contract-title">Season review</div>';
  if (view.reason) {
    return '<div class="contract-box story-box">' + title +
      '<div class="contract-note">' + esc(view.reason) + '</div></div>';
  }
  return '<div class="contract-box story-box">' + title +
    '<ul class="review-lines">' + view.lines.map(function (l) { return '<li>' + esc(l) + '</li>'; }).join('') + '</ul>' +
    (view.headlines.length
      ? '<div class="news-round-title">Remembered for</div><ul class="news-list">' +
          view.headlines.map(function (h) {
            return '<li class="news-item' + (h.player ? ' news-item-player' : '') + '">Round ' + h.round + ': ' + esc(h.text) + '</li>';
          }).join('') + '</ul>'
      : '') +
    '</div>';
}

// The player's career: a line per season raced, then the firsts.
function careerCardHtml(view) {
  var title = '<div class="contract-title">Your career</div>';
  if (view.reason) {
    return '<div class="contract-box story-box">' + title +
      '<div class="contract-note">' + esc(view.reason) + '</div></div>';
  }
  return '<div class="contract-box story-box">' + title +
    '<table class="contract-table story-table"><tbody>' +
      view.seasons.map(function (s) {
        return '<tr class="' + (s.champion ? 'story-row-player' : '') + '">' +
          '<td class="contract-team">' + esc(s.name) + '</td><td>' + esc(s.line) + '</td></tr>';
      }).join('') +
    '</tbody></table>' +
    (view.milestones.length
      ? '<ul class="news-list career-milestones">' +
          view.milestones.map(function (m) { return '<li class="news-item">' + esc(m) + '</li>'; }).join('') + '</ul>'
      : '') +
    '</div>';
}

// The career card under the season's cards, fetched on its own: it reads every season, and the
// season's story should not wait for it.
function loadPaddockCareer() {
  fetch('/api/career/story').then(function (r) { return r.json(); })
    .then(function (career) {
      var el = document.getElementById('paddock-career');
      if (el) el.innerHTML = careerCardHtml(career);
    })
    .catch(function () {});
}

// A finished season's review on the Championships sub-tab, under its heading.
function loadChampReview(champId) {
  var el = document.getElementById('champ-review');
  if (!el) return;
  fetch('/api/championships/' + encodeURIComponent(champId) + '/story')
    .then(function (r) { return r.ok ? r.json() : null; })
    .then(function (story) {
      var target = document.getElementById('champ-review');
      if (!story || !target || target.getAttribute('data-champ') !== champId) return;
      target.innerHTML = reviewCardHtml(story.review);
    })
    .catch(function () {});
}

function loadPaddockStory(champId) {
  var el = document.getElementById('paddock-story');
  if (!el) return;
  el.innerHTML = '';
  fetch('/api/championships/' + encodeURIComponent(champId) + '/story')
    .then(function (r) { return r.ok ? r.json() : null; })
    .then(function (story) {
      // The career may have been switched while this was in flight.
      if (!story || champId !== paddockChampId) return;
      var target = document.getElementById('paddock-story');
      if (!target) return;
      // Two columns: where the season stands on the left, the news — the long part — on the
      // right with its own scroll. A multiplayer career has no teams, so no teammate card.
      target.innerHTML =
        '<div class="paddock-side">' +
          titleCardHtml(story.title) +
          (careerMode === 'multiplayer' ? '' : moodCardHtml(story.mood)) +
          (careerMode === 'multiplayer' ? '' : targetsCardHtml(story.targets)) +
          (careerMode === 'multiplayer' ? '' : interestCardHtml(story.interest)) +
          rivalCardHtml(story.rival) +
          (careerMode === 'multiplayer' ? '' : teammateCardHtml(story.teammates)) +
          '<div id="paddock-career"></div>' +
        '</div>' +
        '<div class="paddock-news">' + newsCardHtml(story.news) + '</div>';
      loadPaddockCareer();
    })
    .catch(function () {});
}

// The rival: who, since when, and how the two of you stand.
function rivalCardHtml(view) {
  var r = view.rival;
  var title = '<div class="contract-title">Rival' + (r ? ' — ' + esc(r.name) + (r.team ? ' (' + esc(r.team) + ')' : '') : '') + '</div>';
  if (!r) {
    return '<div class="contract-box story-box">' + title +
      '<div class="contract-note">' + esc(view.reason || '') + '</div></div>';
  }
  var ahead = view.points_ahead;
  return '<div class="contract-box story-box">' + title +
    '<div class="story-summary">' + esc(view.summary) + '</div>' +
    '<table class="contract-table story-table">' +
      '<thead><tr><th>Races you both started</th><th>Finished ahead</th><th>Points gap</th></tr></thead>' +
      '<tbody><tr>' +
        '<td>' + r.duel.races + '</td>' +
        '<td>' + storyScore({ player: r.duel.player, teammate: r.duel.rival }) + '</td>' +
        '<td>' + (ahead > 0 ? '<strong class="story-lead">+' + ahead + '</strong>' : ahead < 0 ? '−' + (-ahead) : '0') + '</td>' +
      '</tr></tbody>' +
    '</table>' +
    '<div class="contract-note">Finished ahead: you first, then your rival, over every race this season you both started. A finish beats a retirement; races you both retired from count for neither. The rival is whoever you keep finishing closest to, on track and in the standings.</div>' +
    '</div>';
}

var TITLE_STATUS_LABELS ={ clinched: 'Champion', in_contention: 'In the fight', eliminated: 'Out' };

// The title fight: where the player stands, what would settle it, the contract's target, and the
// drivers who can still win. Those who cannot are counted rather than listed.
function titleCardHtml(view) {
  var title = '<div class="contract-title">Title fight</div>';
  var lines = [view.summary, view.clinch, view.past_plan, view.objective, view.reason]
    .filter(Boolean)
    .map(function (s, i) {
      return '<div class="' + (i === 0 && view.summary ? 'story-summary' : 'contract-note story-line') + '">' + esc(s) + '</div>';
    }).join('');
  var f = view.fight;
  if (!f || !f.table.length) return '<div class="contract-box story-box">' + title + lines + '</div>';
  var shown = f.table.filter(function (c) { return c.status !== 'eliminated' || c.player; });
  var out = f.table.length - shown.length;
  var rows = shown.map(function (c) {
    return '<tr class="' + (c.player ? 'story-row-player' : '') + '">' +
      '<td>' + c.position + '</td>' +
      '<td class="contract-team">' + esc(c.name) + '</td>' +
      '<td>' + c.points + '</td>' +
      '<td>' + (c.behind ? '−' + c.behind : '—') + '</td>' +
      '<td><span class="title-status title-status-' + c.status + '">' + TITLE_STATUS_LABELS[c.status] + '</span></td>' +
      '</tr>';
  }).join('');
  return '<div class="contract-box story-box">' + title + lines +
    '<table class="contract-table story-table">' +
      '<thead><tr><th>Pos</th><th>Driver</th><th>Points</th><th>Behind</th><th></th></tr></thead>' +
      '<tbody>' + rows + '</tbody>' +
    '</table>' +
    '<div class="contract-note">' + f.points_left + ' points still to race for in ' + f.races_left + ' race' + (f.races_left === 1 ? '' : 's') +
      (out ? '. ' + out + ' driver' + (out === 1 ? '' : 's') + ' can no longer win it.' : '.') + '</div>' +
    '</div>';
}

// The Live tab's lines about the season being raced: where the player stands in the title fight,
// and what the team wants from the coming race. Asked for when the tab opens, like the roster
// check: both change only when a session is recorded or reassigned.
function loadLiveTitle() {
  var line = document.getElementById('live-title-line');
  var target = document.getElementById('live-target-line');
  if (!line) return;
  fetch('/api/championships').then(function (r) { return r.json(); })
    .then(function (champs) {
      var active = (champs || []).find(function (c) { return c.status === 'Active'; });
      if (!active) {
        line.hidden = true;
        if (target) target.hidden = true;
        return null;
      }
      return fetch('/api/championships/' + encodeURIComponent(active.id) + '/story')
        .then(function (r) { return r.ok ? r.json() : null; })
        .then(function (story) {
          var summary = story && story.title.summary;
          line.textContent = summary ? active.name + ': ' + summary : '';
          line.hidden = !summary;
          var next = story && story.targets.next;
          if (target) {
            target.textContent = next || '';
            target.hidden = !next;
          }
        });
    })
    .catch(function () {});
}

// How the team sees the player: the band, what it says, why it last moved, and a bar per race.
function moodCardHtml(view) {
  var title = '<div class="contract-title">Team mood' + (view.label ? ' — ' + esc(view.label) : '') + '</div>';
  if (view.reason) {
    return '<div class="contract-box story-box">' + title +
      '<div class="contract-note">' + esc(view.reason) + '</div></div>';
  }
  var bars = view.trend.map(function (p) {
    return '<span class="mood-bar mood-' + p.band + '" style="height:' + p.mood + '%" title="Round ' + p.round + ': ' + p.mood + '"></span>';
  }).join('');
  return '<div class="contract-box story-box">' + title +
    '<div class="mood-head">' +
      '<span class="mood-figure mood-text-' + view.band + '">' + view.mood + '</span>' +
      '<div><div class="story-summary">' + esc(view.says) + '</div>' +
      '<div class="contract-note story-line">' + esc(view.why) + '</div></div>' +
    '</div>' +
    '<div class="mood-trend" title="The mood after each race">' + bars + '</div>' +
    (view.warning ? '<div class="grid-box grid-box-warn">' + esc(view.warning) + '</div>' : '') +
    '</div>';
}

// Teams on this grid that would talk to the player today — interest, not offers.
function interestCardHtml(view) {
  var title = '<div class="contract-title">Interest from other teams</div>';
  var rows = view.teams.map(function (t) {
    return '<tr>' +
      '<td>' + t.pace + '</td>' +
      '<td class="contract-team">' + esc(t.team) + '</td>' +
      '<td>' + (t.quicker ? '<strong class="story-lead">a step up</strong>' : '') + '</td>' +
      '</tr>';
  }).join('');
  return '<div class="contract-box story-box">' + title +
    (view.reason ? '<div class="contract-note">' + esc(view.reason) + '</div>' : '') +
    (rows
      ? '<table class="contract-table story-table">' +
          '<thead><tr><th>Pace</th><th>Team</th><th></th></tr></thead>' +
          '<tbody>' + rows + '</tbody></table>'
      : '') +
    (view.note ? '<div class="contract-note story-line">' + esc(view.note) + '</div>' : '') +
    '</div>';
}

var VERDICT_MARKS = { met: '✓', missed: '✗', void: '–' };

// The team's weekend targets: the next one first, then how the raced rounds went.
function targetsCardHtml(view) {
  var title = '<div class="contract-title">Weekend targets</div>';
  if (view.reason) {
    return '<div class="contract-box story-box">' + title +
      '<div class="contract-note">' + esc(view.reason) + '</div></div>';
  }
  var rows = view.judged.map(function (j) {
    return '<tr>' +
      '<td>' + j.round + '</td>' +
      '<td>' + esc(j.track) + '</td>' +
      '<td>' + esc(j.target) + '</td>' +
      '<td><span class="target-verdict target-verdict-' + j.verdict + '" title="' + j.verdict + '">' +
        VERDICT_MARKS[j.verdict] + '</span></td>' +
      '</tr>';
  }).join('');
  return '<div class="contract-box story-box">' + title +
    (view.next ? '<div class="story-summary">' + esc(view.next) + '</div>' : '') +
    (view.record ? '<div class="contract-note story-line">' + esc(view.record) + '</div>' : '') +
    (rows
      ? '<div class="target-history"><table class="contract-table story-table">' +
          '<thead><tr><th>Round</th><th>Track</th><th>Target</th><th></th></tr></thead>' +
          '<tbody>' + rows + '</tbody></table></div>'
      : '') +
    '</div>';
}

// The news feed, newest round first. Headlines about the player stand out from the field's.
function newsCardHtml(news) {
  var title = '<div class="contract-title">Paddock news</div>';
  if (news.reason) {
    return '<div class="contract-box story-box">' + title +
      '<div class="contract-note">' + esc(news.reason) + '</div></div>';
  }
  var rounds = news.rounds.map(function (r) {
    return '<div class="news-round">' +
      '<div class="news-round-title">Round ' + r.round + (r.track ? ' — ' + esc(r.track) : '') + '</div>' +
      '<ul class="news-list">' +
        r.headlines.map(function (h) {
          return '<li class="news-item' + (h.player ? ' news-item-player' : '') + '">' + esc(h.text) + '</li>';
        }).join('') +
      '</ul></div>';
  }).join('');
  return '<div class="contract-box story-box">' + title + rounds + '</div>';
}

document.querySelectorAll('[data-career-sub="paddock"]').forEach(function (btn) {
  btn.addEventListener('click', loadPaddock);
});

// The Live tab is the one the page opens on, so its title line is asked for straight away.
loadLiveTitle();
