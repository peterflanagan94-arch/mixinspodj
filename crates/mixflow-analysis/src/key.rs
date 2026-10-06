use crate::spectral::Framer;

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Camelot wheel number for the MAJOR key rooted at pitch class `i` (0=C).
/// The minor key sharing the same number is the relative minor, at
/// pitch class `(i + 3) % 12` from the corresponding major root — so a
/// minor key's number is looked up via this same table at its relative
/// major's pitch class.
const MAJOR_CAMELOT_NUMBER: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];

/// Krumhansl-Kessler (1982) key profiles — standard published probe-tone
/// data used throughout music-information-retrieval research for
/// correlation-based key finding. Index 0 is the tonic.
const KRUMHANSL_MAJOR: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const KRUMHANSL_MINOR: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

pub struct KeyEstimate {
    pub pitch_class: usize,
    pub is_major: bool,
    pub key_name: String,
    pub camelot: String,
    pub correlation: f32,
}

const MIN_FREQ: f32 = 80.0;
const MAX_FREQ: f32 = 5000.0;

/// Builds a 12-bin chroma profile for the whole track (energy-weighted
/// average across frames) and correlates it against all 24 rotated
/// major/minor key profiles, picking the best match.
pub fn estimate_key(samples: &[f32], sample_rate: u32) -> KeyEstimate {
    let frame_size = 4096usize;
    let hop_size = 2048usize;
    let framer = Framer::new(frame_size, hop_size);
    let n = framer.num_frames(samples.len());

    let mut chroma = [0f32; 12];
    for i in 0..n {
        let start = i * hop_size;
        let spectrum = framer.magnitude_spectrum(samples, start);
        for (bin, &magnitude) in spectrum.iter().enumerate() {
            if bin == 0 {
                continue;
            }
            let freq = bin as f32 * sample_rate as f32 / frame_size as f32;
            if freq < MIN_FREQ || freq > MAX_FREQ {
                continue;
            }
            let midi = 69.0 + 12.0 * (freq / 440.0).log2();
            let pitch_class = (midi.round() as i32).rem_euclid(12) as usize;
            chroma[pitch_class] += magnitude;
        }
    }

    let total: f32 = chroma.iter().sum();
    if total > 0.0 {
        for value in chroma.iter_mut() {
            *value /= total;
        }
    }

    let mut best = (0usize, true, f32::MIN);
    for root in 0..12 {
        let major_corr = correlate(&chroma, &rotate(&KRUMHANSL_MAJOR, root));
        if major_corr > best.2 {
            best = (root, true, major_corr);
        }
        let minor_corr = correlate(&chroma, &rotate(&KRUMHANSL_MINOR, root));
        if minor_corr > best.2 {
            best = (root, false, minor_corr);
        }
    }

    let (pitch_class, is_major, correlation) = best;
    let (key_name, camelot) = key_for(pitch_class, is_major);

    KeyEstimate {
        pitch_class,
        is_major,
        key_name,
        camelot,
        correlation,
    }
}

/// Names a key the way the rest of the app shows it ("C#m", "B") and gives
/// its Camelot code ("5A", "11B"). `pitch_class` is 0 for C up to 11 for B.
pub fn key_for(pitch_class: usize, is_major: bool) -> (String, String) {
    let key_name = format!("{}{}", NOTE_NAMES[pitch_class], if is_major { "" } else { "m" });
    let number = if is_major {
        MAJOR_CAMELOT_NUMBER[pitch_class]
    } else {
        MAJOR_CAMELOT_NUMBER[(pitch_class + 3) % 12]
    };
    let camelot = format!("{}{}", number, if is_major { "B" } else { "A" });
    (key_name, camelot)
}

fn rotate(profile: &[f32; 12], root: usize) -> [f32; 12] {
    let mut out = [0f32; 12];
    for (i, value) in profile.iter().enumerate() {
        out[(i + root) % 12] = *value;
    }
    out
}

fn correlate(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let mean_a = a.iter().sum::<f32>() / 12.0;
    let mean_b = b.iter().sum::<f32>() / 12.0;
    let mut numerator = 0.0;
    let mut da = 0.0;
    let mut db = 0.0;
    for i in 0..12 {
        let xa = a[i] - mean_a;
        let xb = b[i] - mean_b;
        numerator += xa * xb;
        da += xa * xa;
        db += xb * xb;
    }
    if da <= 0.0 || db <= 0.0 {
        0.0
    } else {
        numerator / (da.sqrt() * db.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_chord(freqs: &[f32], seconds: f32, sample_rate: u32) -> Vec<f32> {
        let n = (seconds * sample_rate as f32) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                freqs
                    .iter()
                    .map(|f| (2.0 * std::f32::consts::PI * f * t).sin())
                    .sum::<f32>()
                    / freqs.len() as f32
            })
            .collect()
    }

    #[test]
    fn detects_c_major_from_its_triad() {
        let sample_rate = 44100;
        // C4, E4, G4
        let samples = synth_chord(&[261.63, 329.63, 392.00], 4.0, sample_rate);
        let estimate = estimate_key(&samples, sample_rate);

        assert_eq!(estimate.pitch_class, 0, "expected pitch class C (0)");
        assert!(estimate.is_major, "expected major mode");
        assert_eq!(estimate.key_name, "C");
        assert_eq!(estimate.camelot, "8B");
    }

    #[test]
    fn detects_a_minor_from_its_triad() {
        let sample_rate = 44100;
        // A3, C4, E4
        let samples = synth_chord(&[220.00, 261.63, 329.63], 4.0, sample_rate);
        let estimate = estimate_key(&samples, sample_rate);

        assert_eq!(estimate.pitch_class, 9, "expected pitch class A (9)");
        assert!(!estimate.is_major, "expected minor mode");
        assert_eq!(estimate.key_name, "Am");
        assert_eq!(estimate.camelot, "8A");
    }

    #[test]
    fn camelot_wheel_relative_major_minor_share_a_number() {
        // C major and A minor are relative keys and must share a number
        // (8B / 8A) per the standard Camelot wheel.
        let c_major_number = MAJOR_CAMELOT_NUMBER[0];
        let a_minor_number = MAJOR_CAMELOT_NUMBER[(9 + 3) % 12];
        assert_eq!(c_major_number, a_minor_number);
    }
}
