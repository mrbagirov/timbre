/* Settings state shared by the main window and the tray window.
   In Tauri, replace the localStorage calls with invoke("get_state") / invoke("set_param") and
   the "storage" event with an event emitted from Rust. */
(function () {
  'use strict';

  /* Presets are a base chain plus an optional voice character. Order matters: it is the order in the list. */
  function chain(o) {
    return {
      input: { gain: o.gain === undefined ? 12 : o.gain },
      gate: { on: true, threshold: o.gate === undefined ? -48 : o.gate },
      nr: { on: true, mode: o.nr || 'light', amount: o.nrAmt === undefined ? 60 : o.nrAmt },
      eq: { on: true, lowcut: true, low: o.low || 0, mid: o.mid || 0, high: o.high || 0 },
      ch: { on: true, pitch: o.pitch || 0, warmth: o.warmth || 0, thick: o.thick || 0, radio: o.radio || 0, robot: o.robot || 0 },
      comp: { on: true, threshold: o.comp === undefined ? -22 : o.comp, ratio: o.ratio || 3 },
      lim: { on: true, ceiling: o.ceil === undefined ? -1 : o.ceil }
    };
  }

  var PRESETS = {
    novice:    { group: 'tasks', name: 'новичок', params: chain({ low: 0, mid: 1, high: 2 }) },
    stream:    { group: 'tasks', name: 'стрим', params: chain({ gate: -45, nr: 'hq', nrAmt: 75, low: 2, high: 3, comp: -20, ratio: 4 }) },
    voiceover: { group: 'tasks', name: 'закадр', params: chain({ gate: -52, nrAmt: 40, low: 1, high: 1, comp: -24, ratio: 2.5, ceil: -2, gain: 10 }) },
    call:      { group: 'tasks', name: 'созвон', params: chain({ gate: -42, nr: 'hq', nrAmt: 90, low: -1, mid: 2, high: 1, ratio: 3, comp: -18, ceil: -2 }) },
    warm:      { group: 'voices', name: 'тёплый эфир', params: chain({ low: 3, mid: 1, high: 1, warmth: 60, thick: 15, comp: -22, ratio: 3.5 }) },
    deep:      { group: 'voices', name: 'глубокий', params: chain({ pitch: -3, low: 4, high: -1, warmth: 25, comp: -22, ratio: 3.5 }) },
    bright:    { group: 'voices', name: 'яркий подкаст', params: chain({ low: -1, mid: 1, high: 4, warmth: 15, comp: -20, ratio: 3 }) },
    thick:     { group: 'voices', name: 'объёмный', params: chain({ low: 2, thick: 70, warmth: 30, comp: -22, ratio: 3 }) },
    radio:     { group: 'voices', name: 'радио', params: chain({ radio: 100, warmth: 40, comp: -24, ratio: 4 }) },
    robot:     { group: 'voices', name: 'робот', params: chain({ robot: 80, pitch: -1, comp: -22, ratio: 3 }) },
    chipmunk:  { group: 'voices', name: 'высокий', params: chain({ pitch: 5, high: 1, comp: -22, ratio: 3 }) }
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

  function ensureCustom() { if (state.custom) PRESETS.mine = { group: 'tasks', name: 'мой', params: clone(state.custom) }; else if (state.preset === 'mine') state.preset = 'novice'; }

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
  function saveCustom(p) { state.custom = clone(p); PRESETS.mine = { group: 'tasks', name: 'мой', params: clone(p) }; state.preset = 'mine'; state.params = clone(p); save(); emit('local'); }
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
