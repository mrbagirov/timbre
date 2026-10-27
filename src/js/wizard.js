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

  function num(v, d) { return ui.num(v, d); }
  function pct(arr, q) { if (!arr.length) return Sim.FLOOR; var a = arr.slice().sort(function (x, y) { return x - y; }); return a[Math.min(a.length - 1, Math.floor(q * a.length))]; }
  function setBtn(btn, text, disabled) { btn.textContent = text; btn.setAttribute('aria-disabled', String(!!disabled)); }

  var els = null;
  function grab() {
    if (els) return;
    ui = window.UI;
    els = {
      steps: $$('#setup-steps li'), knob: $('#pane-knob'), cap: $('#pane-capture'), res: $('#pane-result'),
      verdict: $('#knob-verdict'), knobNext: $('#knob-next'), knobWhy: $('#knob-why'), gain: $('#demo-gain'), gainOut: $('#demo-gain-out'),
      capTitle: $('#cap-title'), capLead: $('#cap-lead'), capPrompt: $('#cap-prompt'), capCaption: $('#cap-caption'), capPhrase: $('#cap-phrase'),
      capBar: $('#cap-bar'), capStatus: $('#cap-status'), capGo: $('#cap-go'), capRetry: $('#cap-retry'), capBack: $('#cap-back'),
      hudZone: $('#hud-zone'), hudPeak: $('#hud-peak'), hudInBox: $('.hud__in'), hudOutBox: $('.hud__out'),
      figs: $('#res-figs'), rows: $('#res-rows'), notes: $('#res-notes')
    };
    hudIn = new ui.Meter($('[data-meter="hud-in"]'));
    hudOut = new ui.Meter($('[data-meter="hud-out"]'));

    els.gain.addEventListener('input', function () { Sim.setGain(parseFloat(els.gain.value)); els.gainOut.textContent = (els.gain.value > 0 ? '+' : els.gain.value < 0 ? ui.MINUS : '') + Math.abs(els.gain.value) + ' дБ'; });
    els.knobNext.addEventListener('click', function () { if (S.zoneHit) go(1); });
    els.capGo.addEventListener('click', function () {
      if (S.cap.state === 'ready') startRec(); else if (S.cap.state === 'done') go(S.idx + 1);
    });
    els.capRetry.addEventListener('click', function () { S.cap = { state: 'ready', t: 0, samples: [] }; Sim.setMode('normal'); renderCap(); });
    els.capBack.addEventListener('click', function () { go(S.idx - 1); });
    $$('[data-cancel]').forEach(function (b) { b.addEventListener('click', cancel); });
    $('#res-keep').addEventListener('click', keep);
    $('#res-again').addEventListener('click', again);
    $$('input[name="ab"]').forEach(function (i) {
      i.addEventListener('change', function () {
        if (!i.checked || !S.res) return;
        if (i.value === 'before') { Store.state.preset = S.before.preset; Store.replaceParams(S.before.params); } else Store.replaceParams(S.res.params);
      });
    });
  }

  function fresh(before) {
    return { before: before, idx: 0, meas: {}, cap: { state: 'ready', t: 0, samples: [] }, zoneHit: false, win: [], clock: 0, lastCat: '', res: null, lastSec: -1 };
  }

  function enter() {
    grab();
    S = fresh({ preset: Store.state.preset, params: Store.clone(Store.state.params) });
    els.gain.value = -8; Sim.setGain(-8); els.gain.dispatchEvent(new Event('input'));
    Sim.setMode('loud');
    show(0);
  }

  function leave() { Sim.setMode('auto'); Sim.setGain(0); }

  function cancel() {
    Store.state.preset = S.before.preset; Store.replaceParams(S.before.params);
    location.hash = '#/';
  }

  function again() {
    var before = S.before; S = fresh(before);
    Store.state.preset = before.preset; Store.replaceParams(before.params);
    $$('input[name="ab"]').forEach(function (i) { i.checked = i.value === 'after'; });
    Sim.setMode('loud'); show(0);
  }

  function keep() {
    Store.saveCustom(S.res.params);
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
      setBtn(els.knobNext, 'дальше', !S.zoneHit);
      els.knobWhy.textContent = 'Дальше можно, когда пик попадёт в рамку на шкале.';
      S.lastCat = ''; els.verdict.textContent = 'Скажите фразу громко.';
    } else if (st.id === 'result') {
      Sim.setMode('auto'); computeResult();
    } else {
      Sim.setMode(st.mode === 'silence' ? 'silence' : 'normal');
      els.capTitle.textContent = st.title; els.capLead.textContent = st.lead;
      els.capPrompt.hidden = !st.phrase; els.capCaption.textContent = st.caption; els.capPhrase.textContent = st.phrase;
      renderCap();
    }
  }

  function renderCap() {
    var st = STAGES[S.idx], c = S.cap;
    els.capBar.hidden = c.state !== 'rec';
    els.capRetry.hidden = c.state !== 'done';
    els.capBack.hidden = c.state === 'rec';
    if (c.state === 'ready') { setBtn(els.capGo, 'начать запись', false); els.capGo.removeAttribute('aria-busy'); els.capStatus.textContent = st.id === 'quiet' ? 'Запись идёт ' + st.rec + ' секунд.' : 'Запись идёт ' + st.rec + ' секунд, фразу можно повторять по кругу.'; }
    else if (c.state === 'rec') { setBtn(els.capGo, 'идёт запись…', true); els.capGo.setAttribute('aria-busy', 'true'); }
    else { setBtn(els.capGo, S.idx === 4 ? 'показать результат' : 'дальше', false); els.capGo.removeAttribute('aria-busy'); }
  }

  function startRec() {
    var st = STAGES[S.idx];
    S.cap = { state: 'rec', t: 0, samples: [] }; S.lastSec = -1;
    Sim.setMode(st.mode);
    els.capBar.firstElementChild.style.setProperty('--v', '0');
    renderCap();
  }

  function finishRec() {
    var st = STAGES[S.idx], m = S.meas, s = S.cap.samples, line = '', bad = false;
    S.cap.state = 'done';
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
    els.capStatus.textContent = line;
    renderCap();
  }

  function computeResult() {
    var m = S.meas;
    m.noise = m.noise === undefined ? -51 : m.noise;
    m.whisper = m.whisper || { lvl: -36 }; m.normal = m.normal || { lvl: -18 }; m.loud = m.loud || { lvl: -8, peak: -5 }; m.bands = m.bands || { low: 0, mid: 0, high: 0 };
    S.res = Calibrate.compute(m);
    Store.replaceParams(S.res.params);
    $$('input[name="ab"]').forEach(function (i) { i.checked = i.value === 'after'; });

    els.figs.innerHTML = ''; S.res.figures.forEach(function (f) {
      var li = document.createElement('li'); li.innerHTML = '<span class="fig__label"></span><output class="fig__val"></output>';
      li.firstChild.textContent = f.label + ', дБ'; li.lastChild.textContent = f.value; els.figs.appendChild(li);
    });
    els.rows.innerHTML = ''; S.res.rows.forEach(function (r) {
      var li = document.createElement('li'); li.innerHTML = '<span class="row__name"></span><span class="row__val"></span><span class="row__why"></span>';
      li.children[0].textContent = r.name; li.children[1].textContent = r.value; li.children[2].textContent = r.why; els.rows.appendChild(li);
    });
    els.notes.innerHTML = '';
    S.res.warnings.forEach(function (w) { var li = document.createElement('li'); li.dataset.kind = 'warn'; li.textContent = 'Внимание: ' + w; els.notes.appendChild(li); });
    S.res.tips.forEach(function (w) { var li = document.createElement('li'); li.textContent = 'Совет: ' + w; els.notes.appendChild(li); });
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
      var cat = peak < -45 ? 'quiet' : peak < lo ? 'low' : peak <= hi ? 'ok' : peak <= -1 ? 'hot' : 'clip';
      if (cat === 'ok' && !S.zoneHit) { S.zoneHit = true; setBtn(els.knobNext, 'дальше', false); els.knobWhy.textContent = 'Можно идти дальше. Ручку больше не трогайте.'; }
      if (cat !== S.lastCat) {
        S.lastCat = cat;
        els.verdict.textContent = { quiet: 'Пока тишина. Скажите фразу громко.', low: 'Тихо. Прибавьте усиление на микрофоне.', ok: 'То что нужно: пик в рамке.', hot: 'Близко к перегрузу. Чуть убавьте усиление.', clip: 'Перегруз. Убавьте усиление.' }[cat];
      }
    } else if (S.cap.state === 'rec') {
      var c = S.cap, st = STAGES[S.idx];
      c.t += dt; c.samples.push(n.inDb);
      els.capBar.firstElementChild.style.setProperty('--v', Math.min(1, c.t / st.rec).toFixed(3));
      var left = Math.max(1, Math.ceil(st.rec - c.t));
      if (left !== S.lastSec) { S.lastSec = left; els.capStatus.textContent = 'Идёт запись, осталось ' + left + ' с.'; }
      if (c.t >= st.rec) finishRec();
    }
  }

  window.Wizard = { enter: enter, leave: leave, frame: frame };
})();
