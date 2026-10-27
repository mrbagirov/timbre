//! Second-order sections from the RBJ cookbook.
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

fn norm(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Coeffs {
    Coeffs {
        b0: (b0 / a0) as f32,
        b1: (b1 / a0) as f32,
        b2: (b2 / a0) as f32,
        a1: (a1 / a0) as f32,
        a2: (a2 / a0) as f32,
    }
}

impl Coeffs {
    pub fn identity() -> Self {
        Coeffs {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }

    pub fn highpass(sr: f32, f: f32, q: f32) -> Self {
        let w = 2.0 * PI * f as f64 / sr as f64;
        let (c, s) = (w.cos(), w.sin());
        let al = s / (2.0 * q as f64);
        norm(
            (1.0 + c) / 2.0,
            -(1.0 + c),
            (1.0 + c) / 2.0,
            1.0 + al,
            -2.0 * c,
            1.0 - al,
        )
    }

    pub fn lowpass(sr: f32, f: f32, q: f32) -> Self {
        let w = 2.0 * PI * f as f64 / sr as f64;
        let (c, s) = (w.cos(), w.sin());
        let al = s / (2.0 * q as f64);
        norm(
            (1.0 - c) / 2.0,
            1.0 - c,
            (1.0 - c) / 2.0,
            1.0 + al,
            -2.0 * c,
            1.0 - al,
        )
    }

    pub fn low_shelf(sr: f32, f: f32, gain_db: f32, slope: f32) -> Self {
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let w = 2.0 * PI * f as f64 / sr as f64;
        let (c, s) = (w.cos(), w.sin());
        let al = s / 2.0 * ((a + 1.0 / a) * (1.0 / slope as f64 - 1.0) + 2.0).sqrt();
        let t = 2.0 * a.sqrt() * al;
        norm(
            a * ((a + 1.0) - (a - 1.0) * c + t),
            2.0 * a * ((a - 1.0) - (a + 1.0) * c),
            a * ((a + 1.0) - (a - 1.0) * c - t),
            (a + 1.0) + (a - 1.0) * c + t,
            -2.0 * ((a - 1.0) + (a + 1.0) * c),
            (a + 1.0) + (a - 1.0) * c - t,
        )
    }

    pub fn high_shelf(sr: f32, f: f32, gain_db: f32, slope: f32) -> Self {
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let w = 2.0 * PI * f as f64 / sr as f64;
        let (c, s) = (w.cos(), w.sin());
        let al = s / 2.0 * ((a + 1.0 / a) * (1.0 / slope as f64 - 1.0) + 2.0).sqrt();
        let t = 2.0 * a.sqrt() * al;
        norm(
            a * ((a + 1.0) + (a - 1.0) * c + t),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
            a * ((a + 1.0) + (a - 1.0) * c - t),
            (a + 1.0) - (a - 1.0) * c + t,
            2.0 * ((a - 1.0) - (a + 1.0) * c),
            (a + 1.0) - (a - 1.0) * c - t,
        )
    }

    pub fn peaking(sr: f32, f: f32, gain_db: f32, q: f32) -> Self {
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let w = 2.0 * PI * f as f64 / sr as f64;
        let (c, s) = (w.cos(), w.sin());
        let al = s / (2.0 * q as f64);
        norm(
            1.0 + al * a,
            -2.0 * c,
            1.0 - al * a,
            1.0 + al / a,
            -2.0 * c,
            1.0 - al / a,
        )
    }

    /// Band-pass with 0 dB peak gain.
    pub fn bandpass(sr: f32, f: f32, q: f32) -> Self {
        let w = 2.0 * PI * f as f64 / sr as f64;
        let (c, s) = (w.cos(), w.sin());
        let al = s / (2.0 * q as f64);
        norm(al, 0.0, -al, 1.0 + al, -2.0 * c, 1.0 - al)
    }

    /// Magnitude response in dB at frequency `f`.
    pub fn magnitude_db(&self, sr: f32, f: f32) -> f32 {
        let w = 2.0 * PI * f as f64 / sr as f64;
        let (c1, s1, c2, s2) = (w.cos(), w.sin(), (2.0 * w).cos(), (2.0 * w).sin());
        let (b0, b1, b2, a1, a2) = (
            self.b0 as f64,
            self.b1 as f64,
            self.b2 as f64,
            self.a1 as f64,
            self.a2 as f64,
        );
        let nr = b0 + b1 * c1 + b2 * c2;
        let ni = -(b1 * s1 + b2 * s2);
        let dr = 1.0 + a1 * c1 + a2 * c2;
        let di = -(a1 * s1 + a2 * s2);
        let mag = ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt();
        (20.0 * mag.max(1e-12).log10()) as f32
    }
}

/// Transposed direct form II.
#[derive(Clone, Debug)]
pub struct Biquad {
    c: Coeffs,
    z1: f32,
    z2: f32,
}

impl Biquad {
    pub fn new(c: Coeffs) -> Self {
        Biquad {
            c,
            z1: 0.0,
            z2: 0.0,
        }
    }
    pub fn set(&mut self, c: Coeffs) {
        self.c = c;
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.c.b0 * x + self.z1;
        self.z1 = self.c.b1 * x - self.c.a1 * y + self.z2;
        self.z2 = self.c.b2 * x - self.c.a2 * y;
        y
    }
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}
