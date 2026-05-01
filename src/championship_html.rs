// ── HTML generation ──────────────────────────────────────────────────────────

fn generate_html() -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>AMS2 Career Championships</title>
<style>{css}</style>
</head>
<body>
<header>
  <h1>AMS2 Career Championships</h1>
  <div class="tab-bar">
    <button class="tab-btn tab-active" data-tab="live">&#9679; Live Session</button>
    <button class="tab-btn" data-tab="career">&#127942; Career</button>
    <button class="tab-btn" data-tab="manage">&#9881; Manage</button>
    <button class="tab-btn" data-tab="config">&#9965; Config</button>
  </div>
</header>
<main>
  <div id="tab-career" class="tab-panel tab-panel-hidden">
    <div class="sub-tab-bar">
      <button class="sub-tab-btn sub-tab-active" data-career-sub="champs">Championships</button>
      <button class="sub-tab-btn" data-career-sub="stats">Driver Stats</button>
      <button class="sub-tab-btn" data-career-sub="tracks">Track Stats</button>
      <button id="career-export-btn" class="career-export-btn">&#128229; Download HTML</button>
    </div>
    <div id="career-sub-champs" class="sub-tab-panel">
      <div class="champ-master-detail">
        <div id="career-champ-list" class="champ-list"></div>
        <div id="career-champ-list-resize" class="champ-list-resize"></div>
        <div id="career-champ-detail" class="champ-detail"></div>
      </div>
    </div>
    <div id="career-sub-stats" class="sub-tab-panel sub-tab-panel-hidden">
      <div id="career-stats-container"></div>
    </div>
    <div id="career-sub-tracks" class="sub-tab-panel sub-tab-panel-hidden">
      <div id="career-tracks-container"></div>
    </div>
  </div>
  <div id="tab-live" class="tab-panel">
    <section class="live-section">
      <div id="live-status" class="live-status live-disconnected">
        <span class="live-dot"></span>
        <span id="live-status-text">Not connected — start AMS2 and open this page via the server</span>
      </div>
      <div id="live-info" class="live-info">
        <span id="live-session-type"></span>
        <span id="live-race-state"></span>
        <span id="live-track" class="live-track"></span>
        <span id="live-raw-states" class="live-raw-states"></span>
        <button id="live-record-btn" class="live-record-btn" disabled title="Save the current session now">&#9210; Save Session</button>
        <span id="live-record-msg" class="live-record-msg"></span>
      </div>
      <nav class="live-subnav">
        <button class="live-subtab live-subtab-active" data-sub="timing">Timing</button>
        <button class="live-subtab" data-sub="setup">Telemetry</button>
        <button id="spotter-btn" class="spotter-btn" title="Enable voice spotter">&#128362; Spotter</button>
        <select id="spotter-voice" class="spotter-voice" title="TTS voice"></select>
        <select id="spotter-focus" class="spotter-focus" title="Spotter focus: which driver to track">
          <option value="">Auto (viewed player)</option>
        </select>
      </nav>
      <div id="live-sub-timing" class="live-subpanel">
        <div class="live-body">
          <canvas id="track-map" width="280" height="280" class="track-map"></canvas>
          <div class="grid-scroll">
            <table id="live-table" class="live-table">
              <thead>
                <tr>
                  <th data-col="0" data-type="num">Pos</th>
                  <th data-col="1" data-type="str">Driver</th>
                  <th data-col="2" data-type="num">Laps</th>
                  <th data-col="3" data-type="gap">Interval</th>
                  <th data-col="4" data-type="gap">Gap</th>
                  <th data-col="5" data-type="time">S1</th>
                  <th data-col="6" data-type="time">S2</th>
                  <th data-col="7" data-type="time">S3</th>
                  <th data-col="8" data-type="time">Best Lap</th>
                  <th data-col="9" data-type="time">Last Lap</th>
                  <th data-col="10" data-type="num">Top km/h</th>
                  <th data-col="11" data-type="str">Tyre</th>
                </tr>
              </thead>
              <tbody id="live-tbody">
                <tr><td colspan="12" class="live-empty">Waiting for session data&hellip;</td></tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
      <div id="live-sub-setup" class="live-subpanel live-subpanel-hidden">
        <div id="setup-panel" class="setup-panel">
          <div class="setup-no-data">Connect to AMS2 to see telemetry.</div>
        </div>
      </div>
    </section>
  </div>
  <div id="tab-manage" class="tab-panel tab-panel-hidden">
    <div class="manage-layout">
      <div id="manage-new-form" class="manage-new-form" style="display:none">
        <h3>New Championship</h3>
        <div class="manage-form-row">
          <input id="new-champ-name" type="text" placeholder="Championship name" class="manage-input" style="flex:1">
          <select id="new-champ-points" class="manage-select">
            <option value="25,18,15,12,10,8,6,4,2,1">F1 Modern (25-18-15&hellip;)</option>
            <option value="10,6,4,3,2,1">F1 1991-2002 (10-6-4-3-2-1)</option>
            <option value="9,6,4,3,2,1">F1 Classic (9-6-4-3-2-1)</option>
            <option value="custom">Custom&hellip;</option>
          </select>
          <input id="new-champ-custom" type="text" placeholder="e.g. 25,18,15,12,10" class="manage-input" style="display:none;flex:1">
          <label class="manage-checkbox-label"><input type="checkbox" id="new-champ-manufacturer"> Constructor Scoring</label>
          <button id="new-champ-save" class="manage-btn manage-btn-primary">Create</button>
          <button id="new-champ-cancel" class="manage-btn">Cancel</button>
        </div>
      </div>
      <div class="manage-columns">
        <div class="manage-left">
          <div class="manage-left-header">
            <span>Championships</span>
            <button id="add-champ-btn" class="manage-btn manage-btn-primary">+ New</button>
          </div>
          <div id="champ-list"></div>
        </div>
        <div class="manage-right" id="manage-right">
          <div class="manage-placeholder">Select a championship or create a new one.</div>
        </div>
      </div>
      <div class="manage-danger-zone">
        <button id="purge-sessions-btn" class="manage-btn manage-btn-danger">&#x1f5d1; Delete unassigned sessions</button>
      </div>
      <div id="manage-sessions-panel" class="manage-sessions-panel" style="display:none">
        <div class="manage-sessions-header">
          <span>Available Sessions</span>
          <button id="close-sessions-btn" class="manage-btn">&#x2715; Close</button>
        </div>
        <div id="available-sessions"></div>
      </div>
    </div>
  </div>
  <div id="tab-config" class="tab-panel tab-panel-hidden">
    <div class="config-panel">
      <h2 class="config-heading">Server Configuration</h2>
      <p class="config-note">Changes are saved to <code>config.json</code> next to the executable.
         Settings marked <span class="config-restart-badge">restart</span> take effect after restarting the server.</p>
      <form id="config-form" autocomplete="off">
        <div class="config-group">
          <label class="config-label" for="cfg-port">Port <span class="config-restart-badge">restart</span></label>
          <input class="config-input" id="cfg-port" name="port" type="number" min="1" max="65535" />
          <span class="config-hint">HTTP and WebSocket port (default 8080)</span>
        </div>
        <div class="config-group">
          <label class="config-label" for="cfg-host">Host <span class="config-restart-badge">restart</span></label>
          <input class="config-input" id="cfg-host" name="host" type="text" />
          <span class="config-hint">Bind address. Use <code>0.0.0.0</code> to allow LAN access (default 127.0.0.1)</span>
        </div>
        <div class="config-group">
          <label class="config-label" for="cfg-data-file">Data file <span class="config-restart-badge">restart</span></label>
          <input class="config-input config-input-wide" id="cfg-data-file" name="data_file" type="text" placeholder="(default: championships/ams2_career.json next to executable)" />
          <span class="config-hint">Full path to the career JSON file. Leave empty for the default location.</span>
        </div>
        <div class="config-group">
          <label class="config-label" for="cfg-poll-ms">Poll interval (ms)</label>
          <input class="config-input" id="cfg-poll-ms" name="poll_ms" type="number" min="50" max="5000" />
          <span class="config-hint">Shared memory read interval for the live view (default 200 ms)</span>
        </div>
        <div class="config-group">
          <label class="config-label">Auto-record <span class="config-restart-badge">restart</span></label>
          <div class="config-check-group">
            <label class="config-label-check"><input id="cfg-record-practice" type="checkbox" /> Practice</label>
            <label class="config-label-check"><input id="cfg-record-qualify"  type="checkbox" /> Qualifying</label>
            <label class="config-label-check"><input id="cfg-record-race"     type="checkbox" /> Race</label>
          </div>
          <span class="config-hint">Which session types to save automatically when AMS2 is running</span>
        </div>
        <div class="config-group config-group-check">
          <label class="config-label config-label-check">
            <input id="cfg-show-track-map" name="show_track_map" type="checkbox" />
            Show track radar in live view
          </label>
          <span class="config-hint">Displays the track map canvas next to the timing table</span>
        </div>
        <div class="config-group">
          <label class="config-label" for="cfg-track-map-max-points">Track radar max points</label>
          <input class="config-input" id="cfg-track-map-max-points" name="track_map_max_points" type="number" min="100" max="50000" />
          <span class="config-hint">Maximum number of unique grid cells accumulated before collection stops (default 5000)</span>
        </div>
        <div class="config-actions">
          <button class="manage-btn manage-btn-primary" type="submit">Save</button>
          <span id="config-save-msg" class="config-save-msg"></span>
        </div>
      </form>
    </div>
  </div>
</main>
<script>{js_utils}</script>
<script>{js_telemetry}</script>
<script>{js_track_map}</script>
<script>{js_live}</script>
<script>{js_career}</script>
<script>{js_manage}</script>
<script>{js_config}</script>
<script>{js_main}</script>
</body>
</html>"##,
        css          = CSS,
        js_utils     = JS_UTILS,
        js_telemetry = JS_TELEMETRY,
        js_track_map = JS_TRACK_MAP,
        js_live      = JS_LIVE,
        js_career    = JS_CAREER,
        js_manage    = JS_MANAGE,
        js_config    = JS_CONFIG,
        js_main      = JS_MAIN,
    )
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn build_base_html() -> String {
    generate_html()
}

// ── Styles ───────────────────────────────────────────────────────────────────

const CSS: &str = include_str!("assets/style.css");

// ── Scripts ──────────────────────────────────────────────────────────────────

const JS_UTILS:     &str = include_str!("assets/utils.js");
const JS_TELEMETRY: &str = include_str!("assets/telemetry.js");
const JS_TRACK_MAP: &str = include_str!("assets/track_map.js");
const JS_LIVE:      &str = include_str!("assets/live.js");
const JS_CAREER:    &str = include_str!("assets/career.js");
const JS_MANAGE:    &str = include_str!("assets/manage.js");
const JS_CONFIG:    &str = include_str!("assets/config.js");
const JS_MAIN:      &str = include_str!("assets/main.js");
