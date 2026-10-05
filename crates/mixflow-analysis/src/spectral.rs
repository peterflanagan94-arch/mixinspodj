use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::sync::Arc;

/// Fixed-size windowed FFT framing shared by onset, key, and energy
/// analysis so they all frame audio the same way.
pub struct Framer {
    frame_size: usize,
    hop_size: usize,
    window: Vec<f32>,
    fft: Arc<dyn Fft<f32>>,
}

impl Framer {
    pub fn new(frame_size: usize, hop_size: usize) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(frame_size);
        Self {
            frame_size,
            hop_size,
            window: hann_window(frame_size),
            fft,
        }
    }

    pub fn num_frames(&self, num_samples: usize) -> usize {
        if num_samples < self.frame_size {
            0
        } else {
            (num_samples - self.frame_size) / self.hop_size + 1
        }
    }

    /// Magnitude spectrum (bins `0..frame_size/2`) for the frame starting
    /// at sample index `start`.
    pub fn magnitude_spectrum(&self, samples: &[f32], start: usize) -> Vec<f32> {
        let mut buf: Vec<Complex<f32>> = (0..self.frame_size)
            .map(|i| Complex::new(samples[start + i] * self.window[i], 0.0))
            .collect();
        self.fft.process(&mut buf);
        buf[..self.frame_size / 2].iter().map(|c| c.norm()).collect()
    }
}

fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (size as f32 - 1.0)).cos())
        .collect()
}
