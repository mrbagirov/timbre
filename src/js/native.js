/* Inside Tauri this file replaces the demo signal with the real engine and connects settings to the Rust side.
   In a plain browser window.__TAURI__ is missing and the file does nothing, so the preview keeps working. */
(function () {
  'use strict';
  var T = window.__TAURI__;
  if (!T || !T.core || !T.event) return;
  var invoke = T.core.invoke, listen = T.event.listen;
  document.documentElement.classList.add('native');

  /* ---------- the signal: same surface as sim.js, fed by the engine at 50 Hz ---------- */
  var N = Sim.N, FLOOR = Sim.FLOOR, STAGES = ['raw', 'gate', 'nr', 'ch', 'comp', 'lim'];
  var hist = {}, pos = 0, jit = new Float32Array(N), i;
  STAGES.forEach(function (s) { hist[s] = new Float32Array(N).fill(FLOOR); });
  for (i = 0; i < N; i++) jit[i] = 0.74 + 0.26 * Math.random();

  Sim.advance = function () {};
  Sim.setMode = function () {};
  Sim.setGain = function () {};
  Sim.at = function (stage, k) { return hist[stage][(pos + k) % N]; };
  Sim.jitter = function (k) { return jit[(pos + k) % N]; };

  /* Spectrum balance of speech. The targets are a first guess and have to be tuned on real voices;
     CAUTION halves the correction until they are. */
  var TARGET = { low: -8, high: -18 }, CAUTION = 0.5;
  var ema = { low: 0, high: 0, ready: false };
  Sim.bands = function () {
    return ema.ready ? { low: Math.round(ema.low * 10) / 10, mid: 0, high: Math.round(ema.high * 10) / 10 } : { low: 0, mid: 0, high: 0 };
  };

  listen('engine-frame', function (e) {
    var f = e.payload, n = Sim.now;
    hist.raw[pos] = f.raw; hist.gate[pos] = f.gate; hist.nr[pos] = f.nr; hist.ch[pos] = f.ch; hist.comp[pos] = f.comp; hist.lim[pos] = f.lim;
    jit[pos] = 0.74 + 0.26 * Math.random();
    pos = (pos + 1) % N;
    n.inDb = f.pre; /* the meter and the wizard look at the microphone itself, before the software gain */ n.outDb = f.out; n.gateOpen = f.gateOpen; n.nrDb = f.nrDb; n.grDb = f.grDb; n.limDb = f.limDb; n.loud = Math.max(-70, f.loud);
    if (f.pre > -40 && f.bands) {
      var low = (f.bands[0] - f.bands[1] - TARGET.low) * CAUTION, high = (f.bands[2] - f.bands[1] - TARGET.high) * CAUTION;
      if (!ema.ready) { ema.low = low; ema.high = high; ema.ready = true; } else { ema.low += (low - ema.low) * 0.02; ema.high += (high - ema.high) * 0.02; }
    }
  });

  /* ---------- devices ---------- */
  Bridge.mock = false;
  Bridge.detect = function () { return invoke('detect_devices'); };
  Bridge.openUrl = function (url) { invoke('open_url', { url: url }).catch(function () {}); };

  var DEV_KEY = 'timbre.devices';
  var choice = { input: '', output: '' };
  try { var saved = JSON.parse(localStorage.getItem(DEV_KEY) || 'null'); if (saved) choice = saved; } catch (err) { /* ignore */ }

  var engineMsg = document.getElementById('engine-msg');
  /* the Rust side sends its message in Russian, the dictionary holds the same sentences */
  listen('engine-status', function (e) {
    var s = e.payload, dev = document.getElementById('device');
    var text = I18N.t(s.message || '');
    if (engineMsg) engineMsg.textContent = text;
    if (s.state === 'error' && dev) { dev.dataset.state = 'none'; dev.textContent = I18N.t('ошибка звука'); dev.title = text; }
  });

  function fill(sel, names, current) {
    if (!sel) return;
    sel.innerHTML = '';
    var auto = document.createElement('option'); auto.value = ''; auto.textContent = I18N.t('автоматически'); sel.appendChild(auto);
    names.forEach(function (n) { var o = document.createElement('option'); o.value = n; o.textContent = n; sel.appendChild(o); });
    sel.value = names.indexOf(current) >= 0 ? current : '';
  }
  function populate() {
    Bridge.detect().then(function (r) {
      fill(document.getElementById('set-mic'), r.inputs, choice.input);
      fill(document.getElementById('set-out'), r.outputs, choice.output);
    }).catch(function () {});
  }
  function applyChoice() {
    try { localStorage.setItem(DEV_KEY, JSON.stringify(choice)); } catch (err) { /* ignore */ }
    return invoke('restart_engine', { input: choice.input, output: choice.output });
  }
  document.getElementById('set-mic').addEventListener('change', function (e) { choice.input = e.target.value; applyChoice(); });
  document.getElementById('set-out').addEventListener('change', function (e) { choice.output = e.target.value; applyChoice(); });
  document.getElementById('btn-settings').addEventListener('click', populate);
  /* "check again" on the first-run screen re-opens the streams too */
  ['btn-recheck-mic', 'btn-recheck-cable'].forEach(function (id) { var b = document.getElementById(id); if (b) b.addEventListener('click', function () { invoke('restart_engine', { input: choice.input, output: choice.output }); }); });

  /* ---------- diagnostics for support ---------- */
  var diagBtn = document.getElementById('btn-diag'), diagHelp = document.getElementById('diag-help');
  if (diagBtn) diagBtn.addEventListener('click', function () {
    diagHelp.textContent = I18N.t('Собираю отчёт…');
    invoke('diagnostics').then(function (text) {
      var done = function () { diagHelp.textContent = I18N.t('Отчёт скопирован, вставьте его в чат.'); };
      if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(text).then(done, function () { diagHelp.textContent = text.slice(0, 600); });
      else diagHelp.textContent = text.slice(0, 600);
    }).catch(function (e) { diagHelp.textContent = String(e); });
  });

  /* ---------- state to Rust, events from the tray and the global hotkey ---------- */
  var lastHotkey = '', lastStartup = null;
  function push(s) {
    invoke('set_state', { state: { params: s.params, muted: s.muted, bypass: s.bypass } }).catch(function () {});
    var accel = (s.settings.hotkey || '').replace(/\bWin\b/g, 'Super');
    if (accel && accel !== lastHotkey) { lastHotkey = accel; invoke('set_hotkey', { accel: accel }).catch(function () {}); }
    if (lastStartup !== !!s.settings.startup) {
      var first = lastStartup === null; lastStartup = !!s.settings.startup;
      if (!first || lastStartup) invoke('set_autostart', { enabled: lastStartup }).catch(function () {});
    }
  }
  Store.subscribe(function (s) { push(s); });
  push(Store.state);
  invoke('restart_engine', { input: choice.input, output: choice.output }).catch(function () {});

  invoke('set_lang', { lang: I18N.lang }).catch(function () {});
  listen('hotkey-mute', function () { Store.setFlag('muted', !Store.state.muted); });
  listen('tray-flag', function (e) { var k = e.payload; if (k === 'muted' || k === 'bypass') Store.setFlag(k, !Store.state[k]); });
  listen('tray-preset', function (e) { if (Store.PRESETS[e.payload]) Store.applyPreset(e.payload); });
})();
