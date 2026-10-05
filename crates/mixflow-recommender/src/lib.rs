//! The MixInspo DJ scoring engine — deterministic, explainable "best next
//! track" recommendations. Pure functions only: no filesystem, database,
//! or network access, so this crate stays independently unit-testable and
//! has no opinion about where its inputs come from.
//!
//! Combines BPM compatibility, harmonic (Camelot) compatibility, energy
//! compatibility, genre compatibility, and era compatibility, each
//! weighted by a per-[`Mode`] profile ([`weights`]). Two components from
//! the original design — historical transition frequency and event-type
//! suitability — are deliberately **not implemented yet**: there's no
//! transition history (that needs M7, play tracking) and no real event
//! data (that needs the PREP module) for them to honestly draw on. Adding
//! fake versions of either would be worse than leaving them out; the
//! architecture (an extra weighted component feeding the same weighted
//! average) is ready for them whenever that data exists.
//!
//! Any component whose inputs are missing (a track that hasn't been
//! analyzed yet, say) is simply left out of that track's score — the
//! remaining components are renormalized over whatever weight *is*
//! present, rather than guessing at a neutral value. A track scored from
//! only 2 of 5 components is flagged with a `PARTIAL_DATA` badge so the
//! caller can show that honestly instead of implying full confidence.

mod bpm;
mod energy;
mod era;
mod genre;
mod harmonic;
mod weights;

pub use weights::{weights_for, WeightProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Keep,
    Build,
    Cool,
    Switch,
    Rescue,
}

impl Mode {
    /// Parses the uppercase mode names the frontend uses ("KEEP",
    /// "BUILD", "COOL", "SWITCH", "RESCUE"). Unrecognized input falls
    /// back to `Keep`, the baseline/no-direction mode.
    pub fn parse(s: &str) -> Mode {
        match s.to_uppercase().as_str() {
            "BUILD" => Mode::Build,
            "COOL" => Mode::Cool,
            "SWITCH" => Mode::Switch,
            "RESCUE" => Mode::Rescue,
            _ => Mode::Keep,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackFeatures {
    pub id: i64,
    pub bpm: Option<f32>,
    pub camelot_key: Option<String>,
    /// 1.0 - 10.0
    pub energy: Option<f32>,
    pub genre: Option<String>,
    pub year: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct ScoreComponent {
    pub name: &'static str,
    pub weight: f32,
    pub value: f32,
    pub explanation: &'static str,
}

#[derive(Debug, Clone)]
pub struct ScoredTrack {
    pub track_id: i64,
    pub score_percent: f32,
    /// Sorted by contribution (weight * value) descending, so the first
    /// entry is always the strongest reason this track was recommended.
    pub components: Vec<ScoreComponent>,
    pub badges: Vec<&'static str>,
}

const TOTAL_POSSIBLE_COMPONENTS: usize = 5;

/// Scores every candidate against `now_playing` under `mode`, returning
/// them sorted best-match-first. Candidates with no usable data at all
/// (every component unscored) still come back — tagged `UNSCORED` — so a
/// caller can choose to show or hide them, rather than silently vanishing.
pub fn score_candidates(now_playing: &TrackFeatures, candidates: &[TrackFeatures], mode: Mode) -> Vec<ScoredTrack> {
    let weights = weights_for(mode);
    let mut scored: Vec<ScoredTrack> = candidates
        .iter()
        .map(|candidate| score_one(now_playing, candidate, mode, &weights))
        .collect();
    scored.sort_by(|a, b| b.score_percent.partial_cmp(&a.score_percent).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

fn score_one(now: &TrackFeatures, candidate: &TrackFeatures, mode: Mode, weights: &WeightProfile) -> ScoredTrack {
    let mut components = Vec::new();
    let mut weighted_sum = 0.0;
    let mut weight_total = 0.0;

    let mut push = |name, weight: f32, result: Option<(f32, &'static str)>| {
        if let Some((value, explanation)) = result {
            components.push(ScoreComponent { name, weight, value, explanation });
            weighted_sum += value * weight;
            weight_total += weight;
        }
    };

    push("BPM", weights.bpm, bpm::score(now.bpm, candidate.bpm));
    push(
        "Key",
        weights.harmonic,
        harmonic::score(now.camelot_key.as_deref(), candidate.camelot_key.as_deref()),
    );
    push("Energy", weights.energy, energy::score(now.energy, candidate.energy, mode));
    push(
        "Genre",
        weights.genre,
        genre::score(now.genre.as_deref(), candidate.genre.as_deref(), mode),
    );
    push("Era", weights.era, era::score(now.year, candidate.year));

    components.sort_by(|a, b| {
        (b.weight * b.value)
            .partial_cmp(&(a.weight * a.value))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let badges = if components.is_empty() {
        vec!["UNSCORED"]
    } else if components.len() < TOTAL_POSSIBLE_COMPONENTS {
        vec!["PARTIAL_DATA"]
    } else {
        vec![]
    };

    let score_percent = if weight_total > 0.0 {
        (weighted_sum / weight_total * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    ScoredTrack {
        track_id: candidate.id,
        score_percent,
        components,
        badges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: i64, bpm: f32, key: &str, energy: f32, genre: &str, year: i32) -> TrackFeatures {
        TrackFeatures {
            id,
            bpm: Some(bpm),
            camelot_key: Some(key.to_string()),
            energy: Some(energy),
            genre: Some(genre.to_string()),
            year: Some(year),
        }
    }

    #[test]
    fn a_closely_matched_track_outscores_a_mismatched_one() {
        let now_playing = track(1, 126.0, "4A", 7.0, "Disco", 1978);
        let good_match = track(2, 128.0, "4A", 7.2, "Disco", 1979);
        let bad_match = track(3, 95.0, "9B", 3.0, "Metal", 2015);

        let results = score_candidates(&now_playing, &[bad_match, good_match], Mode::Keep);

        assert_eq!(results[0].track_id, 2, "good match should rank first");
        assert_eq!(results[1].track_id, 3);
        assert!(results[0].score_percent > results[1].score_percent);
        assert!(results[0].badges.is_empty(), "fully-scored track shouldn't be badged");
    }

    #[test]
    fn switching_mode_reorders_results() {
        let now_playing = track(1, 126.0, "4A", 6.0, "Disco", 1978);
        let same_genre_calm = track(2, 125.0, "4A", 6.0, "Disco", 1978);
        let different_genre = track(3, 126.0, "4A", 6.0, "Rock", 1978);

        let keep_results = score_candidates(&now_playing, &[same_genre_calm.clone(), different_genre.clone()], Mode::Keep);
        assert_eq!(keep_results[0].track_id, 2, "Keep should prefer the same genre");

        let switch_results = score_candidates(&now_playing, &[same_genre_calm, different_genre], Mode::Switch);
        assert_eq!(switch_results[0].track_id, 3, "Switch should prefer the genre change");
    }

    #[test]
    fn missing_data_is_flagged_not_guessed() {
        let now_playing = TrackFeatures {
            id: 1,
            bpm: Some(126.0),
            ..Default::default()
        };
        let partially_known = TrackFeatures {
            id: 2,
            bpm: Some(128.0),
            ..Default::default()
        };
        let totally_unknown = TrackFeatures { id: 3, ..Default::default() };

        let results = score_candidates(&now_playing, &[partially_known, totally_unknown], Mode::Keep);

        let partial = results.iter().find(|r| r.track_id == 2).unwrap();
        assert_eq!(partial.components.len(), 1);
        assert!(partial.badges.contains(&"PARTIAL_DATA"));

        let unknown = results.iter().find(|r| r.track_id == 3).unwrap();
        assert_eq!(unknown.score_percent, 0.0);
        assert!(unknown.badges.contains(&"UNSCORED"));
    }

    #[test]
    fn strongest_component_is_listed_first() {
        let now_playing = track(1, 126.0, "4A", 6.0, "Disco", 1978);
        let candidate = track(2, 126.0, "4A", 6.0, "Metal", 2020);

        let results = score_candidates(&now_playing, &[candidate], Mode::Keep);
        let top_component = &results[0].components[0];
        // BPM and Key are both perfect matches and carry the top two
        // weights under Keep — either is a legitimate "top reason".
        assert!(top_component.name == "BPM" || top_component.name == "Key");
        assert_eq!(top_component.value, 1.0);
    }

    #[test]
    fn mode_parsing_falls_back_to_keep() {
        assert_eq!(Mode::parse("BUILD"), Mode::Build);
        assert_eq!(Mode::parse("rescue"), Mode::Rescue);
        assert_eq!(Mode::parse("nonsense"), Mode::Keep);
    }
}
