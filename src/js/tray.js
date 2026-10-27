/* Tray window: presets, mute, bypass. Shares state with the main window through Store. */
(function () {
  'use strict';

  var $ = function (s, r) { return (r || document).querySelector(s); };
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };
  var clamp = function (v, a, b) { return Math.max(a, Math.min(b, v)); };
  var reduced = window.matchMedia('(prefers-reduced-motion: reduce)');

  function render() {
    var s = Store.state;
    $('#tray-mine').hidden = !Store.PRESETS.mine;
    $$('[data-preset]').forEach(function (b) { b.setAttribute('aria-checked', String(b.dataset.preset === s.preset)); });
    $$('[data-flag]').forEach(function (b) { b.setAttribute('aria-pressed', String(!!s[b.dataset.flag])); });
    $('#tray-status').textContent = s.muted ? 'микрофон выключен' : s.bypass ? 'обработка отключена' : 'работает, пресет «' + Store.PRESETS[s.preset].name + '»';
    $('#tray-hotkey').textContent = s.settings.hotkey;
  }

  $$('[data-preset]').forEach(function (b) { b.addEventListener('click', function () { Store.applyPreset(b.dataset.preset); }); });
  $$('[data-flag]').forEach(function (b) { b.addEventListener('click', function () { Store.setFlag(b.dataset.flag, !Store.state[b.dataset.flag]); }); });
  $('#tray-open').addEventListener('click', function () { location.href = './index.html'; });
  $('#tray-quit').addEventListener('click', function () { window.close(); });
  Store.subscribe(render);
  render();

  var el = $('[data-meter="out"]'), fill = $('[data-fill]', el), peak = $('[data-peak]', el), track = $('[role="meter"]', el);
  var pk = -90, hold = 0, last = 0, textAt = 0;
  function frame(ts) {
    requestAnimationFrame(frame);
    if (!last) last = ts;
    var dt = Math.min((ts - last) / 1000, 0.25); last = ts;
    if (reduced.matches && ts - textAt < 250) return;
    var s = Store.state;
    Sim.advance(dt, s.params, s);
    var db = Sim.now.outDb;
    fill.style.setProperty('--v', clamp((db + 60) / 60, 0, 1).toFixed(3));
    if (db >= pk) { pk = db; hold = 1.2; } else { hold -= dt; if (hold < 0) pk -= 20 * dt; }
    peak.style.setProperty('--pk', clamp((pk + 60) / 60, 0, 1).toFixed(3));
    if (ts - textAt > 100) { textAt = ts; track.setAttribute('aria-valuenow', String(Math.round(clamp(db, -60, 0)))); }
  }
  for (var w = 0; w < 8; w++) Sim.advance(0.25, Store.state.params, Store.state);
  requestAnimationFrame(frame);
})();
