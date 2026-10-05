//! The MixFlow scoring engine — deterministic, explainable "best next track"
//! recommendations. Pure functions only: no filesystem or network access,
//! so this crate stays independently unit-testable.
//!
//! Not implemented yet — this is the target of milestone M5. Scoring will
//! combine BPM compatibility, harmonic (Camelot) compatibility, energy
//! compatibility, genre compatibility, era compatibility, historical
//! transition frequency, and event-type suitability, each weighted by a
//! configurable `scoring_weights` profile selected by the active mode
//! (Keep Dancing / Build Energy / Lower Energy / Similar Vibe / Change
//! Genre) and crowd state.

pub struct ScoreComponent {
    pub name: String,
    pub value: f32,
    pub explanation: String,
}

pub struct ScoredTrack {
    pub track_id: i64,
    pub total_score_percent: f32,
    pub components: Vec<ScoreComponent>,
    pub badges: Vec<String>,
}
