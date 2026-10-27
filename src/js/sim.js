/* Sample signal and a level-domain model of the chain, so the lanes have something to draw.
   Real builds replace this file with a listener for the Rust core, which sends the same shape:
   per tick, the level in dB at the input and after each stage. */
(function () {
  'use strict';

  var RATE = 50;            // ticks per second
  var N = 360;              // history length, 7.2 seconds
  var FLOOR = -90;
  var STAGES = ['raw', 'gate', 'nr', 'ch', 'comp', 'lim'];

  function db(a) { return a > 1e-5 ? 20 * Math.log10(a) : FLOOR; }
  function lin(d) { return Math.pow(10, d / 20); }

  // small deterministic generator so the demo looks the same on every run
  var seed = 7;
  function rnd() { seed |= 0; seed = (seed + 0x6D2B79F5) | 0; var t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }

  var hist = {}, pos = 0, jit = new Float32Array(N);
  STAGES.forEach(function (s) { hist[s] = new Float32Array(N).fill(FLOOR); });
  for (var i = 0; i < N; i++) jit[i] = 0.74 + 0.26 * rnd();

  var t = 0, acc = 0, mode = 'auto', gainDb = 0;
  var RANGES = { whisper: [0.012, 0.022], normal: [0.12, 0.22], loud: [0.45, 0.62] };
  var seg = { speech: false, end: 0.9, amp: 0.2, ph: 0 };
  var click = 0;
  var g = 1, gr = 0;
  var powerRing = new Float32Array(150).fill(0), powerPos = 0;

  var now = { inDb: FLOOR, outDb: FLOOR, gateOpen: true, nrDb: 0, grDb: 0, limDb: 0, loud: -70 };

  function nextSegment() {
    if (mode === 'silence') { seg = { speech: false, end: t + 1, amp: 0, ph: 0 }; return; }
    if (mode !== 'auto') {
      if (seg.speech) seg = { speech: false, end: t + 0.25 + rnd() * 0.3, amp: 0, ph: 0 };
      else { var rg = RANGES[mode]; seg = { speech: true, end: t + 1.6 + rnd() * 1.6, amp: rg[0] + rnd() * (rg[1] - rg[0]), ph: rnd() }; }
      return;
    }
    if (seg.speech) { seg = { speech: false, end: t + 0.9 + rnd() * 1.1, amp: 0, ph: 0 }; return; }
    var loud = rnd() < 0.18;
    seg = { speech: true, end: t + 2 + rnd() * 2.2, amp: loud ? 0.62 + rnd() * 0.2 : 0.12 + rnd() * 0.22, ph: rnd() };
  }

  function step(p, flags) {
    t += 1 / RATE;
    if (t >= seg.end) nextSegment();

    var v = 0;
    if (seg.speech) {
      var syl = 0.5 + 0.5 * Math.sin(2 * Math.PI * (t * 3.7 + seg.ph));
      v = seg.amp * (0.4 + 0.6 * Math.pow(syl, 1.4)) * (0.86 + 0.14 * rnd());
    }
    var n = 0.0028 * (0.75 + 0.5 * rnd()) * (1 + 0.25 * Math.sin(2 * Math.PI * t * 0.7));
    if (click > 0) click--; else if (rnd() < 0.012) click = 2;
    var c = click > 0 ? 0.045 + 0.04 * rnd() : 0;

    var gk = lin(gainDb);
    v *= gk; n *= gk; c *= gk;
    var inAmp = v + n + c;
    var preDb = db(inAmp);                 // what the microphone delivers
    var sg = lin(p.input ? p.input.gain : 0); // the software gain in front of the chain
    v *= sg; n *= sg; c *= sg; inAmp *= sg;
    var rawDb = db(inAmp);                 // the signal every stage is compared with

    // gate
    var open = rawDb > p.gate.threshold;
    var target = open ? 1 : 0.012;
    g += (target > g ? 0.6 : 0.07) * (target - g);
    var gg = p.gate.on ? g : 1;
    var vv = v * gg, nn = n * gg, cc = c * gg;
    var gateDb = db(vv + nn + cc);

    // noise reduction acts on what is left of the noise
    var s = p.nr.on ? (p.nr.amount / 100) * (p.nr.mode === 'hq' ? 0.96 : 0.72) : 0;
    var nrAmp = vv + (nn + cc) * (1 - s);
    var nrDb = db(nrAmp);

    // eq only nudges the overall level here; the curve itself is drawn from the same parameters
    var eqGain = p.eq.on ? 0.25 * p.eq.low + 0.45 * p.eq.mid + 0.25 * p.eq.high - (p.eq.lowcut ? 0.4 : 0) : 0;
    var L = db(nrAmp * lin(eqGain));
    var chDb = L;

    // compressor
    var Lout = L, grT = 0;
    if (p.comp.on && L > FLOOR) {
      var Lc = L > p.comp.threshold ? p.comp.threshold + (L - p.comp.threshold) / p.comp.ratio : L;
      grT = L - Lc;
    }
    gr += (grT > gr ? 0.5 : 0.06) * (grT - gr);
    if (p.comp.on && L > FLOOR) { var makeup = (-p.comp.threshold) * (1 - 1 / p.comp.ratio) * 0.4; Lout = L - gr + makeup; }

    // limiter
    var cl = Lout, cut = 0;
    if (p.lim.on && Lout > p.lim.ceiling) { cl = p.lim.ceiling; cut = Lout - cl; }

    var bypass = !!flags.bypass;
    var sig = {
      raw: rawDb,
      gate: bypass ? rawDb : gateDb,
      nr: bypass ? rawDb : nrDb,
      ch: bypass ? rawDb : chDb,
      comp: bypass ? rawDb : Lout,
      lim: bypass ? rawDb : cl
    };
    STAGES.forEach(function (k) { hist[k][pos] = sig[k]; });
    jit[pos] = 0.74 + 0.26 * rnd();
    pos = (pos + 1) % N;

    var outDb = flags.muted ? FLOOR : sig.lim;
    var pw = outDb > FLOOR ? Math.pow(lin(outDb), 2) : 0;
    powerRing[powerPos] = pw; powerPos = (powerPos + 1) % powerRing.length;
    var mean = 0; for (var i = 0; i < powerRing.length; i++) mean += powerRing[i]; mean /= powerRing.length;

    now.inDb = preDb;
    now.outDb = outDb;
    now.gateOpen = open || !p.gate.on;
    now.nrDb = s > 0 ? 20 * Math.log10(1 / (1 - s)) : 0;
    now.grDb = bypass ? 0 : gr;
    now.limDb = bypass ? 0 : cut;
    now.loud = mean > 0 ? Math.max(-70, 10 * Math.log10(mean) - 0.7) : -70;
  }

  function advance(dt, p, flags) {
    acc += Math.min(dt, 0.25);
    var tick = 1 / RATE;
    while (acc >= tick) { step(p, flags); acc -= tick; }
  }

  window.Sim = {
    N: N, FLOOR: FLOOR, RATE: RATE, now: now, advance: advance, db: db, lin: lin,
    /* level in dB at the i-th oldest slot of a stage history */
    /* the wizard drives these: which kind of voice the demo signal imitates, and the gain knob on the mic */
    setMode: function (m) { mode = m; seg = { speech: false, end: t, amp: 0, ph: 0 }; },
    setGain: function (d) { gainDb = d; },
    /* spectrum balance of normal speech against the target curve, dB; a measured value in the real core */
    bands: function () { return { low: 4 + (rnd() - 0.5), mid: 0.5 + (rnd() - 0.5), high: -3 + (rnd() - 0.5) }; },
    at: function (stage, i) { return hist[stage][(pos + i) % N]; },
    jitter: function (i) { return jit[(pos + i) % N]; }
  };
})();
