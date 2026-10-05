//! Decode → BPM / key / energy analysis pipeline, run on a background worker
//! pool so it never blocks the UI.
//!
//! Not implemented yet — this is the target of milestone M2. Planned deps:
//! `symphonia` (decode), `rubato` (resample), `rustfft`/`realfft` (chroma +
//! onset detection), `lofty` (tag/artwork reading, belongs here or in a
//! sibling `mixflow-metadata` module once scanning lands in M1).

pub struct AnalysisResult {
    pub bpm: f32,
    pub bpm_confidence: f32,
    pub camelot_key: String,
    pub energy: f32,
}
