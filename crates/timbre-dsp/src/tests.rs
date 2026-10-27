use super::*;
use crate::biquad::Coeffs;
use crate::dynamics::{Compressor, Gate, Limiter};
use crate::resample::Resampler;
use std::f32::consts::PI;

const SR: f32 = 48_000.0;

fn sine(f: f32, amp: f32, n: usize, sr: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * (2.0 * PI * f * i as f32 / sr).sin())
        .collect()
}
struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    }
}
fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}
fn rms_db(x: &[f32]) -> f32 {
    10.0 * (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32)
        .max(1e-12)
        .log10()
}
fn neutral() -> Params {
    let mut p = Params::default();
    p.ch.on = false;
    p.gate.on = false;
    p.nr.on = false;
    p.eq.on = false;
    p.comp.on = false;
    p.lim.on = false;
    p
}

#[test]
fn eq_filters_have_the_expected_response() {
    let hp = Coeffs::highpass(SR, 80.0, 0.707);
    assert!(
        hp.magnitude_db(SR, 40.0) < -10.0,
        "{}",
        hp.magnitude_db(SR, 40.0)
    );
    assert!(hp.magnitude_db(SR, 1000.0).abs() < 0.1);
    let ls = Coeffs::low_shelf(SR, 160.0, 6.0, 1.0);
    assert!((ls.magnitude_db(SR, 20.0) - 6.0).abs() < 0.4);
    assert!(ls.magnitude_db(SR, 5000.0).abs() < 0.2);
    let hs = Coeffs::high_shelf(SR, 5500.0, 6.0, 1.0);
    assert!((hs.magnitude_db(SR, 20000.0) - 6.0).abs() < 0.5);
    assert!(hs.magnitude_db(SR, 200.0).abs() < 0.2);
    let pk = Coeffs::peaking(SR, 1200.0, 6.0, 0.9);
    assert!((pk.magnitude_db(SR, 1200.0) - 6.0).abs() < 0.2);
    assert!(pk.magnitude_db(SR, 60.0).abs() < 0.5);
}

#[test]
fn resampler_keeps_pitch_and_length() {
    for (from, to) in [(44_100u32, 48_000u32), (96_000, 48_000), (48_000, 44_100)] {
        let mut r = Resampler::new(from, to);
        let input = sine(1000.0, 0.5, from as usize, from as f32); // one second
        let mut out = Vec::new();
        for chunk in input.chunks(441) {
            r.push(chunk, &mut out);
        }
        let expect = to as f32;
        assert!(
            (out.len() as f32 - expect).abs() < 8.0,
            "{from}->{to}: {}",
            out.len()
        );
        // count zero crossings: 1 kHz over one second is about 2000
        let zc = out.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        assert!(
            (zc as i32 - 1000).abs() <= 3,
            "{from}->{to}: {zc} rising crossings"
        );
        assert!(peak(&out[100..]) > 0.45 && peak(&out) < 0.52);
    }
}

#[test]
fn gate_closes_on_the_room_and_opens_on_speech() {
    let mut g = Gate::new(SR);
    let mut rng = Lcg(7);
    // room at about -60 dB
    let mut last = 0.0;
    for _ in 0..80 {
        let mut b: Vec<f32> = (0..FRAME).map(|_| rng.next() * 0.001).collect();
        g.process(&mut b, -50.0, true);
        last = rms_db(&b);
    }
    assert!(last < -90.0 + 20.0, "room still at {last} dB"); // well below the -63 dB it came in at
                                                             // speech-level tone
    let mut open = false;
    let mut tail = vec![];
    for k in 0..20 {
        let mut b = sine(300.0, 0.1, FRAME, SR);
        open = g.process(&mut b, -50.0, true);
        if k > 15 {
            tail.extend(b);
        }
    }
    assert!(open);
    assert!(
        (peak(&tail) - 0.1).abs() < 0.01,
        "speech peak {}",
        peak(&tail)
    );
}

#[test]
fn gate_holds_through_a_short_pause() {
    let mut g = Gate::new(SR);
    for _ in 0..10 {
        let mut b = sine(300.0, 0.1, FRAME, SR);
        g.process(&mut b, -50.0, true);
    }
    let mut b = vec![0.0; FRAME];
    assert!(
        g.process(&mut b, -50.0, true),
        "closed after one silent block"
    );
}

#[test]
fn compressor_reaches_the_static_curve() {
    let mut c = Compressor::new(SR);
    let amp = 10f32.powf(-10.0 / 20.0);
    let mut out = vec![];
    for k in 0..100 {
        let mut b = sine(440.0, amp, FRAME, SR);
        c.process(&mut b, -20.0, 4.0, true);
        if k >= 90 {
            out.extend(b);
        }
    }
    // -20 + (-10 - -20) / 4 = -17.5, plus makeup 20 * 0.75 * 0.4 = 6
    let expect = -11.5;
    let got = 20.0 * peak(&out).log10();
    assert!((got - expect).abs() < 1.2, "got {got}, expected {expect}");
}

#[test]
fn limiter_never_passes_the_ceiling_and_delays_by_the_lookahead() {
    let mut l = Limiter::new(SR);
    let ceil = 10f32.powf(-6.0 / 20.0);
    let mut worst = 0.0f32;
    for k in 0..40 {
        let mut b: Vec<f32> = sine(200.0, 1.0, FRAME, SR);
        let cut = l.process(&mut b, -6.0, true);
        if k > 2 {
            worst = worst.max(peak(&b));
            assert!(cut > 0.0);
        }
    }
    assert!(worst <= ceil + 1e-4, "peak {worst} over ceiling {ceil}");
    let mut l = Limiter::new(SR);
    let mut b = vec![0.0; FRAME];
    b[0] = 0.4;
    l.process(&mut b, -1.0, true);
    let idx = b.iter().position(|v| v.abs() > 0.3).unwrap();
    assert_eq!(idx, LOOKAHEAD);
}

#[test]
fn loudness_of_a_reference_tone() {
    let mut l = loudness::Loudness::new(300);
    let mut v = 0.0;
    for _ in 0..320 {
        v = l.push(&sine(1000.0, 0.1, FRAME, SR));
    }
    assert!(v > -23.5 && v < -21.5, "{v}");
    let mut s = loudness::Loudness::new(300);
    assert_eq!(s.push(&vec![0.0; FRAME]), -70.0);
}

#[test]
fn chain_latency_is_constant_with_everything_off() {
    let mut c = Chain::new();
    let p = neutral();
    let f = Flags::default();
    let mut all = vec![];
    for k in 0..6 {
        let mut input = vec![0.0f32; FRAME];
        if k == 0 {
            input[10] = 0.5;
        }
        let mut out = vec![0.0f32; FRAME];
        c.process(&input, &mut out, &p, &f);
        all.extend(out);
    }
    let idx = all
        .iter()
        .position(|v| v.abs() > 0.25)
        .expect("impulse lost");
    assert_eq!(idx, 10 + LATENCY);
}

#[test]
fn bypass_has_the_same_latency_as_the_chain() {
    let mut c = Chain::new();
    let p = Params::default();
    let f = Flags { bypass: true };
    let mut all = vec![];
    for k in 0..6 {
        let mut input = vec![0.0f32; FRAME];
        if k == 0 {
            input[10] = 0.5;
        }
        let mut out = vec![0.0f32; FRAME];
        c.process(&input, &mut out, &p, &f);
        all.extend(out);
    }
    let idx = all.iter().position(|v| v.abs() > 0.25).unwrap();
    assert_eq!(idx, 10 + LATENCY);
}

#[test]
fn denoise_is_stable_and_amount_zero_is_transparent() {
    // RNNoise is a speech/noise classifier, flat synthetic hiss is outside what it was trained on,
    // so this checks stability and the dry path only. Real suppression has to be judged on real audio.
    let mut c = Chain::new();
    let mut p = neutral();
    p.nr.on = true;
    p.nr.mode = "hq".into();
    p.nr.amount = 0.0;
    let f = Flags::default();
    let mut rng = Lcg(42);
    let mut sig = vec![];
    let mut all = vec![];
    for _ in 0..40 {
        let input: Vec<f32> = (0..FRAME).map(|_| rng.next() * 0.05).collect();
        let mut out = vec![0.0f32; FRAME];
        c.process(&input, &mut out, &p, &f);
        sig.extend(input);
        all.extend(out);
    }
    for i in 0..all.len() - LATENCY {
        assert!(
            (all[i + LATENCY] - sig[i]).abs() < 1e-5,
            "dry path altered at {i}"
        );
    }
    p.nr.amount = 100.0;
    for _ in 0..200 {
        let input: Vec<f32> = (0..FRAME).map(|_| rng.next() * 0.2).collect();
        let mut out = vec![0.0f32; FRAME];
        let m = c.process(&input, &mut out, &p, &f);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
        assert!(m.nr_reduction >= 0.0 && m.nr_reduction <= 30.0);
    }
}

/// Where does the denoiser's output line up with its input? NR_DELAY is set from this.
#[test]
fn denoise_alignment_probe() {
    use nnnoiseless::DenoiseState;
    let mut d = DenoiseState::new();
    let n = FRAME * 120;
    // voiced-like: harmonics of a gliding pitch under a syllable envelope, so the lag is unambiguous
    let mut phase = 0.0f32;
    let sig: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let f0 = 110.0 + 90.0 * (t / 1.2).fract() + 25.0 * (2.0 * PI * 0.7 * t).sin();
            phase += 2.0 * PI * f0 / SR;
            let env = 0.5 + 0.5 * (2.0 * PI * 3.0 * t).sin();
            let mut s = 0.0;
            for h in 1..=12 {
                s += (phase * h as f32).sin() / h as f32;
            }
            s * env * 0.12
        })
        .collect();
    let mut out = vec![0.0f32; n];
    for (i, chunk) in sig.chunks(FRAME).enumerate() {
        let scaled: Vec<f32> = chunk.iter().map(|v| v * 32768.0).collect();
        let mut o = vec![0.0f32; FRAME];
        d.process_frame(&mut o, &scaled);
        for (j, v) in o.iter().enumerate() {
            out[i * FRAME + j] = v / 32768.0;
        }
    }
    let mut best = (0usize, f32::MIN);
    // proper cross-correlation: out[i + lag] against sig[i]
    for lag in 0..1200usize {
        let mut s = 0.0f32;
        for i in FRAME * 40..FRAME * 100 {
            s += out[i + lag] * sig[i];
        }
        if s > best.1 {
            best = (lag, s);
        }
    }
    println!("denoiser output lags its input by {} samples", best.0);
    assert!(
        (FRAME - 2..=FRAME + 1).contains(&best.0),
        "NR_DELAY must match the denoiser, measured {}",
        best.0
    );
}

fn zero_crossings(x: &[f32]) -> usize {
    x.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count()
}

fn run_character(sig: &[f32], s: &character::Settings) -> Vec<f32> {
    let mut c = character::Character::new();
    let mut out = sig.to_vec();
    for chunk in out.chunks_mut(FRAME) {
        c.process(chunk, s);
    }
    out
}

#[test]
fn character_off_is_a_pure_delay() {
    let sig = sine(300.0, 0.4, FRAME * 20, SR);
    let out = run_character(
        &sig,
        &character::Settings {
            on: true,
            ..Default::default()
        },
    );
    for i in 0..sig.len() - character::CH_DELAY {
        assert!(
            (out[i + character::CH_DELAY] - sig[i]).abs() < 1e-5,
            "differs at {i}"
        );
    }
}

#[test]
fn pitch_shift_moves_the_frequency_by_an_octave() {
    let n = FRAME * 100;
    let sig = sine(300.0, 0.4, n, SR);
    let secs = (n - FRAME * 20) as f32 / SR;
    for (st, want) in [(12.0f32, 600.0f32), (-12.0, 150.0), (7.0, 449.0)] {
        let out = run_character(
            &sig,
            &character::Settings {
                on: true,
                pitch: st,
                ..Default::default()
            },
        );
        let hz = zero_crossings(&out[FRAME * 20..]) as f32 / secs;
        assert!(
            (hz - want).abs() / want < 0.06,
            "{st} semitones: {hz} Hz, expected about {want}"
        );
        assert!(out[FRAME * 20..]
            .iter()
            .all(|v| v.is_finite() && v.abs() < 1.0));
    }
}

#[test]
fn radio_keeps_the_voice_band_and_cuts_the_edges() {
    let level = |f: f32| {
        let out = run_character(
            &sine(f, 0.3, FRAME * 40, SR),
            &character::Settings {
                on: true,
                radio: 100.0,
                ..Default::default()
            },
        );
        rms_db(&out[FRAME * 20..])
    };
    let mid = level(1000.0);
    assert!(
        level(80.0) < mid - 15.0,
        "low edge only {} dB down",
        mid - level(80.0)
    );
    assert!(
        level(9000.0) < mid - 15.0,
        "high edge only {} dB down",
        mid - level(9000.0)
    );
}

#[test]
fn warmth_stays_bounded_and_adds_body() {
    let sig = sine(200.0, 0.2, FRAME * 40, SR);
    let out = run_character(
        &sig,
        &character::Settings {
            on: true,
            warmth: 100.0,
            ..Default::default()
        },
    );
    assert!(out.iter().all(|v| v.is_finite() && v.abs() < 1.2));
    let d = rms_db(&out[FRAME * 20..]) - rms_db(&sig[FRAME * 20..]);
    assert!(d.abs() < 4.0, "level jumped by {d} dB");
    // a pure sine gets odd harmonics: the waveform is no longer a sine
    let sine_out = run_character(
        &sig,
        &character::Settings {
            on: true,
            ..Default::default()
        },
    );
    let diff: f32 = out[FRAME * 20..]
        .iter()
        .zip(&sine_out[FRAME * 20..])
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / out[FRAME * 20..].len() as f32;
    assert!(diff > 0.01, "saturation did nothing ({diff})");
}

#[test]
fn robot_and_thickness_change_the_signal_and_stay_finite() {
    let sig = sine(250.0, 0.3, FRAME * 40, SR);
    for s in [
        character::Settings {
            on: true,
            robot: 100.0,
            ..Default::default()
        },
        character::Settings {
            on: true,
            thick: 100.0,
            ..Default::default()
        },
    ] {
        let out = run_character(&sig, &s);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() < 1.5));
        let plain = run_character(
            &sig,
            &character::Settings {
                on: true,
                ..Default::default()
            },
        );
        let diff: f32 = out[FRAME * 20..]
            .iter()
            .zip(&plain[FRAME * 20..])
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / out[FRAME * 20..].len() as f32;
        assert!(diff > 0.02, "no audible change ({diff})");
    }
}

#[test]
fn software_gain_raises_the_level_and_reports_both_peaks() {
    let mut c = Chain::new();
    let mut p = neutral();
    p.input.gain = 12.0;
    let f = Flags::default();
    let (mut pre, mut raw) = (f32::MIN, f32::MIN);
    let mut tail = vec![];
    let sig = sine(300.0, 0.05, FRAME * 400, SR);
    for (k, block) in sig.chunks(FRAME).enumerate() {
        let mut out = vec![0.0f32; FRAME];
        let m = c.process(block, &mut out, &p, &f);
        if k > 380 {
            pre = pre.max(m.pre);
            raw = raw.max(m.raw);
            tail.extend(out);
        }
    }
    assert!((pre - (-26.0)).abs() < 0.5, "pre {pre}");
    assert!((raw - (-14.0)).abs() < 0.5, "raw {raw}");
    assert!((20.0 * peak(&tail).log10() - (-14.0)).abs() < 0.6);
}

#[test]
fn chain_with_all_effects_on_stays_finite() {
    let mut c = Chain::new();
    let mut p = Params::default();
    p.input.gain = 18.0;
    p.ch = params_char(5.0, 80.0, 60.0, 30.0, 20.0);
    let f = Flags::default();
    let mut rng = Lcg(3);
    for k in 0..300 {
        let input: Vec<f32> = (0..FRAME)
            .map(|i| {
                0.2 * (2.0 * PI * 220.0 * (k * FRAME + i) as f32 / SR).sin() + rng.next() * 0.01
            })
            .collect();
        let mut out = vec![0.0f32; FRAME];
        let m = c.process(&input, &mut out, &p, &f);
        assert!(
            out.iter().all(|v| v.is_finite() && v.abs() <= 1.0),
            "block {k}"
        );
        assert!(m.ch.is_finite() && m.pre.is_finite());
    }
}

fn params_char(pitch: f32, warmth: f32, thick: f32, radio: f32, robot: f32) -> CharP {
    CharP {
        on: true,
        pitch,
        warmth,
        thick,
        radio,
        robot,
    }
}
