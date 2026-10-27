//! Short-term loudness: K-weighting (BS.1770, 48 kHz) over a 3 second window of mean squares.
use crate::biquad::{Biquad, Coeffs};

pub struct Loudness {
    shelf: Biquad,
    rlb: Biquad,
    ring: Vec<f32>,
    pos: usize,
    filled: usize,
}

impl Loudness {
    /// `frames` is the window length in blocks, e.g. 300 blocks of 10 ms.
    pub fn new(frames: usize) -> Self {
        let shelf = Coeffs {
            b0: 1.535_124_9,
            b1: -2.691_696_2,
            b2: 1.198_392_8,
            a1: -1.690_659_3,
            a2: 0.732_480_8,
        };
        let rlb = Coeffs {
            b0: 1.0,
            b1: -2.0,
            b2: 1.0,
            a1: -1.990_047_5,
            a2: 0.990_072_25,
        };
        Loudness {
            shelf: Biquad::new(shelf),
            rlb: Biquad::new(rlb),
            ring: vec![0.0; frames],
            pos: 0,
            filled: 0,
        }
    }

    /// Feeds one block and returns the short-term loudness in LUFS (-70 when silent).
    pub fn push(&mut self, block: &[f32]) -> f32 {
        let mut ms = 0.0f32;
        for &x in block {
            let y = self.rlb.process(self.shelf.process(x));
            ms += y * y;
        }
        ms /= block.len().max(1) as f32;
        self.ring[self.pos] = ms;
        self.pos = (self.pos + 1) % self.ring.len();
        self.filled = (self.filled + 1).min(self.ring.len());
        let mean = self.ring.iter().sum::<f32>() / self.filled as f32;
        if mean <= 1e-10 {
            -70.0
        } else {
            (-0.691 + 10.0 * mean.log10()).max(-70.0)
        }
    }
}
