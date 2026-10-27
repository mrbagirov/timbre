/* Calibration rules: measurements in, chain parameters and reasons out.
   Pure functions, no DOM, so the same file can be unit-tested in Node and reused as the spec for the Rust core.

   Measurements, all in dB relative to full scale:
     noise    level of the room, 90th percentile of the silent take
     whisper  { lvl } speech level of the whispered take (75th percentile of the frames above the noise)
     normal   { lvl } same for the normal take
     loud     { lvl, peak } speech level and the 98th percentile of the loud take
     bands    spectrum balance of normal speech against a target curve, dB: { low, mid, high }
*/
(function () {
  'use strict';

  var MINUS = '−';
  var DEC = typeof I18N !== 'undefined' ? I18N.dec : ',';
  function clamp(v, a, b) { return Math.max(a, Math.min(b, v)); }
  function r(v) { return Math.round(v); }
  function n(v, d) { var s = Math.abs(v).toFixed(d || 0).replace('.', DEC); return (v < 0 && Number(s.replace(DEC, '.')) !== 0 ? MINUS : '') + s; }
  function sg(v) { return v > 0 ? '+' + v : v < 0 ? MINUS + Math.abs(v) : '0'; }
  /* Fills "{}" holes in order. Every template here is a key of the English dictionary. */
  function F(tpl) { var a = Array.prototype.slice.call(arguments, 1), i = 0; return tpl.replace(/\{\}/g, function () { return a[i++]; }); }

  function compute(m) {
    var warnings = [], tips = [], rows = [];
    var noise = m.noise, W = m.whisper.lvl, N = m.normal.lvl, L = m.loud.lvl, peak = m.loud.peak;
    var snr = W - noise;

    /* gate: above the room, below the whisper */
    var gateThr;
    if (snr >= 14) gateThr = noise + 8; else if (snr >= 8) gateThr = noise + 5; else gateThr = noise + 3;
    gateThr = r(clamp(Math.max(Math.min(gateThr, W - 6), noise + 2), -65, -30));
    if (snr < 8) warnings.push(F('Шёпот всего на {} дБ выше шума комнаты, поэтому гейт сделан мягким и может подрезать концы слов. Помогает подвинуть микрофон ближе ко рту или тише включить вентилятор.', r(snr)));
    rows.push({ key: 'gate', name: 'гейт', value: F('{} дБ', n(gateThr)),
      why: [F('Порог на {} дБ выше шума комнаты ({} дБ) и на {} дБ ниже шёпота, слова не обрежутся.', r(gateThr - noise), n(r(noise)), r(W - gateThr))] });

    /* noise reduction: stronger the louder the room */
    var hq = noise > -58;
    var amount = r(clamp(50 + (noise + 65) * 2.5, 40, 90) / 5) * 5;
    rows.push({ key: 'nr', name: 'шумодав', value: F(hq ? 'качественный, {}%' : 'лёгкий, {}%', amount),
      why: [hq ? F('Фон в комнате заметный ({} дБ), включён качественный режим.', n(r(noise))) : F('Комната тихая ({} дБ), лёгкого режима хватает и он не трогает голос.', n(r(noise)))] });

    /* eq: pull the spectrum toward the target */
    var b = m.bands || { low: 0, mid: 0, high: 0 };
    var eqLow = r(clamp(-0.7 * b.low, -6, 6)), eqMid = r(clamp(-0.5 * b.mid, -4, 4)), eqHigh = r(clamp(-0.7 * b.high, -6, 6));
    var eqWhy = [];
    if (b.low > 2) { eqWhy.push(F('Низа на {} дБ больше нормы, обычно это близко к микрофону.', r(b.low))); tips.push('Отодвиньтесь от микрофона на ширину ладони: вблизи низ раздувается, и эквалайзер лечит это хуже, чем расстояние.'); }
    else if (b.low < -2) eqWhy.push(F('Низа на {} дБ меньше нормы, добавлено тепло.', r(-b.low)));
    if (b.high < -2) eqWhy.push(F('Верха не хватает на {} дБ, добавлена чёткость.', r(-b.high)));
    else if (b.high > 2) eqWhy.push(F('Верх звенит на {} дБ, приглушён.', r(b.high)));
    if (!eqWhy.length) eqWhy.push('Тембр близок к целевому, правки минимальны.');
    eqWhy.push('Срез низа на 80 Гц включён.');
    rows.push({ key: 'eq', name: 'эквалайзер', value: F('низ {}, середина {}, верх {} дБ', sg(eqLow), sg(eqMid), sg(eqHigh)), why: eqWhy });

    /* compressor: threshold under normal speech, ratio by the spread */
    var D = L - W;
    var thr = r(clamp(N - 5, -36, -12));
    var ratio = D >= 36 ? 4 : D >= 28 ? 3 : D >= 20 ? 2.5 : 2;
    rows.push({ key: 'comp', name: 'компрессор', value: F('{} дБ, {} к 1', n(thr), String(ratio).replace('.', DEC)),
      why: [F('Между шёпотом и громкой речью {} дБ. Компрессор берёт всё, что громче обычной речи, и выравнивает.', r(D))] });

    /* limiter: just under the loudest take */
    var ceil = peak > -3 ? -2 : -1;
    if (peak > -1) warnings.push('Громкая фраза упёрлась в 0 дБ. Убавьте усиление ручкой на микрофоне и пройдите калибровку заново, иначе звук будет хрипеть до обработки.');
    if (peak < -14) warnings.push(F('Громкая фраза тихая, пик {} дБ. Прибавьте усиление ручкой на микрофоне: чем выше сигнал над шумом, тем меньше работы у шумодава.', n(r(peak))));
    rows.push({ key: 'lim', name: 'лимитер', value: F('{} дБ', n(ceil, 1)), why: [F('Самая громкая фраза доходит до {} дБ, потолок поставлен чуть ниже нуля.', n(r(peak)))] });

    return {
      params: {
        gate: { on: true, threshold: gateThr },
        nr: { on: true, mode: hq ? 'hq' : 'light', amount: amount },
        eq: { on: true, lowcut: true, low: eqLow, mid: eqMid, high: eqHigh },
        comp: { on: true, threshold: thr, ratio: ratio },
        lim: { on: true, ceiling: ceil }
      },
      rows: rows,
      figures: [
        { label: 'шум комнаты', value: n(r(noise)) },
        { label: 'шёпот', value: n(r(W)) },
        { label: 'обычная речь', value: n(r(N)) },
        { label: 'громко', value: n(r(L)) }
      ],
      warnings: warnings,
      tips: tips
    };
  }

  var api = { compute: compute };
  if (typeof module !== 'undefined' && module.exports) module.exports = api; else window.Calibrate = api;
})();
