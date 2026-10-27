//! Gate, compressor and limiter. All work on 48 kHz mono blocks.

#[inline]
pub fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}
#[inline]
pub fn lin_to_db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}
#[inline]
fn coef(sr: f32, seconds: f32) -> f32 {
    1.0 - (-1.0 / (seconds * sr)).exp()
}

pub fn peak_db(buf: &[f32]) -> f32 {
    let p = buf.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    lin_to_db(p).max(-90.0)
}

/// Downward gate with hysteresis, hold and smoothed gain.
pub struct Gate {
    a_attack: f32,
    a_release: f32,
    gain: f32,
    open: bool,
    hold: u32,
    hold_frames: u32,
}

const GATE_CLOSED_GAIN: f32 = 0.0056; // -45 dB

impl Gate {
    pub fn new(sr: f32) -> Self {
        Gate {
            a_attack: coef(sr, 0.003),
            a_release: coef(sr, 0.12),
            gain: 1.0,
            open: true,
            hold: 0,
            hold_frames: 6,
        }
    }

    /// Returns whether the gate is open at the end of the block.
    pub fn process(&mut self, buf: &mut [f32], threshold_db: f32, enabled: bool) -> bool {
        if !enabled {
            self.open = true;
            self.gain = 1.0;
            return true;
        }
        let level = peak_db(buf);
        if level > threshold_db {
            self.open = true;
            self.hold = self.hold_frames;
        } else if self.open && level < threshold_db - 3.0 {
            if self.hold > 0 {
                self.hold -= 1;
            } else {
                self.open = false;
            }
        }
        let target = if self.open { 1.0 } else { GATE_CLOSED_GAIN };
        for s in buf.iter_mut() {
            let a = if target > self.gain {
                self.a_attack
            } else {
                self.a_release
            };
            self.gain += (target - self.gain) * a;
            *s *= self.gain;
        }
        self.open
    }
}

/// Feed-forward compressor with a soft knee and a smoothed gain reduction.
pub struct Compressor {
    a_attack: f32,
    a_release: f32,
    gr: f32,
}

const KNEE_DB: f32 = 6.0;

impl Compressor {
    pub fn new(sr: f32) -> Self {
        Compressor {
            a_attack: coef(sr, 0.006),
            a_release: coef(sr, 0.10),
            gr: 0.0,
        }
    }

    /// Returns the mean gain reduction in dB over the block.
    pub fn process(
        &mut self,
        buf: &mut [f32],
        threshold_db: f32,
        ratio: f32,
        enabled: bool,
    ) -> f32 {
        if !enabled {
            self.gr += (0.0 - self.gr) * self.a_release * 8.0;
            return 0.0;
        }
        let ratio = ratio.max(1.0);
        let makeup = ((-threshold_db) * (1.0 - 1.0 / ratio) * 0.4).max(0.0);
        let mut sum = 0.0;
        for s in buf.iter_mut() {
            let l = lin_to_db(s.abs());
            let over = l - threshold_db;
            let out_l = if over <= -KNEE_DB / 2.0 {
                l
            } else if over >= KNEE_DB / 2.0 {
                threshold_db + over / ratio
            } else {
                let t = over + KNEE_DB / 2.0;
                l + (1.0 / ratio - 1.0) * t * t / (2.0 * KNEE_DB)
            };
            let target = (l - out_l).max(0.0);
            let a = if target > self.gr {
                self.a_attack
            } else {
                self.a_release
            };
            self.gr += (target - self.gr) * a;
            *s *= db_to_lin(-self.gr + makeup);
            sum += self.gr;
        }
        sum / buf.len() as f32
    }
}

/// Look-ahead peak limiter. Always adds `LOOKAHEAD` samples of delay, enabled or not.
pub const LOOKAHEAD: usize = 72;

pub struct Limiter {
    buf: [f32; LOOKAHEAD],
    abs: [f32; LOOKAHEAD],
    pos: usize,
    g: f32,
    a_attack: f32,
    a_release: f32,
}

impl Limiter {
    pub fn new(sr: f32) -> Self {
        Limiter {
            buf: [0.0; LOOKAHEAD],
            abs: [0.0; LOOKAHEAD],
            pos: 0,
            g: 1.0,
            a_attack: 1.0 - (-4.0 / LOOKAHEAD as f32).exp(),
            a_release: coef(sr, 0.06),
        }
    }

    /// Returns the deepest gain reduction in dB over the block.
    pub fn process(&mut self, buf: &mut [f32], ceiling_db: f32, enabled: bool) -> f32 {
        let ceil = if enabled { db_to_lin(ceiling_db) } else { 1.0 };
        let mut min_g = 1.0f32;
        for s in buf.iter_mut() {
            let x = *s;
            let out_x = self.buf[self.pos];
            self.buf[self.pos] = x;
            self.abs[self.pos] = x.abs();
            self.pos = (self.pos + 1) % LOOKAHEAD;
            let peak = self.abs.iter().fold(0.0f32, |m, v| m.max(*v));
            let target = if peak > ceil { ceil / peak } else { 1.0 };
            let a = if target < self.g {
                self.a_attack
            } else {
                self.a_release
            };
            self.g += (target - self.g) * a;
            min_g = min_g.min(self.g);
            *s = (out_x * self.g).clamp(-ceil, ceil);
        }
        -lin_to_db(min_g)
    }
}
