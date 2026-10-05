//! Decode -> BPM / key / energy analysis pipeline.
//!
//! Each stage is a plain, documented, tunable DSP algorithm — no external
//! model, no Essentia (AGPL, incompatible with closed-source distribution),
//! just well-understood classic MIR techniques implemented directly so we
//! control licensing and behavior completely:
//!
//! - **BPM**: spectral-flux onset detection function, autocorrelated to
//!   find the dominant beat period, with octave-error correction toward
//!   the common 80-160 BPM dance range ([`tempo`]).
//! - **Key**: a whole-track chroma vector correlated against the standard
//!   Krumhansl-Kessler major/minor key profiles, mapped to the Camelot
//!   wheel ([`key`]).
//! - **Energy / danceability**: an explainable, tunable composite of
//!   loudness, spectral brightness, onset density, and tempo ([`energy`]).
//!
//! This crate does no filesystem or database I/O beyond reading the one
//! audio file it's asked to decode — callers (the Tauri app) own scheduling
//! this as background work and persisting the result.

mod decode;
mod energy;
mod key;
mod onset;
mod spectral;
mod tempo;

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("io error: {0}")]
    Io(String),
    #[error("decode error: {0}")]
    Decode(String),
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub bpm: f32,
    pub bpm_confidence: f32,
    pub key: String,
    pub camelot_key: String,
    /// 1.0 - 10.0
    pub energy: f32,
    /// 0.0 - 1.0
    pub danceability: f32,
    pub loudness_rms: f32,
    pub spectral_centroid: f32,
    pub onset_density: f32,
}

const ONSET_FRAME_SIZE: usize = 1024;
const ONSET_HOP_SIZE: usize = 512;
const MIN_SAMPLES_TO_ANALYZE: usize = ONSET_FRAME_SIZE * 4;

/// Decodes `path` and runs the full analysis pipeline on it.
pub fn analyze_file(path: &Path) -> Result<AnalysisResult, AnalysisError> {
    let audio = decode::decode_to_mono(path)?;
    analyze_samples(&audio.samples, audio.sample_rate)
}

/// Runs the analysis pipeline on already-decoded mono PCM. Exposed
/// separately from [`analyze_file`] so tests (and future callers that
/// already have decoded audio) don't need a real file on disk.
pub fn analyze_samples(samples: &[f32], sample_rate: u32) -> Result<AnalysisResult, AnalysisError> {
    if samples.len() < MIN_SAMPLES_TO_ANALYZE {
        return Err(AnalysisError::Decode("audio too short to analyze".into()));
    }

    let onsets = onset::analyze_onsets(samples, sample_rate, ONSET_FRAME_SIZE, ONSET_HOP_SIZE);
    let tempo_estimate = tempo::estimate_tempo(&onsets.odf, onsets.frame_rate);
    let key_estimate = key::estimate_key(samples, sample_rate);
    let energy_estimate = energy::estimate_energy(
        samples,
        sample_rate,
        onsets.onset_density,
        tempo_estimate.bpm,
        tempo_estimate.confidence,
    );

    Ok(AnalysisResult {
        bpm: tempo_estimate.bpm,
        bpm_confidence: tempo_estimate.confidence,
        key: key_estimate.key_name,
        camelot_key: key_estimate.camelot,
        energy: energy_estimate.energy_1_10,
        danceability: energy_estimate.danceability_0_1,
        loudness_rms: energy_estimate.loudness_rms,
        spectral_centroid: energy_estimate.spectral_centroid,
        onset_density: onsets.onset_density,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_pipeline_runs_end_to_end_on_a_synthetic_track() {
        let sample_rate = 44100;
        let seconds = 6.0;
        let n = (seconds * sample_rate as f32) as usize;
        let bpm = 124.0;
        let beat_interval = 60.0 / bpm;

        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let beat_phase = (t % beat_interval) / beat_interval;
                let kick = if beat_phase < 0.05 { 1.0 - beat_phase / 0.05 } else { 0.0 };
                let chord = (2.0 * std::f32::consts::PI * 261.63 * t).sin()
                    + (2.0 * std::f32::consts::PI * 329.63 * t).sin()
                    + (2.0 * std::f32::consts::PI * 392.00 * t).sin();
                0.5 * kick + 0.1 * chord
            })
            .collect();

        let result = analyze_samples(&samples, sample_rate).expect("analysis should succeed");

        assert!((result.bpm - bpm).abs() < 5.0, "bpm: {}", result.bpm);
        assert_eq!(result.camelot_key, "8B");
        assert!((1.0..=10.0).contains(&result.energy));
        assert!((0.0..=1.0).contains(&result.danceability));
    }

    #[test]
    fn rejects_audio_that_is_too_short() {
        let samples = vec![0.0f32; 10];
        let result = analyze_samples(&samples, 44100);
        assert!(result.is_err());
    }
}
