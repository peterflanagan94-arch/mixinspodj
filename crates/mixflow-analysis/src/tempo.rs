pub struct TempoEstimate {
    pub bpm: f32,
    /// Normalized autocorrelation strength at the chosen lag, in `[0, 1]`.
    pub confidence: f32,
}

const MIN_BPM: f32 = 60.0;
const MAX_BPM: f32 = 200.0;
const DANCE_RANGE: (f32, f32) = (80.0, 160.0);

/// Estimates tempo via autocorrelation of the onset detection function —
/// the lag with the strongest periodicity is the beat period. Octave
/// errors (detecting double/half the true tempo) are corrected by
/// preferring whichever of {bpm, bpm*2, bpm/2} that falls in the common
/// dance-music range and has the strongest autocorrelation there.
pub fn estimate_tempo(odf: &[f32], frame_rate: f32) -> TempoEstimate {
    if odf.len() < 4 || frame_rate <= 0.0 {
        return TempoEstimate { bpm: 0.0, confidence: 0.0 };
    }

    let mean = odf.iter().sum::<f32>() / odf.len() as f32;
    let centered: Vec<f32> = odf.iter().map(|v| v - mean).collect();

    let min_lag = (frame_rate * 60.0 / MAX_BPM).round().max(1.0) as usize;
    let max_lag = ((frame_rate * 60.0 / MIN_BPM).round() as usize).min(centered.len().saturating_sub(1));
    if min_lag >= max_lag {
        return TempoEstimate { bpm: 0.0, confidence: 0.0 };
    }

    let energy = autocorrelation_at(&centered, 0).max(1e-9);

    let mut best_lag = min_lag;
    let mut best_value = f32::MIN;
    for lag in min_lag..=max_lag {
        let value = autocorrelation_at(&centered, lag);
        if value > best_value {
            best_value = value;
            best_lag = lag;
        }
    }

    let raw_bpm = frame_rate * 60.0 / best_lag as f32;
    let raw_confidence = (best_value / energy).clamp(0.0, 1.0);

    let mut best_in_range: Option<(f32, f32)> = None;
    for &candidate_bpm in &[raw_bpm, raw_bpm * 2.0, raw_bpm / 2.0] {
        if candidate_bpm < DANCE_RANGE.0 || candidate_bpm > DANCE_RANGE.1 {
            continue;
        }
        let lag = (frame_rate * 60.0 / candidate_bpm).round() as usize;
        if lag == 0 || lag >= centered.len() {
            continue;
        }
        let value = (autocorrelation_at(&centered, lag) / energy).clamp(0.0, 1.0);
        if best_in_range.map_or(true, |(_, best_value)| value > best_value) {
            best_in_range = Some((candidate_bpm, value));
        }
    }

    let (bpm, confidence) = best_in_range.unwrap_or((raw_bpm, raw_confidence));
    TempoEstimate { bpm, confidence }
}

fn autocorrelation_at(centered: &[f32], lag: usize) -> f32 {
    if lag >= centered.len() {
        return 0.0;
    }
    centered[..centered.len() - lag]
        .iter()
        .zip(&centered[lag..])
        .map(|(a, b)| a * b)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_click_track(bpm: f32, seconds: f32, sample_rate: u32) -> Vec<f32> {
        let n = (seconds * sample_rate as f32) as usize;
        let mut samples = vec![0.0f32; n];
        let beat_interval = 60.0 / bpm;
        let click_len = (0.04 * sample_rate as f32) as usize;

        let mut beat = 0;
        loop {
            let start = (beat as f32 * beat_interval * sample_rate as f32) as usize;
            if start >= n {
                break;
            }
            for i in 0..click_len {
                if start + i >= n {
                    break;
                }
                let t = i as f32 / sample_rate as f32;
                let envelope = 1.0 - (i as f32 / click_len as f32);
                samples[start + i] += envelope * (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
            }
            beat += 1;
        }
        samples
    }

    #[test]
    fn detects_approximately_correct_bpm_in_dance_range() {
        let sample_rate = 44100;
        let samples = synth_click_track(128.0, 10.0, sample_rate);

        let onsets = crate::onset::analyze_onsets(&samples, sample_rate, 1024, 512);
        let tempo = estimate_tempo(&onsets.odf, onsets.frame_rate);

        assert!(
            (tempo.bpm - 128.0).abs() < 4.0,
            "expected ~128 BPM, got {}",
            tempo.bpm
        );
        assert!(tempo.confidence > 0.1, "confidence too low: {}", tempo.confidence);
    }

    #[test]
    fn detects_a_slower_tempo_too() {
        let sample_rate = 44100;
        let samples = synth_click_track(90.0, 10.0, sample_rate);

        let onsets = crate::onset::analyze_onsets(&samples, sample_rate, 1024, 512);
        let tempo = estimate_tempo(&onsets.odf, onsets.frame_rate);

        assert!(
            (tempo.bpm - 90.0).abs() < 4.0,
            "expected ~90 BPM, got {}",
            tempo.bpm
        );
    }
}
