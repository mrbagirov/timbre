//! Voice chain for a microphone, 48 kHz mono, processed in blocks of 480 samples (10 ms).
//! Order: gate, noise suppression, EQ, compressor, limiter. Latency is constant whatever is switched on.

pub mod biquad;
pub mod dynamics;
pub mod loudness;
pub mod resample;

use biquad::{Biquad, Coeffs};
use dynamics::{db_to_lin, lin_to_db, peak_db, Compressor, Gate, Limiter, LOOKAHEAD};
use loudness::Loudness;
use nnnoiseless::DenoiseState;
use serde::Deserialize;

pub const SAMPLE_RATE: u32 = 48_000;
pub const FRAME: usize = DenoiseState::FRAME_SIZE; // 480
/// Samples between input and the output of the chain.
pub const NR_DELAY: usize = FRAME;
pub const LATENCY: usize = NR_DELAY + LOOKAHEAD;
const FLOOR: f32 = -90.0;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GateP {
    pub on: bool,
    pub threshold: f32,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct NrP {
    pub on: bool,
    pub mode: String,
    pub amount: f32,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct EqP {
    pub on: bool,
    pub lowcut: bool,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CompP {
    pub on: bool,
    pub threshold: f32,
    pub ratio: f32,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LimP {
    pub on: bool,
    pub ceiling: f32,
}

/// The same shape the interface keeps in `Store.state.params`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Params {
    pub gate: GateP,
    pub nr: NrP,
    pub eq: EqP,
    pub comp: CompP,
    pub lim: LimP,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            gate: GateP {
                on: true,
                threshold: -50.0,
            },
            nr: NrP {
                on: true,
                mode: "light".into(),
                amount: 60.0,
            },
            eq: EqP {
                on: true,
                lowcut: true,
                low: 0.0,
                mid: 1.0,
                high: 2.0,
            },
            comp: CompP {
                on: true,
                threshold: -22.0,
                ratio: 3.0,
            },
            lim: LimP {
                on: true,
                ceiling: -1.0,
            },
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Flags {
    pub bypass: bool,
}

/// What one block tells the interface. Levels are peak dB, -90 is silence.
#[derive(Clone, Debug, Default)]
pub struct FrameMeters {
    pub raw: f32,
    pub gate: f32,
    pub nr: f32,
    pub comp: f32,
    pub lim: f32,
    pub gate_open: bool,
    pub nr_reduction: f32,
    pub gr: f32,
    pub lim_cut: f32,
    pub loud: f32,
    /// Mean-square level in dB of three bands of the input: low, mid, high.
    pub bands: [f32; 3],
}

struct Ring {
    buf: Vec<f32>,
    pos: usize,
}
impl Ring {
    fn new(n: usize) -> Self {
        Ring {
            buf: vec![0.0; n],
            pos: 0,
        }
    }
    /// Pushes a block and returns the same block delayed by the ring length.
    fn delay(&mut self, block: &[f32], out: &mut [f32]) {
        for (i, &x) in block.iter().enumerate() {
            out[i] = self.buf[self.pos];
            self.buf[self.pos] = x;
            self.pos = (self.pos + 1) % self.buf.len();
        }
    }
}

pub struct Chain {
    sr: f32,
    gate: Gate,
    denoise: Box<DenoiseState<'static>>,
    dry_delay: Ring,
    hp: Biquad,
    low: Biquad,
    mid: Biquad,
    high: Biquad,
    eq_cache: Option<EqP>,
    comp: Compressor,
    lim: Limiter,
    raw_delay: Ring,
    loud: Loudness,
    band_filters: [Biquad; 3],
    nr_red: f32,
    first_nr: bool,
}

impl Chain {
    pub fn new() -> Self {
        let sr = SAMPLE_RATE as f32;
        Chain {
            sr,
            gate: Gate::new(sr),
            denoise: DenoiseState::new(),
            dry_delay: Ring::new(NR_DELAY),
            hp: Biquad::new(Coeffs::identity()),
            low: Biquad::new(Coeffs::identity()),
            mid: Biquad::new(Coeffs::identity()),
            high: Biquad::new(Coeffs::identity()),
            eq_cache: None,
            comp: Compressor::new(sr),
            lim: Limiter::new(sr),
            raw_delay: Ring::new(LATENCY),
            loud: Loudness::new(300),
            band_filters: [
                Biquad::new(Coeffs::bandpass(sr, 160.0, 1.0)),
                Biquad::new(Coeffs::bandpass(sr, 1000.0, 0.6)),
                Biquad::new(Coeffs::bandpass(sr, 6000.0, 0.8)),
            ],
            nr_red: 0.0,
            first_nr: true,
        }
    }

    fn update_eq(&mut self, e: &EqP) {
        if self.eq_cache.as_ref() == Some(e) {
            return;
        }
        let sr = self.sr;
        self.hp.set(if e.lowcut {
            Coeffs::highpass(sr, 80.0, 0.707)
        } else {
            Coeffs::identity()
        });
        self.low.set(Coeffs::low_shelf(sr, 160.0, e.low, 1.0));
        self.mid.set(Coeffs::peaking(sr, 1200.0, e.mid, 0.9));
        self.high.set(Coeffs::high_shelf(sr, 5500.0, e.high, 1.0));
        self.eq_cache = Some(e.clone());
    }

    /// Processes exactly one block of `FRAME` samples.
    pub fn process(
        &mut self,
        input: &[f32],
        out: &mut [f32],
        p: &Params,
        flags: &Flags,
    ) -> FrameMeters {
        assert_eq!(input.len(), FRAME);
        assert_eq!(out.len(), FRAME);
        let mut m = FrameMeters {
            raw: peak_db(input),
            ..Default::default()
        };

        // band levels of the input, for the calibration wizard
        for (k, f) in self.band_filters.iter_mut().enumerate() {
            let mut ms = 0.0f32;
            for &x in input {
                let y = f.process(x);
                ms += y * y;
            }
            m.bands[k] = (10.0 * (ms / FRAME as f32).max(1e-12).log10()).max(FLOOR);
        }

        let mut x = [0.0f32; FRAME];
        x.copy_from_slice(input);

        // gate
        m.gate_open = self.gate.process(&mut x, p.gate.threshold, p.gate.on);
        m.gate = peak_db(&x);

        // noise suppression, always running so the state stays warm and the delay stays constant
        let mut scaled = [0.0f32; FRAME];
        for i in 0..FRAME {
            scaled[i] = x[i] * 32768.0;
        }
        let mut wet = [0.0f32; FRAME];
        self.denoise.process_frame(&mut wet, &scaled);
        if self.first_nr {
            wet.fill(0.0);
            self.first_nr = false;
        }
        let mut dry = [0.0f32; FRAME];
        self.dry_delay.delay(&x, &mut dry);
        let w = if p.nr.on {
            (p.nr.amount / 100.0).clamp(0.0, 1.0) * if p.nr.mode == "hq" { 0.96 } else { 0.72 }
        } else {
            0.0
        };
        let (mut e_dry, mut e_out) = (0.0f32, 0.0f32);
        for i in 0..FRAME {
            let y = dry[i] * (1.0 - w) + (wet[i] / 32768.0) * w;
            e_dry += dry[i] * dry[i];
            e_out += y * y;
            x[i] = y;
        }
        let red = if e_dry > 1e-9 {
            (10.0 * (e_dry / e_out.max(1e-12)).log10()).clamp(0.0, 30.0)
        } else {
            0.0
        };
        self.nr_red += (red - self.nr_red) * 0.1;
        m.nr_reduction = if p.nr.on { self.nr_red } else { 0.0 };
        m.nr = peak_db(&x);

        // eq
        if p.eq.on {
            self.update_eq(&p.eq);
            for s in x.iter_mut() {
                *s = self
                    .high
                    .process(self.mid.process(self.low.process(self.hp.process(*s))));
            }
        }

        // compressor
        m.gr = self
            .comp
            .process(&mut x, p.comp.threshold, p.comp.ratio, p.comp.on);
        m.comp = peak_db(&x);

        // limiter, always in the path for a constant delay
        m.lim_cut = self.lim.process(&mut x, p.lim.ceiling, p.lim.on);
        m.lim = peak_db(&x);

        if flags.bypass {
            let mut r = [0.0f32; FRAME];
            self.raw_delay.delay(input, &mut r);
            out.copy_from_slice(&r);
            m.gate = m.raw;
            m.nr = m.raw;
            m.comp = m.raw;
            m.lim = m.raw;
            m.gr = 0.0;
            m.lim_cut = 0.0;
            m.nr_reduction = 0.0;
            m.gate_open = true;
        } else {
            let mut r = [0.0f32; FRAME];
            self.raw_delay.delay(input, &mut r); // keeps the ring warm for a click-free switch
            out.copy_from_slice(&x);
        }
        m.loud = self.loud.push(out);
        m
    }
}

impl Default for Chain {
    fn default() -> Self {
        Self::new()
    }
}

pub fn lin(db: f32) -> f32 {
    db_to_lin(db)
}
pub fn db(x: f32) -> f32 {
    lin_to_db(x)
}

#[cfg(test)]
mod tests;
