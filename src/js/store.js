/* Settings state shared by the main window and the tray window.
   In Tauri, replace the localStorage calls with invoke("get_state") / invoke("set_param") and
   the "storage" event with an event emitted from Rust. */
(function () {
  'use strict';

  var PRESETS = {
    novice: { name: 'новичок', params: {
      gate: { on: true, threshold: -50 },
      nr:   { on: true, mode: 'light', amount: 60 },
      eq:   { on: true, lowcut: true, low: 0, mid: 1, high: 2 },
      comp: { on: true, threshold: -22, ratio: 3 },
      lim:  { on: true, ceiling: -1 }
    } },
    stream: { name: 'стрим', params: {
      gate: { on: true, threshold: -45 },
      nr:   { on: true, mode: 'hq', amount: 75 },
      eq:   { on: true, lowcut: true, low: 2, mid: 0, high: 3 },
      comp: { on: true, threshold: -20, ratio: 4 },
      lim:  { on: true, ceiling: -1 }
    } },
    voiceover: { name: 'закадр', params: {
      gate: { on: true, threshold: -55 },
      nr:   { on: true, mode: 'light', amount: 40 },
      eq:   { on: true, lowcut: true, low: 1, mid: 0, high: 1 },
      comp: { on: true, threshold: -24, ratio: 2.5 },
      lim:  { on: true, ceiling: -2 }
    } },
    call: { name: 'созвон', params: {
      gate: { on: true, threshold: -40 },
      nr:   { on: true, mode: 'hq', amount: 90 },
      eq:   { on: true, lowcut: true, low: -1, mid: 2, high: 1 },
      comp: { on: true, threshold: -18, ratio: 3 },
      lim:  { on: true, ceiling: -2 }
    } }
  };

  var KEY = 'timbre.v1';
  var subs = [];

  function clone(o) { return JSON.parse(JSON.stringify(o)); }

  var state = { preset: 'novice', params: clone(PRESETS.novice.params), muted: false, bypass: false, settings: { startup: false, hotkey: 'Ctrl+Alt+M' } };

  function merge(target, src) {
    Object.keys(src || {}).forEach(function (k) {
      if (src[k] && typeof src[k] === 'object' && !Array.isArray(src[k])) { target[k] = target[k] || {}; merge(target[k], src[k]); }
      else target[k] = src[k];
    });
    return target;
  }

  function ensureCustom() { if (state.custom) PRESETS.mine = { name: 'мой', params: clone(state.custom) }; else if (state.preset === 'mine') state.preset = 'novice'; }

  try { var saved = JSON.parse(localStorage.getItem(KEY) || 'null'); if (saved) merge(state, saved); } catch (e) { /* storage may be unavailable */ }

  ensureCustom();
  function save() { try { localStorage.setItem(KEY, JSON.stringify(state)); } catch (e) { /* ignore */ } }
  function emit(source) { subs.forEach(function (f) { f(state, source); }); }

  function get(path) { return path.split('.').reduce(function (o, k) { return o[k]; }, state.params); }

  function set(path, value) {
    var keys = path.split('.'), last = keys.pop();
    var obj = keys.reduce(function (o, k) { return o[k]; }, state.params);
    if (obj[last] === value) return;
    obj[last] = value; save(); emit('local');
  }

  function replaceParams(params) { state.params = clone(params); save(); emit('local'); }
  function applyPreset(id) { if (!PRESETS[id]) return; state.preset = id; state.params = clone(PRESETS[id].params); save(); emit('local'); }
  function saveCustom(p) { state.custom = clone(p); PRESETS.mine = { name: 'мой', params: clone(p) }; state.preset = 'mine'; state.params = clone(p); save(); emit('local'); }
  function setFlag(name, value) { state[name] = value; save(); emit('local'); }
  function setSetting(name, value) { state.settings[name] = value; save(); emit('local'); }
  function isDirty() { return JSON.stringify(state.params) !== JSON.stringify(PRESETS[state.preset].params); }
  function subscribe(f) { subs.push(f); }

  window.addEventListener('storage', function (e) {
    if (e.key !== KEY || !e.newValue) return;
    try { merge(state, JSON.parse(e.newValue)); ensureCustom(); emit('remote'); } catch (err) { /* ignore */ }
  });

  window.Store = { PRESETS: PRESETS, state: state, get: get, set: set, replaceParams: replaceParams, applyPreset: applyPreset, setFlag: setFlag, saveCustom: saveCustom, setSetting: setSetting, isDirty: isDirty, subscribe: subscribe, clone: clone };
})();
