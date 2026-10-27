/* Main window: controls, lanes, meters, auto-setup, first run, settings. */
(function () {
  'use strict';

  var $ = function (s, r) { return (r || document).querySelector(s); };
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };
  var clamp = function (v, a, b) { return Math.max(a, Math.min(b, v)); };
  var reduced = window.matchMedia('(prefers-reduced-motion: reduce)');
  var tr = I18N.t, DEC = I18N.dec;

  /* ---------- text ---------- */
  var MINUS = '−';
  function num(v, d) { var s = Math.abs(v).toFixed(d || 0).replace('.', DEC); return (v < 0 && Number(s.replace(DEC, '.')) !== 0 ? MINUS : '') + s; }
  function sgn(v) { return v > 0 ? '+' + v : v < 0 ? MINUS + Math.abs(v) : '0'; }
  var fmt = {
    'gate.threshold': function (v) { return num(v) + ' дБ'; },
    'nr.amount': function (v) { return v + '%'; },
    'eq.low': function (v) { return sgn(v) + ' дБ'; },
    'eq.mid': function (v) { return sgn(v) + ' дБ'; },
    'eq.high': function (v) { return sgn(v) + ' дБ'; },
    'comp.threshold': function (v) { return num(v) + ' дБ'; },
    'comp.ratio': function (v) { return String(v).replace('.', DEC) + ' к 1'; },
    'lim.ceiling': function (v) { return num(v, 1) + ' дБ'; }
  };

  /* ---------- controls ---------- */
  function render() {
    var s = Store.state;
    var mine = !!Store.PRESETS.mine; $('#pr-mine').hidden = !mine; $('label[for="pr-mine"]').hidden = !mine;
    $$('#presets input').forEach(function (i) { i.checked = i.value === s.preset; });
    $$('[data-param]').forEach(function (el) {
      var v = Store.get(el.dataset.param);
      if (el.type === 'range') {
        el.value = v;
        el.style.setProperty('--fill', ((v - el.min) / (el.max - el.min)) * 100 + '%');
      } else if (el.type === 'radio') el.checked = el.value === v;
    });
    $$('[data-out]').forEach(function (o) { var p = o.dataset.out; o.textContent = tr((fmt[p] || String)(Store.get(p))); });
    $$('[data-toggle]').forEach(function (b) {
      var on = Store.get(b.dataset.toggle);
      b.setAttribute('aria-checked', String(on)); b.textContent = tr(on ? 'вкл' : 'выкл');
      var lane = b.closest('.lane'); if (lane) lane.dataset.on = String(on);
    });
    $$('[data-chip]').forEach(function (b) { b.setAttribute('aria-pressed', String(Store.get(b.dataset.chip))); });

    var mute = $('#btn-mute'); mute.setAttribute('aria-pressed', String(s.muted)); mute.textContent = tr(s.muted ? 'микрофон выключен' : 'микрофон включён');
    $('#btn-bypass').setAttribute('aria-pressed', String(s.bypass));

    var dirty = Store.isDirty();
    $('#dirty-text').textContent = dirty ? tr('настройки отличаются от пресета «' + Store.PRESETS[s.preset].name + '».') : '';
    $('#btn-reset').hidden = !dirty;

    var st = $('#set-startup'); st.setAttribute('aria-checked', String(!!s.settings.startup)); st.textContent = tr(s.settings.startup ? 'вкл' : 'выкл');
    if (!capturing) $('#set-hotkey').value = s.settings.hotkey;
  }

  $$('[data-param]').forEach(function (el) {
    var path = el.dataset.param;
    if (el.type === 'range') el.addEventListener('input', function () { Store.set(path, parseFloat(el.value)); });
    else if (el.type === 'radio') el.addEventListener('change', function () { if (el.checked) Store.set(path, el.value); });
  });
  $$('[data-toggle]').forEach(function (b) { b.addEventListener('click', function () { Store.set(b.dataset.toggle, !Store.get(b.dataset.toggle)); }); });
  $$('[data-chip]').forEach(function (b) { b.addEventListener('click', function () { Store.set(b.dataset.chip, !Store.get(b.dataset.chip)); }); });
  $$('#presets input').forEach(function (i) { i.addEventListener('change', function () { if (i.checked) Store.applyPreset(i.value); }); });
  $('#btn-reset').addEventListener('click', function () { Store.applyPreset(Store.state.preset); });
  $('#btn-mute').addEventListener('click', function () { Store.setFlag('muted', !Store.state.muted); });
  $('#btn-bypass').addEventListener('click', function () { Store.setFlag('bypass', !Store.state.bypass); });
  Store.subscribe(function () { render(); });

  /* ---------- lanes ---------- */
  var dpr = Math.min(window.devicePixelRatio || 1, 2);
  var col = {};
  function readColors() {
    var cs = getComputedStyle(document.documentElement);
    ['accent', 'accent-soft', 'rule', 'rule-2', 'ink', 'muted'].forEach(function (k) { col[k] = cs.getPropertyValue('--color-' + k).trim(); });
  }
  readColors();

  var lanes = $$('canvas[data-viz]').map(function (el) {
    var c = { el: el, name: el.dataset.viz, ctx: el.getContext('2d'), w: 0, h: 0 };
    function size() {
      var r = el.getBoundingClientRect();
      c.w = Math.max(1, Math.round(r.width)); c.h = Math.max(1, Math.round(r.height));
      el.width = c.w * dpr; el.height = c.h * dpr; c.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    size();
    if ('ResizeObserver' in window) new ResizeObserver(size).observe(el.parentElement); else window.addEventListener('resize', size);
    return c;
  });

  function u(d) { return clamp((d + 66) / 66, 0, 1); }

  function drawWave(c, stage, thr) {
    var ctx = c.ctx, W = c.w, H = c.h, mid = H / 2, amp = H * 0.44, n = Sim.N, step = W / n, bw = Math.max(1, step + 0.4);
    ctx.clearRect(0, 0, W, H);
    ctx.fillStyle = col.rule; ctx.fillRect(0, mid - 0.5, W, 1);
    var i, h;
    ctx.fillStyle = col['accent-soft'];
    for (i = 0; i < n; i++) { h = Math.max(0.6, u(Sim.at('raw', i)) * amp * Sim.jitter(i)); ctx.fillRect(i * step, mid - h, bw, h * 2); }
    ctx.fillStyle = col.accent;
    for (i = 0; i < n; i++) { h = Math.max(0.5, u(Sim.at(stage, i)) * amp * Sim.jitter(i)); ctx.fillRect(i * step, mid - h, bw, h * 2); }
    if (thr !== null && thr !== undefined) {
      var y = u(thr) * amp;
      ctx.fillStyle = col.muted; ctx.fillRect(0, mid - y - 0.5, W, 1); ctx.fillRect(0, mid + y - 0.5, W, 1);
    }
  }

  function eqDb(f, p) {
    if (!p.on) return 0;
    var l = Math.log2(f);
    var low = p.low / (1 + Math.exp((l - Math.log2(160)) * 2.2));
    var mid = p.mid * Math.exp(-Math.pow(l - Math.log2(1200), 2) / (2 * 0.8 * 0.8));
    var high = p.high / (1 + Math.exp(-(l - Math.log2(5500)) * 2.2));
    var hp = p.lowcut ? -10 * Math.log10(1 + Math.pow(80 / f, 4)) : 0;
    return low + mid + high + hp;
  }
  function voiceShape(f) {
    var l = Math.log2(f), tilt = clamp(1 - Math.max(0, l - Math.log2(180)) * 0.2, 0.05, 1);
    var b = function (c, w, a) { return a * Math.exp(-Math.pow(l - Math.log2(c), 2) / (2 * w * w)); };
    return clamp(0.25 * tilt + b(500, 0.45, 0.55) + b(1500, 0.4, 0.4) + b(2800, 0.35, 0.28), 0, 1);
  }

  function drawEq(c) {
    var ctx = c.ctx, W = c.w, H = c.h, mid = H / 2, amp = H * 0.36, p = Store.state.params.eq;
    ctx.clearRect(0, 0, W, H);
    ctx.fillStyle = col.rule; ctx.fillRect(0, mid - 0.5, W, 1);
    var fMin = 30, fMax = 16000, steps = Math.max(40, Math.floor(W / 3)), i, f, x;
    var recent = 0; for (i = 0; i < 6; i++) recent = Math.max(recent, u(Sim.at('raw', Sim.N - 1 - i)));
    var lvl = recent > 0.45 ? recent : 0.12;
    ctx.beginPath(); ctx.moveTo(0, H);
    for (i = 0; i <= steps; i++) {
      x = (i / steps) * W; f = fMin * Math.pow(fMax / fMin, i / steps);
      var bed = lvl * voiceShape(f) * Math.pow(10, clamp(eqDb(f, p), -24, 12) / 20);
      ctx.lineTo(x, H - clamp(bed, 0, 1) * H * 0.85);
    }
    ctx.lineTo(W, H); ctx.closePath(); ctx.fillStyle = col['accent-soft']; ctx.fill();
    ctx.beginPath();
    for (i = 0; i <= steps; i++) {
      x = (i / steps) * W; f = fMin * Math.pow(fMax / fMin, i / steps);
      var y = mid - clamp(eqDb(f, p), -15, 15) / 15 * amp;
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    }
    ctx.lineWidth = 2; ctx.lineJoin = 'round'; ctx.strokeStyle = col.accent; ctx.stroke();
  }

  function drawLane(c) {
    var p = Store.state.params;
    if (c.name === 'eq') drawEq(c);
    else drawWave(c, c.name, c.name === 'gate' ? p.gate.threshold : c.name === 'comp' ? p.comp.threshold : c.name === 'lim' ? p.lim.ceiling : null);
  }

  /* ---------- meters ---------- */
  function Meter(el) {
    this.el = el; this.fill = $('[data-fill]', el); this.peakEl = $('[data-peak]', el); this.val = $('[data-val]', el); this.track = $('[role="meter"]', el);
    this.pk = -90; this.hold = 0; this.clip = 0;
  }
  Meter.prototype.update = function (db, dt, text) {
    var v = clamp((db + 60) / 60, 0, 1);
    this.fill.style.setProperty('--v', v.toFixed(3));
    if (db >= this.pk) { this.pk = db; this.hold = 1.2; } else { this.hold -= dt; if (this.hold < 0) this.pk -= 20 * dt; }
    this.peakEl.style.setProperty('--pk', clamp((this.pk + 60) / 60, 0, 1).toFixed(3));
    if (db > -1.5) this.clip = 1.5; else this.clip = Math.max(0, this.clip - dt);
    this.el.dataset.clip = String(this.clip > 0);
    if (text) {
      var shown = db <= -60 ? MINUS + '60 дБ' : num(Math.round(db)) + ' дБ';
      this.val.textContent = tr(this.clip > 0 ? shown + ', перегруз' : shown);
      this.track.setAttribute('aria-valuenow', String(Math.round(clamp(db, -60, 0))));
    }
  };
  window.UI = { Meter: Meter, num: num, clamp: clamp, MINUS: MINUS, afterSetup: function (before) {
    undoTo = before; autoNote.textContent = tr('Пресет «мой» собран по калибровке. Его можно выбрать наверху, а любой ползунок поправить вручную.'); autoUndo.hidden = false;
    clearTimeout(undoTimer); undoTimer = setTimeout(function () { autoUndo.hidden = true; }, 10000);
  } };
  var mIn = new Meter($('[data-meter="in"]')), mOut = new Meter($('[data-meter="out"]'));
  var loudEl = $('#loud');

  function updateStats() {
    var s = Store.state, p = s.params, n = Sim.now, by = s.bypass;
    var set = function (k, txt) { var el = $('[data-stat="' + k + '"]'), v = tr(txt); if (el && el.textContent !== v) el.textContent = v; };
    set('gate', by ? 'обход' : !p.gate.on ? 'выключен' : n.gateOpen ? 'открыт' : 'закрыт');
    set('nr', by ? 'обход' : !p.nr.on ? 'выключен' : 'шума меньше на ' + num(n.nrDb) + ' дБ');
    set('comp', by ? 'обход' : !p.comp.on ? 'выключен' : 'сжатие ' + num(n.grDb, 1) + ' дБ');
    set('lim', by ? 'обход' : !p.lim.on ? 'выключен' : n.limDb > 0.1 ? 'срезано ' + num(n.limDb, 1) + ' дБ' : 'срезов нет');
  }

  /* ---------- the calibration wizard lives in wizard.js; this is its entry and its undo ---------- */
  var DEFAULT_NOTE = 'Тембр попросит сказать несколько фраз шёпотом, обычным голосом и громко, и подберёт настройки под вас и вашу комнату.';
  var btnAuto = $('#btn-auto'), autoNote = $('#auto-note'), autoUndo = $('#auto-undo'), undoTimer = 0, undoTo = null;
  autoNote.textContent = tr(DEFAULT_NOTE);
  btnAuto.addEventListener('click', function () { location.hash = '#/setup'; });
  autoUndo.addEventListener('click', function () {
    if (!undoTo) return;
    Store.state.preset = undoTo.preset; Store.replaceParams(undoTo.params);
    autoUndo.hidden = true; clearTimeout(undoTimer); autoNote.textContent = tr('Прежние настройки возвращены.');
  });

  /* ---------- loop ---------- */
  var last = 0, textAt = 0, drawAt = 0;
  function frame(ts) {
    requestAnimationFrame(frame);
    if (!last) last = ts;
    var dt = Math.min((ts - last) / 1000, 0.25);
    last = ts;
    var view = appEl.dataset.view;
    if (view === 'welcome') return;
    var slow = reduced.matches;
    if (slow && ts - drawAt < 200) return;
    drawAt = ts;

    var s = Store.state;
    Sim.advance(dt, s.params, s);
    var n = Sim.now, text = ts - textAt > 100;
    if (text) textAt = ts;

    if (view === 'setup') {
      if (window.Wizard) Wizard.frame(dt, text);
      for (var k = 0; k < lanes.length; k++) if (lanes[k].el.hasAttribute('data-hud')) drawLane(lanes[k]);
      return;
    }

    mIn.update(n.inDb, dt, text);
    mOut.update(n.outDb, dt, text);
    if (text) {
      loudEl.textContent = n.loud <= -70 ? MINUS + '70' : num(Math.round(n.loud));
      updateStats();
    }
    for (var i = 0; i < lanes.length; i++) if (!lanes[i].el.hasAttribute('data-hud')) drawLane(lanes[i]);
  }

  /* ---------- devices and first run ---------- */
  var deviceEl = $('#device');
  function detectMain() {
    deviceEl.dataset.state = 'wait'; deviceEl.textContent = tr('ищем микрофон');
    Bridge.detect().then(function (r) {
      var used = r.mic.found || !!r.mic.name;
      deviceEl.dataset.state = used ? 'ok' : 'none';
      deviceEl.textContent = tr(r.mic.found ? r.mic.name + ' подключён' : used ? 'микрофон: ' + r.mic.name : 'микрофон не найден');
    });
  }

  var stMic = $('#st-mic'), stCable = $('#st-cable'), btnStart = $('#btn-start'), startWhy = $('#start-why');
  var ready = false;
  function setStatus(el, text, bad) { el.removeAttribute('aria-busy'); el.textContent = tr(text); if (bad) el.dataset.tone = 'bad'; else delete el.dataset.tone; }
  function setRoute(el, text, bad) { el.textContent = tr(text); if (bad) el.dataset.tone = 'bad'; else delete el.dataset.tone; }

  function runDetect() {
    [stMic, stCable].forEach(function (el) { el.setAttribute('aria-busy', 'true'); el.innerHTML = '<span class="skeleton"></span>'; delete el.dataset.tone; });
    $('#btn-recheck-mic').hidden = true; $('#cable-actions').hidden = true;
    setRoute($('#rt-mic'), 'проверяем'); setRoute($('#rt-cable'), 'проверяем');
    ready = false; btnStart.setAttribute('aria-disabled', 'true'); startWhy.textContent = tr('Ждём результат проверки.');
    Bridge.detect().then(function (r) {
      var micOk = r.mic.found || !!r.mic.name;
      if (r.mic.found) { setStatus(stMic, r.mic.name + ' найден' + (r.mic.transport ? ', подключён по ' + r.mic.transport : '') + '.'); setRoute($('#rt-mic'), 'найден'); }
      else if (micOk) { setStatus(stMic, 'AM8 по названию не найден. Сейчас используется «' + r.mic.name + '», другой микрофон можно выбрать в настройках.'); setRoute($('#rt-mic'), 'используется другой'); }
      else { setStatus(stMic, 'Микрофон не найден. Подключите AM8 по USB и нажмите «проверить снова».', true); setRoute($('#rt-mic'), 'не найден', true); $('#btn-recheck-mic').hidden = false; }
      if (r.cable.found) { setStatus(stCable, r.cable.name + ' найден.'); setRoute($('#rt-cable'), 'найден'); }
      else { setStatus(stCable, 'VB-Cable не найден.', true); setRoute($('#rt-cable'), 'не найден', true); $('#cable-actions').hidden = false; }
      ready = micOk && r.cable.found;
      btnStart.setAttribute('aria-disabled', String(!ready));
      startWhy.textContent = ready ? '' : tr(!micOk && !r.cable.found ? 'Нужны микрофон и кабель.' : !micOk ? 'Нужен микрофон.' : 'Нужен кабель.');
    });
  }
  $('#btn-recheck-mic').addEventListener('click', runDetect);
  $('#btn-recheck-cable').addEventListener('click', runDetect);
  $('#btn-get-cable').addEventListener('click', function () { Bridge.openUrl('https://vb-audio.com/Cable/'); });
  btnStart.addEventListener('click', function () { if (ready) location.hash = '#/'; });

  var tabs = $$('[role="tab"]');
  function selectTab(t) {
    tabs.forEach(function (x) {
      var on = x === t; x.setAttribute('aria-selected', String(on)); x.tabIndex = on ? 0 : -1;
      $('#' + x.getAttribute('aria-controls')).hidden = !on;
    });
  }
  tabs.forEach(function (t, i) {
    t.addEventListener('click', function () { selectTab(t); });
    t.addEventListener('keydown', function (e) {
      var j = e.key === 'ArrowRight' ? i + 1 : e.key === 'ArrowLeft' ? i - 1 : -1;
      if (j < 0) return; e.preventDefault(); var nt = tabs[(j + tabs.length) % tabs.length]; selectTab(nt); nt.focus();
    });
  });

  var appEl = $('#app'), vMain = $('#view-main'), vWelcome = $('#view-welcome'), vSetup = $('#view-setup'), prevView = '';
  function route() {
    var h = (location.hash || '#/').replace(/\?.*$/, ''), view = h === '#/welcome' ? 'welcome' : h === '#/setup' ? 'setup' : 'main';
    if (prevView === 'setup' && view !== 'setup' && window.Wizard) Wizard.leave();
    appEl.dataset.view = view;
    vMain.hidden = view !== 'main'; vWelcome.hidden = view !== 'welcome'; vSetup.hidden = view !== 'setup';
    if (view === 'welcome') runDetect(); else if (view === 'main') detectMain();
    if (view === 'setup' && prevView !== 'setup' && window.Wizard) Wizard.enter();
    prevView = view;
    window.scrollTo(0, 0);
    if (dlg.open) dlg.close();
  }

  /* ---------- settings ---------- */
  var dlg = $('#settings'), capturing = false, hotkey = $('#set-hotkey'), hotkeyHelp = $('#hotkey-help');
  $('#btn-settings').addEventListener('click', function () { dlg.showModal(); });
  $('#settings-x').addEventListener('click', function () { dlg.close(); });
  $('#settings-close').addEventListener('click', function () { dlg.close(); });
  dlg.addEventListener('click', function (e) { if (e.target === dlg) dlg.close(); });
  $('#open-wizard').addEventListener('click', function () { dlg.close(); });
  $('#set-startup').addEventListener('click', function () { Store.setSetting('startup', !Store.state.settings.startup); });

  hotkey.addEventListener('click', function () { capturing = true; hotkey.value = tr('нажмите сочетание'); hotkeyHelp.textContent = tr('Escape отменяет.'); });
  hotkey.addEventListener('blur', function () { if (capturing) { capturing = false; render(); hotkeyHelp.textContent = tr('Нажмите на поле и сочетание клавиш.'); } });
  function combo(e) {
    var k = e.key.length === 1 ? e.key.toUpperCase() : e.key, m = [];
    if (e.ctrlKey) m.push('Ctrl'); if (e.altKey) m.push('Alt'); if (e.shiftKey) m.push('Shift'); if (e.metaKey) m.push('Win');
    return { mods: m.length, text: m.concat([k]).join('+'), key: k };
  }
  window.addEventListener('keydown', function (e) {
    if (capturing) {
      e.preventDefault();
      if (e.key === 'Escape') { capturing = false; render(); hotkeyHelp.textContent = tr('Нажмите на поле и сочетание клавиш.'); return; }
      if (['Control', 'Alt', 'Shift', 'Meta'].indexOf(e.key) >= 0) return;
      var c = combo(e);
      if (!c.mods) { hotkeyHelp.textContent = tr('Добавьте Ctrl, Alt или Shift, чтобы клавиша не мешала набору текста.'); return; }
      capturing = false; Store.setSetting('hotkey', c.text); hotkeyHelp.textContent = tr('Нажмите на поле и сочетание клавиш.'); return;
    }
    if (document.documentElement.classList.contains('native')) return; // the global shortcut handles it
    if (dlg.open || vMain.hidden) return;
    if (e.repeat) return;
    var cc = combo(e);
    if (cc.mods && cc.text === Store.state.settings.hotkey) { e.preventDefault(); Store.setFlag('muted', !Store.state.muted); }
  });

  /* ---------- language ---------- */
  var langSel = $('#set-lang'), langBar = $('#lang-bar');
  if (langSel) { langSel.value = I18N.lang; langSel.addEventListener('change', function () { I18N.set(langSel.value); }); }
  if (langBar) { langBar.textContent = I18N.lang === 'ru' ? 'English' : 'Русский'; langBar.addEventListener('click', function () { I18N.set(I18N.lang === 'ru' ? 'en' : 'ru'); }); }

  /* ---------- go ---------- */
  for (var w = 0; w < 28; w++) Sim.advance(0.25, Store.state.params, Store.state);
  window.addEventListener('hashchange', route);
  render();
  route();
  requestAnimationFrame(frame);
})();
