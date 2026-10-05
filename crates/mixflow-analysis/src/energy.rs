use crate::spectral::Framer;

pub struct EnergyEstimate {
    /// 0..1 composite, before scaling to the 1-10 display range.
    pub energy_0_1: f32,
    pub energy_1_10: f32,
    pub danceability_0_1: f32,
    pub loudness_rms: f32,
    pub spectral_centroid: f32,
}

/// A deliberately simple, explainable, tunable composite — not a
/// scientifically calibrated loudness/energy model. Each component is
/// normalized over a documented range chosen to be reasonable for
/// commercially mastered dance/pop music; retune these ranges once we
/// have real libraries to validate against, same philosophy as the
/// recommendation scoring weights.
pub fn estimate_energy(
    samples: &[f32],
    sample_rate: u32,
    onset_density: f32,
    bpm: f32,
    bpm_confidence: f32,
) -> EnergyEstimate {
    let loudness_rms = rms_loudness(samples);
    let spectral_centroid = average_spectral_centroid(samples, sample_rate);

    let loudness_component = (loudness_rms / 0.3).clamp(0.0, 1.0);
    let centroid_component = ((spectral_centroid - 500.0) / 5500.0).clamp(0.0, 1.0);
    let onset_component = (onset_density / 8.0).clamp(0.0, 1.0);
    let tempo_component = ((bpm - 60.0) / 140.0).clamp(0.0, 1.0);

    let energy_0_1 = (0.40 * loudness_component
        + 0.25 * centroid_component
        + 0.25 * onset_component
        + 0.10 * tempo_component)
        .clamp(0.0, 1.0);

    let bpm_in_range = 1.0 - ((bpm.clamp(60.0, 180.0) - 120.0).abs() / 60.0).clamp(0.0, 1.0);
    let danceability_0_1 = (0.5 * bpm_in_range + 0.5 * bpm_confidence).clamp(0.0, 1.0);

    EnergyEstimate {
        energy_0_1,
        energy_1_10: 1.0 + energy_0_1 * 9.0,
        danceability_0_1,
        loudness_rms,
        spectral_centroid,
    }
}

fn rms_loudness(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

fn average_spectral_centroid(samples: &[f32], sample_rate: u32) -> f32 {
    let frame_size = 2048usize;
    let hop_size = 1024usize;
    let framer = Framer::new(frame_size, hop_size);
    let n = framer.num_frames(samples.len());
    if n == 0 {
        return 0.0;
    }

    let mut total = 0.0;
    let mut counted = 0usize;
    for i in 0..n {
        let start = i * hop_size;
        let spectrum = framer.magnitude_spectrum(samples, start);
        let mut weighted_sum = 0.0;
        let mut magnitude_sum = 0.0;
        for (bin, &magnitude) in spectrum.iter().enumerate() {
            let freq = bin as f32 * sample_rate as f32 / frame_size as f32;
            weighted_sum += freq * magnitude;
            magnitude_sum += magnitude;
        }
        if magnitude_sum > 0.0 {
            total += weighted_sum / magnitude_sum;
            counted += 1;
        }
    }
    if counted == 0 {
        0.0
    } else {
        total / counted as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn louder_brighter_busier_signal_scores_higher_energy() {
        let sample_rate = 44100;
        let n = (4.0 * sample_rate as f32) as usize;

        // Quiet, low, static tone.
        let quiet: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                0.05 * (2.0 * std::f32::consts::PI * 150.0 * t).sin()
            })
            .collect();

        // Loud, bright, busy signal (high-frequency content + amplitude bursts).
        let loud: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let burst = if (i / 2000) % 2 == 0 { 1.0 } else { 0.3 };
                0.8 * burst * (2.0 * std::f32::consts::PI * 3000.0 * t).sin()
            })
            .collect();

        let quiet_energy = estimate_energy(&quiet, sample_rate, 1.0, 100.0, 0.5);
        let loud_energy = estimate_energy(&loud, sample_rate, 6.0, 128.0, 0.8);

        assert!(
            loud_energy.energy_0_1 > quiet_energy.energy_0_1,
            "expected loud ({}) > quiet ({})",
            loud_energy.energy_0_1,
            quiet_energy.energy_0_1
        );
    }
}
