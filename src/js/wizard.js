/* Calibration wizard: gain knob, a silent take, then whisper, normal and loud takes, then the result with a before/after switch.
   The preview imitates the voice with Sim modes. In the real app the same stages record the microphone through the Rust core. */
(function () {
  'use strict';

  var $ = function (s, r) { return (r || document).querySelector(s); };
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };

  var STAGES = [
    { id: 'knob' },
    { id: 'quiet', rec: 5, mode: 'silence', title: 'послушаем комнату', lead: 'Сидите тихо и не двигайтесь, пока идёт запись. Если играет музыка, выключите её.', caption: '', phrase: '' },
    { id: 'whisper', rec: 6, mode: 'whisper', title: 'теперь шёпотом', lead: 'Как ночью, когда рядом кто-то спит. Повторяйте фразу, пока идёт запись.', caption: 'шёпотом', phrase: '«Ночью в доме все спят, поэтому говорю тихо.»' },
    { id: 'normal', rec: 6, mode: 'normal', title: 'обычным голосом', lead: 'Как на созвоне с друзьями, на той дистанции до микрофона, на которой сидите обычно.', caption: 'обычным голосом', phrase: '«Привет, меня слышно хорошо, а шум остался за кадром.»' },
    { id: 'loud', rec: 6, mode: 'loud', title: 'теперь громко', lead: 'Как в самый азартный момент стрима. Перегрузить не страшно: ради этого мы и меряем.', caption: 'громко', phrase: '«Раз, два, три, проверка микрофона!»' },
    { id: 'result' }
  ];
  var ZONE = [-9, -3];

  var S = null, hudIn = null, hudOut = null, ui = null;

  var tr = I18N.t;
  function num(v, d) { return ui.num(v, d); }
  function pct(arr, q) { if (!arr.length) return Sim.FLOOR; var a = arr.slice().sort(function (x, y) { return x - y; }); return a[Math.min(a.length - 1, Math.floor(q * a.length))]; }
  function setBtn(btn, text, disabled) { btn.textContent = tr(text); btn.setAttribute('aria-disabled', String(!!disabled)); }

  /* ---------- real takes and the before/after comparison (native app only) ---------- */
  function native() { return document.documentElement.classList.contains('native') && !!window.NativeAudio; }
  function b64ToI16(b64) { var bin = atob(b64), u = new Uint8Array(bin.length); for (var i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i); return new Int16Array(u.buffer, 0, u.length >> 1); }
  function i16ToB64(a) {
    var u = new Uint8Array(a.buffer, a.byteOffset, a.byteLength), out = '';
    for (var i = 0; i < u.length; i += 0x8000) out += String.fromCharCode.apply(null, u.subarray(i, i + 0x8000));
    return btoa(out);
  }
  function stopPlay() {
    if (!S || !S.player) return;
    try { S.player.srcA.stop(); S.player.srcB.stop(); S.player.ctx.close(); } catch (e) {}
    S.player = null; var b = $('#res-play'); if (b) b.textContent = tr('послушать запись');
  }
  function side() { var c = $('input[name="ab"]:checked'); return c ? c.value : 'after'; }
  function setSide(v) { if (S && S.player) { S.player.ga.gain.value = v === 'before' ? 1 : 0; S.player.gb.gain.value = v === 'before' ? 0 : 1; } }
  function togglePlay() {
    if (!S || !S.ab) return;
    if (S.player) { stopPlay(); return; }
    var ctx = new (window.AudioContext || window.webkitAudioContext)({ sampleRate: 48000 });
    function buf(a) { var b = ctx.createBuffer(1, a.length, 48000), d = b.getChannelData(0); for (var i = 0; i < a.length; i++) d[i] = a[i] / 32768; return b; }
    var ga = ctx.createGain(), gb = ctx.createGain(), srcA = ctx.createBufferSource(), srcB = ctx.createBufferSource();
    srcA.buffer = buf(S.ab.a); srcB.buffer = buf(S.ab.b); srcA.loop = srcB.loop = true;
    srcA.connect(ga); srcB.connect(gb); ga.connect(ctx.destination); gb.connect(ctx.destination);
    S.player = { ctx: ctx, srcA: srcA, srcB: srcB, ga: ga, gb: gb };
    setSide(side()); srcA.start(); srcB.start();
    $('#res-play').textContent = tr('остановить');
  }
  /* The same take of the user's voice goes through the chain twice: with the settings from before and with the new ones. */
  function buildAB() {
    var sess = S, box = $('#ab-play'); box.hidden = true; sess.ab = null;
    if (!native()) return;
    var parts = [], total = 0;
    ['quiet', 'whisper', 'normal', 'loud'].forEach(function (id) {
      var c = sess.clips[id]; if (!c) return;
      if (id === 'quiet') c = c.subarray(0, 96000);
      parts.push(c); total += c.length;
    });
    if (parts.length < 3) return;
    var all = new Int16Array(total), at = 0; parts.forEach(function (c) { all.set(c, at); at += c.length; });
    var b64 = i16ToB64(all);
    $('#ab-status').textContent = tr('Готовлю запись для сравнения…'); box.hidden = false; $('#res-play').setAttribute('aria-disabled', 'true');
    Promise.all([NativeAudio.process(b64, sess.before.params), NativeAudio.process(b64, sess.res.params)]).then(function (r) {
      if (S !== sess) return;
      sess.ab = { a: b64ToI16(r[0]), b: b64ToI16(r[1]) };
      $('#res-play').removeAttribute('aria-disabled');
      $('#ab-status').textContent = tr('Одна и та же запись вашего голоса: переключайте «было» и «стало».');
    }, function () { if (S === sess) box.hidden = true; });
  }
  /* The gain goes into the microphone itself when it can be reached, the rest stays in the program. */
  function splitGain() {
    S.split = null;
    if (!(window.Mic && Mic.available())) return;
    var g = S.res.params.input.gain, hw0 = Mic.gainDb();
    var hw1 = Math.max(-12, Math.min(12, Math.round((hw0 + g) * 2) / 2));
    var sw = Math.max(0, Math.round(g - (hw1 - hw0)));
    S.split = { hw: hw1, sw: sw };
    var row = S.res.rows[0];
    var sgn = function (v) { return (v > 0 ? '+' : v < 0 ? ui.MINUS : '') + Math.abs(v); };
    row.value = Calibrate.F('в микрофоне {} дБ, в программе {} дБ', sgn(hw1), sgn(sw));
    row.why.push('Усиление поставлено в самом микрофоне, поэтому его слышат все программы.');
  }

  var els = null;
  function grab() {
    if (els) return;
    ui = window.UI;
    els = {
      steps: $$('#setup-steps li'), knob: $('#pane-knob'), cap: $('#pane-capture'), res: $('#pane-result'),
      verdict: $('#knob-verdict'), knobNext: $('#knob-next'), knobWhy: $('#knob-why'), gain: $('#demo-gain'), gainOut: $('#demo-gain-out'),
      capTitle: $('#cap-title'), capLead: $('#cap-lead'), capPrompt: $('#cap-prompt'), capCaption: $('#cap-caption'), capPhrase: $('#cap-phrase'),
      capBar: $('#cap-bar'), capStatus: $('#cap-status'), capGo: $('#cap-go'), capRetry: $('#cap-retry'), capBack: $('#cap-back'),
      hudZone: $('#hud-zone'), hudPeak: $('#hud-peak'), hudGain: $('#hud-gain'), hudInBox: $('.hud__in'), hudOutBox: $('.hud__out'),
      figs: $('#res-figs'), rows: $('#res-rows'), notes: $('#res-notes')
    };
    hudIn = new ui.Meter($('[data-meter="hud-in"]'));
    hudOut = new ui.Meter($('[data-meter="hud-out"]'));

    els.gain.addEventListener('input', function () { Sim.setGain(parseFloat(els.gain.value)); els.gainOut.textContent = tr((els.gain.value > 0 ? '+' : els.gain.value < 0 ? ui.MINUS : '') + Math.abs(els.gain.value) + ' дБ'); });
    els.knobNext.addEventListener('click', function () { if (S.canGo) go(1); });
    els.capGo.addEventListener('click', function () {
      if (S.cap.state === 'ready') startRec(); else if (S.cap.state === 'done') go(S.idx + 1);
    });
    els.capRetry.addEventListener('click', function () { S.cap = { state: 'ready', t: 0, samples: [] }; Sim.setMode('normal'); renderCap(); });
    els.capBack.addEventListener('click', function () { go(S.idx - 1); });
    $$('[data-cancel]').forEach(function (b) { b.addEventListener('click', cancel); });
    $('#res-keep').addEventListener('click', keep);
    $('#res-play').addEventListener('click', function () { if (this.getAttribute('aria-disabled') !== 'true') togglePlay(); });
    $('#res-again').addEventListener('click', again);
    $$('input[name="ab"]').forEach(function (i) {
      i.addEventListener('change', function () {
        if (!i.checked || !S.res) return;
        setSide(i.value);
        if (i.value === 'before') { Store.state.preset = S.before.preset; Store.replaceParams(S.before.params); } else Store.replaceParams(S.res.params);
      });
    });
  }

  function fresh(before) {
    return { before: before, clips: {}, split: null, ab: null, player: null, idx: 0, meas: {}, cap: { state: 'ready', t: 0, samples: [] }, canGo: false, win: [], clock: 0, lastCat: '', res: null, lastSec: -1, shownGo: null };
  }

  function enter() {
    grab();
    S = fresh({ preset: Store.state.preset, params: Store.clone(Store.state.params) });
    els.gain.value = -8; Sim.setGain(-8); els.gain.dispatchEvent(new Event('input'));
    Sim.setMode('loud');
    show(0);
  }

  function leave() { stopPlay(); Sim.setMode('auto'); Sim.setGain(0); }

  function cancel() {
    Store.state.preset = S.before.preset; Store.replaceParams(S.before.params);
    location.hash = '#/';
  }

  function again() {
    stopPlay(); var before = S.before; S = fresh(before);
    Store.state.preset = before.preset; Store.replaceParams(before.params);
    $$('input[name="ab"]').forEach(function (i) { i.checked = i.value === 'after'; });
    Sim.setMode('loud'); show(0);
  }

  function keep() {
    var p = S.res.params;
    if (S.split && window.Mic && Mic.available()) { p.input.gain = S.split.sw; Mic.setGain(S.split.hw); }
    stopPlay();
    Store.saveCustom(p); Store.replaceParams(p);
    var before = S.before;
    location.hash = '#/';
    window.UI.afterSetup(before);
  }

  function go(i) { S.idx = Math.max(0, Math.min(STAGES.length - 1, i)); S.cap = { state: 'ready', t: 0, samples: [] }; show(S.idx); }

  function show(i) {
    var st = STAGES[i];
    els.steps.forEach(function (li, k) { li.dataset.state = k < i ? 'done' : k === i ? 'current' : 'todo'; if (k === i) li.setAttribute('aria-current', 'step'); else li.removeAttribute('aria-current'); });
    els.knob.hidden = st.id !== 'knob'; els.cap.hidden = !(i >= 1 && i <= 4); els.res.hidden = st.id !== 'result';
    els.hudInBox.hidden = st.id === 'result'; els.hudOutBox.hidden = st.id !== 'result';
    els.hudZone.hidden = st.id !== 'knob'; $('#hud-zone-note').hidden = st.id !== 'knob';
    if (st.id === 'knob') {
      Sim.setMode('loud');
      setBtn(els.knobNext, 'дальше', !S.canGo);
      els.knobWhy.textContent = tr('Дальше можно, когда фразу слышно и нет перегруза.');
      S.lastCat = ''; els.verdict.textContent = tr('Скажите фразу громко.');
    } else if (st.id === 'result') {
      Sim.setMode('auto'); computeResult();
    } else {
      Sim.setMode(st.mode === 'silence' ? 'silence' : 'normal');
      els.capTitle.textContent = tr(st.title); els.capLead.textContent = tr(st.lead);
      els.capPrompt.hidden = !st.phrase; els.capCaption.textContent = tr(st.caption); els.capPhrase.textContent = tr(st.phrase);
      renderCap();
    }
  }

  function renderCap() {
    var st = STAGES[S.idx], c = S.cap;
    els.capBar.hidden = c.state !== 'rec';
    els.capRetry.hidden = c.state !== 'done';
    els.capBack.hidden = c.state === 'rec';
    if (c.state === 'ready') { setBtn(els.capGo, 'начать запись', false); els.capGo.removeAttribute('aria-busy'); els.capStatus.textContent = tr(st.id === 'quiet' ? 'Запись идёт ' + st.rec + ' секунд.' : 'Запись идёт ' + st.rec + ' секунд, фразу можно повторять по кругу.'); }
    else if (c.state === 'rec') { setBtn(els.capGo, 'идёт запись…', true); els.capGo.setAttribute('aria-busy', 'true'); }
    else { setBtn(els.capGo, S.idx === 4 ? 'показать результат' : 'дальше', false); els.capGo.removeAttribute('aria-busy'); }
  }

  function startRec() {
    var st = STAGES[S.idx];
    S.cap = { state: 'rec', t: 0, samples: [] }; S.lastSec = -1;
    Sim.setMode(st.mode);
    els.capBar.firstElementChild.style.setProperty('--v', '0');
    if (native()) NativeAudio.recStart().catch(function () {});
    renderCap();
  }

  function finishRec() {
    var st = STAGES[S.idx], m = S.meas, s = S.cap.samples, line = '', bad = false;
    S.cap.state = 'done';
    if (native()) { var sess = S, sid = st.id; NativeAudio.recStop().then(function (b) { if (S === sess) sess.clips[sid] = b64ToI16(b); }, function () {}); }
    Sim.setMode(st.id === 'quiet' ? 'silence' : 'normal');
    if (st.id === 'quiet') {
      m.noise = pct(s, 0.9);
      line = 'Шум комнаты ' + num(Math.round(m.noise)) + ' дБ.' + (m.noise > -45 ? ' Шумно, шумодаву придётся работать сильнее.' : '');
    } else {
      var floor = (m.noise === undefined ? -60 : m.noise) + 6;
      var act = s.filter(function (d) { return d > floor; });
      var lvl = pct(act, 0.75);
      if (act.length < 25) { bad = true; line = 'Почти не слышно над шумом комнаты. Повторите ближе к микрофону.'; lvl = Math.max(lvl, floor); }
      if (st.id === 'whisper') { m.whisper = { lvl: lvl }; if (!bad) line = 'Шёпот ' + num(Math.round(lvl)) + ' дБ, на ' + Math.round(lvl - m.noise) + ' дБ выше шума комнаты.'; }
      if (st.id === 'normal') { m.normal = { lvl: lvl }; m.bands = Sim.bands(); if (!bad) line = 'Обычная речь ' + num(Math.round(lvl)) + ' дБ.'; }
      if (st.id === 'loud') {
        var pk = pct(s, 0.98); m.loud = { lvl: lvl, peak: pk };
        if (!bad) line = 'Громкая речь ' + num(Math.round(lvl)) + ' дБ, пик ' + num(Math.round(pk)) + ' дБ.' + (pk > -1 ? ' Звук упёрся в 0 дБ: убавьте усиление ручкой и запишите заново.' : '');
        if (pk > -1) bad = true;
      }
    }
    els.capStatus.textContent = tr(line);
    renderCap();
  }

  function computeResult() {
    var m = S.meas;
    m.noise = m.noise === undefined ? -51 : m.noise;
    m.whisper = m.whisper || { lvl: -36 }; m.normal = m.normal || { lvl: -18 }; m.loud = m.loud || { lvl: -8, peak: -5 }; m.bands = m.bands || { low: 0, mid: 0, high: 0 };
    S.res = Calibrate.compute(m);
    splitGain();
    Store.replaceParams(S.res.params);
    buildAB();
    $$('input[name="ab"]').forEach(function (i) { i.checked = i.value === 'after'; });

    els.figs.innerHTML = ''; S.res.figures.forEach(function (f) {
      var li = document.createElement('li'); li.innerHTML = '<span class="fig__label"></span><output class="fig__val"></output>';
      li.firstChild.textContent = tr(f.label + ', дБ'); li.lastChild.textContent = f.value; els.figs.appendChild(li);
    });
    els.rows.innerHTML = ''; S.res.rows.forEach(function (r) {
      var li = document.createElement('li'); li.innerHTML = '<span class="row__name"></span><span class="row__val"></span><span class="row__why"></span>';
      li.children[0].textContent = tr(r.name); li.children[1].textContent = tr(r.value); li.children[2].textContent = r.why.map(function (x) { return tr(x); }).join(' '); els.rows.appendChild(li);
    });
    els.notes.innerHTML = '';
    S.res.warnings.forEach(function (w) { var li = document.createElement('li'); li.dataset.kind = 'warn'; li.textContent = tr('Внимание:') + ' ' + tr(w); els.notes.appendChild(li); });
    S.res.tips.forEach(function (w) { var li = document.createElement('li'); li.textContent = tr('Совет:') + ' ' + tr(w); els.notes.appendChild(li); });
  }

  /* called by app.js on every frame while the setup view is open */
  function frame(dt, text) {
    if (!S) return;
    var n = Sim.now; S.clock += dt;
    hudIn.update(n.inDb, dt, text);
    if (S.idx === 5) { hudOut.update(n.outDb, dt, text); return; }

    S.win.push({ t: S.clock, db: n.inDb });
    while (S.win.length && S.clock - S.win[0].t > 4) S.win.shift();
    var peak = Sim.FLOOR; for (var i = 0; i < S.win.length; i++) if (S.win[i].db > peak) peak = S.win[i].db;
    if (text) els.hudPeak.textContent = peak <= -60 ? ui.MINUS + '60' : num(Math.round(peak));

    if (S.idx === 0) {
      var lo = ZONE[0], hi = ZONE[1];
      var cat = peak < -42 ? 'quiet' : peak < lo ? 'low' : peak <= hi ? 'ok' : peak <= -1 ? 'hot' : 'clip';
      if (text) els.hudGain.textContent = peak < -42 ? '0' : String(ui.clamp(Math.round(-6 - peak), 0, 36));
      if (cat === 'clip') S.canGo = false; else if (cat !== 'quiet') S.canGo = true;
      var live = S.canGo;
      if (live !== S.shownGo) {
        S.shownGo = live; setBtn(els.knobNext, 'дальше', !live);
        els.knobWhy.textContent = tr(live ? 'Можно идти дальше. Ручку больше не трогайте.' : 'Дальше можно, когда фразу слышно и нет перегруза.');
      }
      if (cat !== S.lastCat) {
        S.lastCat = cat;
        els.verdict.textContent = tr({ quiet: 'Пока тишина. Скажите фразу громко.', low: 'Тихо, но это не страшно: Тембр добавит усиление в программе. Можно прибавить и ручкой.', ok: 'То что нужно: пик в рамке.', hot: 'Близко к перегрузу. Чуть убавьте усиление ручкой.', clip: 'Перегруз. Убавьте усиление ручкой.' }[cat]);
      }
    } else if (S.cap.state === 'rec') {
      var c = S.cap, st = STAGES[S.idx];
      c.t += dt; c.samples.push(n.inDb);
      els.capBar.firstElementChild.style.setProperty('--v', Math.min(1, c.t / st.rec).toFixed(3));
      var left = Math.max(1, Math.ceil(st.rec - c.t));
      if (left !== S.lastSec) { S.lastSec = left; els.capStatus.textContent = tr('Идёт запись, осталось ' + left + ' с.'); }
      if (c.t >= st.rec) finishRec();
    }
  }

  window.Wizard = { enter: enter, leave: leave, frame: frame };
})();
