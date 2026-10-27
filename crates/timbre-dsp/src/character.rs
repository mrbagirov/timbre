//! Voice character: pitch, warmth, thickness, radio band and robot.
//! Always delays the signal by `CH_DELAY` samples, whatever is switched on, so the latency of the chain never changes.
use crate::biquad::{Biquad, Coeffs};
use std::f32::consts::PI;

const SR: f32 = 48_000.0;
/// Grain length of the pitch shifter in samples (32 ms).
const WIN: usize = 1536;
/// Fixed delay of the whole stage: the average delay of the pitch shifter.
pub const CH_DELAY: usize = WIN / 2;

struct DelayLine {
    buf: Vec<f32>,
    pos: usize,
}

impl DelayLine {
    fn new(n: usize) -> Self {
        DelayLine {
            buf: vec![0.0; n],
            pos: 0,
        }
    }
    fn push(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) % self.buf.len();
    }
    /// The sample written `d` samples ago (fractional, linear interpolation); d = 1 is the latest sample.
    fn tap(&self, d: f32) -> f32 {
        let n = self.buf.len();
        let d = d.clamp(1.0, (n - 2) as f32);
        let i = d.floor();
        let f = d - i;
        let a = self.buf[(self.pos + n - i as usize) % n];
        let b = self.buf[(self.pos + n - i as usize - 1) % n];
        a + (b - a) * f
    }
}

/// Two-tap granular pitch shifter. Fine for ±12 semitones on speech, with the usual warble at the extremes.
struct PitchShifter {
    line: DelayLine,
    phase: f32,
}

impl PitchShifter {
    fn new() -> Self {
        PitchShifter {
            line: DelayLine::new(WIN * 2 + 8),
            phase: 0.0,
        }
    }
    #[inline]
    fn process(&mut self, x: f32, ratio: f32) -> f32 {
        self.line.push(x);
        let w = WIN as f32;
        self.phase = (self.phase + (1.0 - ratio) / w).rem_euclid(1.0);
        let p2 = (self.phase + 0.5).rem_euclid(1.0);
        let g1 = (PI * self.phase).sin().powi(2);
        let g2 = (PI * p2).sin().powi(2);
        self.line.tap(self.phase * w + 1.0) * g1 + self.line.tap(p2 * w + 1.0) * g2
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Settings {
    pub on: bool,
    /// Semitones, -12 to 12.
    pub pitch: f32,
    /// 0 to 100.
    pub warmth: f32,
    pub thick: f32,
    pub radio: f32,
    pub robot: f32,
}

pub struct Character {
    pitch: PitchShifter,
    dry: DelayLine,
    chorus: DelayLine,
    lfo1: f32,
    lfo2: f32,
    hp: [Biquad; 2],
    lp: [Biquad; 2],
    ring: f32,
}

impl Character {
    pub fn new() -> Self {
        Character {
            pitch: PitchShifter::new(),
            dry: DelayLine::new(CH_DELAY + 8),
            chorus: DelayLine::new(4096),
            lfo1: 0.0,
            lfo2: 0.25,
            hp: [
                Biquad::new(Coeffs::highpass(SR, 380.0, 0.707)),
                Biquad::new(Coeffs::highpass(SR, 380.0, 0.707)),
            ],
            lp: [
                Biquad::new(Coeffs::lowpass(SR, 3200.0, 0.707)),
                Biquad::new(Coeffs::lowpass(SR, 3200.0, 0.707)),
            ],
            ring: 0.0,
        }
    }

    pub fn process(&mut self, buf: &mut [f32], s: &Settings) {
        let active = s.on
            && (s.pitch.abs() > 0.05
                || s.warmth > 0.5
                || s.thick > 0.5
                || s.radio > 0.5
                || s.robot > 0.5);
        let ratio = 2f32.powf(s.pitch / 12.0);
        let drive = 1.0 + s.warmth / 100.0 * 5.0;
        // level matched at a typical speech peak, so the effect changes colour, not volume
        let match_gain = 0.25 / (drive * 0.25).tanh();
        let warm_mix = (s.warmth / 100.0).clamp(0.0, 1.0);
        let thick = (s.thick / 100.0).clamp(0.0, 1.0);
        let radio = (s.radio / 100.0).clamp(0.0, 1.0);
        let robot = (s.robot / 100.0).clamp(0.0, 1.0);

        for v in buf.iter_mut() {
            let x = *v;
            // the delayed dry signal keeps the latency constant when nothing is active
            self.dry.push(x);
            let dry = self.dry.tap(CH_DELAY as f32 + 1.0);
            let shifted = self.pitch.process(x, if active { ratio } else { 1.0 });
            if !active {
                *v = dry;
                continue;
            }
            let mut y = if s.pitch.abs() > 0.05 { shifted } else { dry };

            // warmth: soft saturation, level-matched
            if warm_mix > 0.0 {
                let sat = (drive * y).tanh() * match_gain;
                y = y * (1.0 - warm_mix) + sat * warm_mix;
            }

            // thickness: two detuned, slowly moving copies
            if thick > 0.0 {
                self.chorus.push(y);
                self.lfo1 = (self.lfo1 + 0.37 / SR).fract();
                self.lfo2 = (self.lfo2 + 0.53 / SR).fract();
                let d1 = (0.012 + 0.003 * (2.0 * PI * self.lfo1).sin()) * SR;
                let d2 = (0.019 + 0.004 * (2.0 * PI * self.lfo2).sin()) * SR;
                let wet = (self.chorus.tap(d1) + self.chorus.tap(d2)) * 0.5;
                y = y * (1.0 - 0.35 * thick) + wet * (0.7 * thick);
            }

            // radio: a narrow band, level-matched by ear
            if radio > 0.0 {
                let mut r = y;
                for f in self.hp.iter_mut() {
                    r = f.process(r);
                }
                for f in self.lp.iter_mut() {
                    r = f.process(r);
                }
                y = y * (1.0 - radio) + r * 1.6 * radio;
            } else {
                // keep the filter state warm and cheap to enter
                let _ = self.hp[0].process(0.0);
            }

            // robot: ring modulation
            if robot > 0.0 {
                self.ring = (self.ring + 70.0 / SR).fract();
                let m = (2.0 * PI * self.ring).sin();
                y = y * (1.0 - robot) + y * m * 1.4 * robot;
            }
            *v = y;
        }
    }
}

impl Default for Character {
    fn default() -> Self {
        Self::new()
    }
}
