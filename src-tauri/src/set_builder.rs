//! Auto-sequences a full DJ set toward a target length and energy arc.
//!
//! Reuses the exact scoring engine NEXT already uses — "what fits well
//! after this track" — but aimed at a target energy for each position in
//! the set instead of one fixed direction, so a 90-minute set can actually
//! follow a shape (warm-up, peak, wind-down, ...) instead of drifting one
//! way or holding flat the whole way through. At each step the gap between
//! the last pick's energy and that position's target energy decides which
//! existing mode (KEEP/BUILD/COOL) scores the next pick — no new scoring
//! logic, just a new way of choosing which of the existing modes to ask.

use std::collections::HashSet;

use mixflow_recommender::{score_candidates, Mode, TrackFeatures};
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::DbState;

/// However short the target length or long the average track, a set never
/// grows past this many tracks.
const MAX_TRACKS: usize = 200;
/// Used only when a track is missing its duration tag.
const DEFAULT_TRACK_MS: i64 = 210_000;
/// Candidates longer than this never enter the pool — a DJ set is built
/// from tracks, not from a 20-minute game-show audio rip or soundtrack
/// needle-drop that happens to share a workable BPM and key.
const MAX_CANDIDATE_MS: i64 = 10 * 60_000;
/// Same half-width as the KEEP energy band (crates/mixflow-recommender/src/energy.rs)
/// — staying under it picks KEEP, matching that band's own deliberate gap.
const HOLD_THRESHOLD: f32 = 0.75;

#[derive(Debug, Clone)]
struct SetCandidate {
    features: TrackFeatures,
    title: Option<String>,
    artist: Option<String>,
    duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetTrackDto {
    pub track_id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub bpm: Option<f32>,
    pub camelot_key: Option<String>,
    pub energy: Option<f32>,
    pub duration_ms: Option<i64>,
    /// What the arc wanted the energy to be at this point in the set.
    pub target_energy: f32,
    /// "OPEN", or the mode (KEEP/BUILD/COOL) used to pick this track.
    pub transition: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SetBuilderResult {
    pub tracks: Vec<SetTrackDto>,
    pub total_duration_ms: i64,
    pub target_duration_ms: i64,
}

/// A handful of DJ-recognisable energy shapes, each a set of (position,
/// energy) control points on a 1-10 scale, linearly interpolated between
/// them. Position runs 0.0 (set start) to 1.0 (set end).
fn arc_points(arc: &str) -> &'static [(f32, f32)] {
    match arc {
        "steady_build" => &[(0.0, 3.0), (1.0, 9.0)],
        "peak_time" => &[(0.0, 6.5), (0.3, 8.0), (0.8, 8.5), (1.0, 7.5)],
        "dinner_then_dancefloor" => &[(0.0, 2.5), (0.4, 2.5), (0.55, 5.0), (0.7, 8.0), (1.0, 8.5)],
        // "warm_up_peak_wind_down" and anything unrecognised.
        _ => &[(0.0, 3.0), (0.15, 4.0), (0.5, 7.5), (0.7, 9.0), (0.85, 9.0), (1.0, 5.0)],
    }
}

fn target_energy(arc: &str, position: f32) -> f32 {
    let points = arc_points(arc);
    let position = position.clamp(0.0, 1.0);
    for pair in points.windows(2) {
        let (p0, e0) = pair[0];
        let (p1, e1) = pair[1];
        if position >= p0 && position <= p1 {
            if p1 == p0 {
                return e1;
            }
            let t = (position - p0) / (p1 - p0);
            return e0 + (e1 - e0) * t;
        }
    }
    points.last().map(|&(_, e)| e).unwrap_or(5.0)
}

/// Which mode to ask the scoring engine for, given where the last pick's
/// energy sits relative to where this position in the set wants to be.
fn mode_for_target(last_energy: Option<f32>, target: f32) -> Mode {
    let Some(last) = last_energy else { return Mode::Keep };
    let delta = target - last;
    if delta > HOLD_THRESHOLD {
        Mode::Build
    } else if delta < -HOLD_THRESHOLD {
        Mode::Cool
    } else {
        Mode::Keep
    }
}

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Build => "BUILD",
        Mode::Cool => "COOL",
        Mode::Rescue => "RESCUE",
        Mode::Switch => "SWITCH",
        Mode::Keep => "KEEP",
    }
}

/// The library can hold more than one file of the same song — this is what
/// NEXT's own duplicate-of-now-playing exclusion keys on, generalised here
/// to "already used anywhere in this set", not just the current track.
fn normalized_title_artist(title: &Option<String>, artist: &Option<String>) -> Option<(String, String)> {
    let title = title.as_ref()?.trim().to_lowercase();
    let artist = artist.as_deref().unwrap_or("").trim().to_lowercase();
    Some((title, artist))
}

fn mark_used(c: &SetCandidate, used_ids: &mut HashSet<i64>, used_ta: &mut HashSet<(String, String)>) {
    used_ids.insert(c.features.id);
    if let Some(ta) = normalized_title_artist(&c.title, &c.artist) {
        used_ta.insert(ta);
    }
}

fn to_dto(c: &SetCandidate, target_energy: f32, transition: &str) -> SetTrackDto {
    SetTrackDto {
        track_id: c.features.id,
        title: c.title.clone(),
        artist: c.artist.clone(),
        bpm: c.features.bpm,
        camelot_key: c.features.camelot_key.clone(),
        energy: c.features.energy,
        duration_ms: c.duration_ms,
        target_energy,
        transition: transition.to_string(),
    }
}

/// Picks the single track from `pool` whose energy sits closest to
/// `target`, used both for the no-seed opener and for ordering a seed
/// subset's own first track.
fn closest_to_energy(pool: &[SetCandidate], target: f32) -> Option<SetCandidate> {
    pool.iter()
        .filter(|c| c.features.energy.is_some())
        .min_by(|a, b| {
            let da = (a.features.energy.unwrap() - target).abs();
            let db = (b.features.energy.unwrap() - target).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

/// Orders a fixed set of "must include" tracks (e.g. a DJ's own Spotify
/// playlist, matched to the library) into a mixable sequence, using the
/// same scoring the main fill loop uses — greedily chaining whichever
/// remaining seed fits best after the last one placed, starting from
/// whichever seed is closest to the arc's opening energy. Unknown ids
/// (not found in `pool`) and duplicate ids are silently dropped rather
/// than guessed at; ordering is only ever over what's actually there.
fn order_seeds(pool: &[SetCandidate], arc: &str, seed_ids: &[i64]) -> Vec<SetCandidate> {
    let mut seen = HashSet::new();
    let mut remaining: Vec<SetCandidate> = seed_ids
        .iter()
        .filter(|id| seen.insert(**id))
        .filter_map(|id| pool.iter().find(|c| c.features.id == *id).cloned())
        .collect();
    if remaining.is_empty() {
        return remaining;
    }

    let Some(opener) = closest_to_energy(&remaining, target_energy(arc, 0.0)) else {
        return remaining;
    };
    remaining.retain(|c| c.features.id != opener.features.id);
    let mut ordered = vec![opener];

    while !remaining.is_empty() {
        let last = ordered.last().expect("just pushed").features.clone();
        let candidate_features: Vec<TrackFeatures> = remaining.iter().map(|c| c.features.clone()).collect();
        // Position within just the seed subset — a rough stand-in so early
        // vs. late seeds lean toward the right part of the arc; the real
        // position (against the whole set) is used once the main fill loop
        // takes over afterward.
        let position = ordered.len() as f32 / (ordered.len() + remaining.len()) as f32;
        let mode = mode_for_target(last.energy, target_energy(arc, position));
        let scored = score_candidates(&last, &candidate_features, mode);
        let next_id = scored.into_iter().find(|s| !s.badges.iter().any(|b| *b == "UNSCORED")).map(|s| s.track_id);
        let index = match next_id {
            Some(id) => remaining.iter().position(|c| c.features.id == id).unwrap_or(0),
            None => 0,
        };
        ordered.push(remaining.remove(index));
    }
    ordered
}

/// The actual sequencing: pure and DB-free, like the scoring crate itself,
/// so it's directly unit-testable. With no `seed_track_ids`, picks an
/// opener automatically; with some, orders them for mixability and places
/// them first — a DJ's own "must play these" list (e.g. matched from a
/// Spotify playlist). Either way, it then repeatedly scores everything not
/// used yet against the last pick, under whichever mode moves toward that
/// position's target energy, until the running total reaches
/// `target_duration_ms`.
fn sequence(pool: Vec<SetCandidate>, arc: &str, target_duration_ms: i64, seed_track_ids: &[i64]) -> SetBuilderResult {
    if pool.is_empty() || target_duration_ms <= 0 {
        return SetBuilderResult { tracks: Vec::new(), total_duration_ms: 0, target_duration_ms: target_duration_ms.max(0) };
    }

    let mut used_ids = HashSet::new();
    let mut used_ta = HashSet::new();
    let mut total_ms: i64 = 0;
    let mut tracks = Vec::new();
    let mut last: TrackFeatures;

    if seed_track_ids.is_empty() {
        let opener_target = target_energy(arc, 0.0);
        let Some(opener) = closest_to_energy(&pool, opener_target) else {
            return SetBuilderResult { tracks: Vec::new(), total_duration_ms: 0, target_duration_ms };
        };
        mark_used(&opener, &mut used_ids, &mut used_ta);
        total_ms = opener.duration_ms.unwrap_or(DEFAULT_TRACK_MS);
        last = opener.features.clone();
        tracks.push(to_dto(&opener, opener_target, "OPEN"));
    } else {
        let ordered_seeds = order_seeds(&pool, arc, seed_track_ids);
        if ordered_seeds.is_empty() {
            return SetBuilderResult { tracks: Vec::new(), total_duration_ms: 0, target_duration_ms };
        }
        last = ordered_seeds[0].features.clone();
        for seed in &ordered_seeds {
            mark_used(seed, &mut used_ids, &mut used_ta);
            total_ms += seed.duration_ms.unwrap_or(DEFAULT_TRACK_MS);
            let position = (total_ms as f32 / target_duration_ms as f32).min(1.0);
            tracks.push(to_dto(seed, target_energy(arc, position), "PLAYLIST"));
            last = seed.features.clone();
        }
    }

    while total_ms < target_duration_ms && tracks.len() < MAX_TRACKS {
        let position = (total_ms as f32 / target_duration_ms as f32).min(1.0);
        let target = target_energy(arc, position);
        let mode = mode_for_target(last.energy, target);

        let remaining_features: Vec<TrackFeatures> = pool
            .iter()
            .filter(|c| !used_ids.contains(&c.features.id))
            .filter(|c| match normalized_title_artist(&c.title, &c.artist) {
                Some(ta) => !used_ta.contains(&ta),
                None => true,
            })
            .map(|c| c.features.clone())
            .collect();
        if remaining_features.is_empty() {
            break;
        }

        let scored = score_candidates(&last, &remaining_features, mode);
        let Some(best) = scored.into_iter().find(|s| !s.badges.iter().any(|b| *b == "UNSCORED")) else {
            break;
        };
        let chosen = pool
            .iter()
            .find(|c| c.features.id == best.track_id)
            .expect("scored id came from this pool")
            .clone();

        mark_used(&chosen, &mut used_ids, &mut used_ta);
        total_ms += chosen.duration_ms.unwrap_or(DEFAULT_TRACK_MS);
        last = chosen.features.clone();
        tracks.push(to_dto(&chosen, target, mode_label(mode)));
    }

    SetBuilderResult { tracks, total_duration_ms: total_ms, target_duration_ms }
}

fn load_set_candidates(conn: &Connection, genre: Option<&str>) -> rusqlite::Result<Vec<SetCandidate>> {
    // A genre tag is required — it's the one signal that rules out a
    // non-music audio file sharing a workable BPM and key (see
    // MAX_CANDIDATE_MS above for the other half of that same guard).
    let base = format!(
        "SELECT id, bpm, camelot_key, energy, genre, year, title, artist, duration_ms FROM tracks \
         WHERE missing_file = 0 AND title IS NOT NULL AND title != '' \
         AND bpm IS NOT NULL AND camelot_key IS NOT NULL AND energy IS NOT NULL \
         AND genre IS NOT NULL AND genre != '' \
         AND (duration_ms IS NULL OR duration_ms <= {MAX_CANDIDATE_MS})"
    );

    fn map_row(row: &rusqlite::Row) -> rusqlite::Result<SetCandidate> {
        Ok(SetCandidate {
            features: TrackFeatures {
                id: row.get(0)?,
                bpm: row.get::<_, Option<f64>>(1)?.map(|v| v as f32),
                camelot_key: row.get(2)?,
                energy: row.get::<_, Option<f64>>(3)?.map(|v| v as f32),
                genre: row.get(4)?,
                year: row.get(5)?,
                ..Default::default()
            },
            title: row.get(6)?,
            artist: row.get(7)?,
            duration_ms: row.get(8)?,
        })
    }

    match genre.map(str::trim).filter(|g| !g.is_empty()) {
        Some(g) => {
            let mut stmt = conn.prepare(&format!("{base} AND genre LIKE ?1"))?;
            let pattern = format!("%{g}%");
            let rows = stmt.query_map(params![pattern], map_row)?.collect();
            rows
        }
        None => {
            let mut stmt = conn.prepare(&base)?;
            let rows = stmt.query_map([], map_row)?.collect();
            rows
        }
    }
}

/// Builds a full set: picks an opener automatically, or — given
/// `seed_track_ids` — orders those tracks first (e.g. a DJ's own Spotify
/// playlist, matched to the library) and fills in the rest around them.
/// Then repeatedly asks the scoring engine for the best next track toward
/// `arc`'s target energy at each point, until the running time reaches
/// `duration_minutes`. `genre`, if given, restricts the whole candidate
/// pool to tracks whose genre tag contains it (so a compound tag like
/// "Pop, Dance" still matches a "Dance" filter).
#[tauri::command]
pub fn build_set(
    db: State<DbState>,
    duration_minutes: u32,
    arc: String,
    genre: Option<String>,
    seed_track_ids: Vec<i64>,
) -> Result<SetBuilderResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let pool = load_set_candidates(&conn, genre.as_deref()).map_err(|e| e.to_string())?;
    let target_duration_ms = (duration_minutes as i64) * 60_000;
    Ok(sequence(pool, &arc, target_duration_ms, &seed_track_ids))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: i64, bpm: f32, key: &str, energy: f32, title: &str, duration_ms: i64) -> SetCandidate {
        SetCandidate {
            features: TrackFeatures {
                id,
                bpm: Some(bpm),
                camelot_key: Some(key.to_string()),
                energy: Some(energy),
                genre: Some("House".to_string()),
                year: Some(2020),
                ..Default::default()
            },
            title: Some(title.to_string()),
            artist: Some("Artist".to_string()),
            duration_ms: Some(duration_ms),
        }
    }

    #[test]
    fn arc_interpolates_linearly_between_control_points() {
        assert_eq!(target_energy("steady_build", 0.0), 3.0);
        assert_eq!(target_energy("steady_build", 1.0), 9.0);
        assert_eq!(target_energy("steady_build", 0.5), 6.0);
    }

    #[test]
    fn unknown_arc_falls_back_to_warm_up_peak_wind_down() {
        assert_eq!(target_energy("nonsense", 0.0), target_energy("warm_up_peak_wind_down", 0.0));
    }

    #[test]
    fn mode_choice_follows_the_gap_to_the_target_with_a_hold_band() {
        assert_eq!(mode_for_target(Some(5.0), 7.0), Mode::Build);
        assert_eq!(mode_for_target(Some(7.0), 5.0), Mode::Cool);
        assert_eq!(mode_for_target(Some(5.0), 5.3), Mode::Keep);
        assert_eq!(mode_for_target(None, 8.0), Mode::Keep);
    }

    #[test]
    fn a_steady_build_set_actually_rises_in_energy() {
        let pool: Vec<SetCandidate> = (0..12)
            .map(|i| candidate(i, 124.0 + i as f32, "8A", 2.0 + i as f32 * 0.7, &format!("Track {i}"), 180_000))
            .collect();

        let result = sequence(pool, "steady_build", 20 * 60_000, &[]);
        assert!(result.tracks.len() >= 5, "expected several tracks, got {}", result.tracks.len());
        let first_energy = result.tracks.first().unwrap().energy.unwrap();
        let last_energy = result.tracks.last().unwrap().energy.unwrap();
        assert!(last_energy > first_energy, "set should end higher-energy than it started");
    }

    #[test]
    fn stops_once_the_target_duration_is_reached() {
        let pool: Vec<SetCandidate> = (0..20).map(|i| candidate(i, 128.0, "8A", 6.0, &format!("Track {i}"), 180_000)).collect();
        let result = sequence(pool, "peak_time", 10 * 60_000, &[]);
        assert!(result.total_duration_ms >= result.target_duration_ms);
        assert!(result.tracks.len() <= 5, "a 10-minute set from 3-minute tracks shouldn't need 6+");
    }

    #[test]
    fn never_repeats_a_track_or_a_duplicate_of_one_already_used() {
        let mut pool = vec![
            candidate(1, 128.0, "8A", 5.0, "Same Song", 180_000),
            candidate(2, 128.0, "8A", 5.0, "Same Song", 180_000), // duplicate file, same title+artist
        ];
        pool.push(candidate(3, 128.0, "8A", 5.0, "Different Song", 180_000));

        let result = sequence(pool, "peak_time", 15 * 60_000, &[1]);
        let ids: Vec<i64> = result.tracks.iter().map(|t| t.track_id).collect();
        assert_eq!(ids, vec![1, 3], "the duplicate of track 1 should never be picked");
    }

    #[test]
    fn a_seed_track_opens_the_set() {
        let pool: Vec<SetCandidate> = (0..5).map(|i| candidate(i, 128.0, "8A", 6.0, &format!("Track {i}"), 180_000)).collect();
        let result = sequence(pool, "peak_time", 15 * 60_000, &[3]);
        assert_eq!(result.tracks[0].track_id, 3);
        assert_eq!(result.tracks[0].transition, "PLAYLIST");
    }

    #[test]
    fn an_unknown_seed_id_produces_an_empty_set_rather_than_guessing() {
        let pool = vec![candidate(1, 128.0, "8A", 6.0, "Track", 180_000)];
        let result = sequence(pool, "peak_time", 15 * 60_000, &[999]);
        assert!(result.tracks.is_empty());
    }

    #[test]
    fn an_empty_pool_produces_an_empty_set() {
        let result = sequence(Vec::new(), "peak_time", 15 * 60_000, &[]);
        assert!(result.tracks.is_empty());
        assert_eq!(result.total_duration_ms, 0);
    }

    #[test]
    fn multiple_seeds_are_all_placed_first_and_the_rest_fills_in_around_them() {
        let mut pool = vec![
            candidate(1, 120.0, "8A", 8.0, "Playlist Track A", 180_000),
            candidate(2, 120.0, "8A", 3.0, "Playlist Track B", 180_000),
        ];
        for i in 10..20 {
            pool.push(candidate(i, 120.0, "8A", 5.0, &format!("Library Track {i}"), 180_000));
        }

        let result = sequence(pool, "peak_time", 15 * 60_000, &[1, 2]);
        let seed_positions: Vec<usize> = result
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.track_id == 1 || t.track_id == 2)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(seed_positions, vec![0, 1], "both seeds should be placed first, before any filled-in track");
        assert!(result.tracks.iter().take(2).all(|t| t.transition == "PLAYLIST"));
        assert!(result.tracks.len() > 2, "the rest of the target length should be filled in around the seeds");
    }

    #[test]
    fn duplicate_seed_ids_are_only_placed_once() {
        let pool: Vec<SetCandidate> = (0..5).map(|i| candidate(i, 128.0, "8A", 6.0, &format!("Track {i}"), 180_000)).collect();
        let result = sequence(pool, "peak_time", 15 * 60_000, &[1, 1, 1]);
        let ones = result.tracks.iter().filter(|t| t.track_id == 1).count();
        assert_eq!(ones, 1);
    }

    #[test]
    fn seeds_that_do_not_exist_are_dropped_rather_than_failing_the_whole_build() {
        let pool: Vec<SetCandidate> = (0..5).map(|i| candidate(i, 128.0, "8A", 6.0, &format!("Track {i}"), 180_000)).collect();
        let result = sequence(pool, "peak_time", 15 * 60_000, &[999, 2]);
        assert_eq!(result.tracks[0].track_id, 2, "the real seed should still be used even though the first id didn't exist");
    }

    fn insert_track(
        conn: &Connection,
        path: &str,
        title: &str,
        genre: Option<&str>,
        duration_ms: Option<i64>,
    ) {
        conn.execute(
            "INSERT INTO tracks (file_path, title, artist, genre, bpm, camelot_key, energy, duration_ms) \
             VALUES (?1, ?2, 'Artist', ?3, 128.0, '8A', 6.0, ?4)",
            params![path, title, genre, duration_ms],
        )
        .expect("insert should succeed");
    }

    #[test]
    fn candidates_need_a_genre_tag_and_cannot_be_over_ten_minutes() {
        let conn = mixflow_db::connect_in_memory().expect("in-memory db should open");
        insert_track(&conn, "/a.mp3", "Tagged and short", Some("House"), Some(180_000));
        insert_track(&conn, "/b.mp3", "No genre at all", None, Some(180_000));
        insert_track(&conn, "/c.mp3", "Tagged but a 22-minute game-show rip", Some("House"), Some(22 * 60_000));
        insert_track(&conn, "/d.mp3", "Tagged, duration unknown", Some("House"), None);

        let candidates = load_set_candidates(&conn, None).expect("query should succeed");
        let titles: Vec<&str> = candidates.iter().map(|c| c.title.as_deref().unwrap()).collect();

        assert!(titles.contains(&"Tagged and short"));
        assert!(titles.contains(&"Tagged, duration unknown"), "unknown duration shouldn't be excluded");
        assert!(!titles.contains(&"No genre at all"), "untagged tracks shouldn't be candidates");
        assert!(!titles.contains(&"Tagged but a 22-minute game-show rip"), "over-length tracks shouldn't be candidates");
    }
}
