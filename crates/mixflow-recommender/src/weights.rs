use crate::Mode;

/// Per-mode component weights — deliberately plain constants, not a
/// database table, for v1: "configurable internally so they can be
/// tuned during testing" per the original design brief. Promoting these
/// to `scoring_weights` (already in the schema) is a natural follow-up
/// once there's real usage data to tune against.
#[derive(Debug, Clone, Copy)]
pub struct WeightProfile {
    pub bpm: f32,
    pub harmonic: f32,
    pub energy: f32,
    pub genre: f32,
    pub era: f32,
}

pub fn weights_for(mode: Mode) -> WeightProfile {
    match mode {
        Mode::Keep => WeightProfile {
            bpm: 0.35,
            harmonic: 0.30,
            energy: 0.20,
            genre: 0.10,
            era: 0.05,
        },
        Mode::Build | Mode::Cool => WeightProfile {
            bpm: 0.30,
            harmonic: 0.25,
            energy: 0.30,
            genre: 0.10,
            era: 0.05,
        },
        Mode::Switch => WeightProfile {
            bpm: 0.25,
            harmonic: 0.20,
            energy: 0.15,
            genre: 0.35,
            era: 0.05,
        },
        Mode::Rescue => WeightProfile {
            bpm: 0.20,
            harmonic: 0.15,
            energy: 0.40,
            genre: 0.15,
            era: 0.10,
        },
    }
}
