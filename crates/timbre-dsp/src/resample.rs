//! Streaming linear resampler. Good enough for speech between 44.1, 48 and 96 kHz.

#[derive(Clone, Debug)]
pub struct Resampler {
    step: f64,
    phase: f64,
    prev: f32,
}

impl Resampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        Resampler {
            step: in_rate as f64 / out_rate as f64,
            phase: 0.0,
            prev: 0.0,
        }
    }

    pub fn is_identity(&self) -> bool {
        (self.step - 1.0).abs() < 1e-9
    }

    /// Appends the resampled `input` to `out`.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if input.is_empty() {
            return;
        }
        if self.is_identity() {
            out.extend_from_slice(input);
            return;
        }
        let n = input.len() as f64;
        // phase counts in input samples; -1 is `prev`
        while self.phase < n - 1.0 {
            let i = self.phase.floor();
            let frac = (self.phase - i) as f32;
            let ii = i as isize;
            let x0 = if ii < 0 {
                self.prev
            } else {
                input[ii as usize]
            };
            let x1 = input[(ii + 1) as usize];
            out.push(x0 + (x1 - x0) * frac);
            self.phase += self.step;
        }
        self.phase -= n;
        self.prev = input[input.len() - 1];
    }
}
