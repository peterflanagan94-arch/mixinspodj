use crate::spectral::Framer;

pub struct OnsetAnalysis {
    /// Spectral-flux onset detection function, one value per frame.
    pub odf: Vec<f32>,
    pub frame_rate: f32,
    pub onset_density: f32,
}

/// Spectral-flux onset detection function: for each frame, the sum of
/// positive (half-wave-rectified) increases in magnitude spectrum versus
/// the previous frame. Peaks in this signal correspond to note/drum onsets.
pub fn analyze_onsets(
    samples: &[f32],
    sample_rate: u32,
    frame_size: usize,
    hop_size: usize,
) -> OnsetAnalysis {
    let framer = Framer::new(frame_size, hop_size);
    let n = framer.num_frames(samples.len());

    let mut odf = Vec::with_capacity(n);
    let mut prev_spectrum: Option<Vec<f32>> = None;

    for i in 0..n {
        let start = i * hop_size;
        let spectrum = framer.magnitude_spectrum(samples, start);
        let flux = match &prev_spectrum {
            None => 0.0,
            Some(prev) => spectrum
                .iter()
                .zip(prev.iter())
                .map(|(cur, prev)| (cur - prev).max(0.0))
                .sum(),
        };
        odf.push(flux);
        prev_spectrum = Some(spectrum);
    }

    let frame_rate = sample_rate as f32 / hop_size as f32;
    let onset_count = count_peaks(&odf);
    let duration_secs = samples.len() as f32 / sample_rate as f32;
    let onset_density = if duration_secs > 0.0 {
        onset_count as f32 / duration_secs
    } else {
        0.0
    };

    OnsetAnalysis {
        odf,
        frame_rate,
        onset_density,
    }
}

/// Adaptive-threshold local-maxima peak picking: a frame counts as an
/// onset if it's a local max and clears mean + 1 standard deviation.
fn count_peaks(odf: &[f32]) -> usize {
    if odf.len() < 3 {
        return 0;
    }
    let mean = odf.iter().sum::<f32>() / odf.len() as f32;
    let variance = odf.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / odf.len() as f32;
    let threshold = mean + variance.sqrt();

    let mut count = 0;
    for i in 1..odf.len() - 1 {
        if odf[i] > threshold && odf[i] >= odf[i - 1] && odf[i] >= odf[i + 1] {
            count += 1;
        }
    }
    count
}
