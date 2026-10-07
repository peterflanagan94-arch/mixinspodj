use std::collections::HashMap;

use mixflow_db::plays;
use mixflow_recommender::{score_candidates, Mode, ScoredTrack, TrackFeatures};
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::plays::CurrentSession;
use crate::DbState;

/// How many scored tracks NEXT (and Random's pool) ever sees. Scoring the
/// whole library is cheap; sending thousands of rows across to the webview
/// and having React build a row for each one is not.
const RESULT_LIMIT: usize = 100;
/// Of those, how many of the tail slots are set aside for a different key at
/// a similar tempo. A library with hundreds of tracks in one key can
/// otherwise crowd every other key out of the list entirely — those tracks
/// would still mix fine on tempo alone, they just never get the chance to
/// show up below the wall of exact-key matches.
const VARIETY_RESERVED: usize = 20;
/// "Similar tempo" for that reservation: within 6% of the now-playing BPM —
/// the same threshold `bpm::score` already treats as a compatible match.
const VARIETY_BPM_TOLERANCE: f32 = 0.06;

#[derive(Debug, Clone, Serialize)]
pub struct ScoreComponentDto {
    pub name: String,
    pub weight: f32,
    pub value: f32,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoredTrackDto {
    pub track_id: i64,
    pub score_percent: f32,
    pub components: Vec<ScoreComponentDto>,
    pub badges: Vec<String>,
    /// When this track was last played in the current session (local time),
    /// if it has been. The UI dims the row and shows this.
    pub played_at: Option<String>,
}

/// Scores every other (non-missing-file) track in the library against
/// `now_playing_id` under `mode`, best match first, using both the audio
/// features and the DJ's own play history.
#[tauri::command]
pub fn recommend_next(
    db: State<DbState>,
    session: State<CurrentSession>,
    now_playing_id: i64,
    mode: String,
) -> Result<Vec<ScoredTrackDto>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    score_for(&conn, session.0, now_playing_id, &mode)
}

/// A random pick from the best matches for the track playing now, drawn from the
/// top ten so it's always a sensible fit, never a poor one.
#[tauri::command]
pub fn random_suggestion(
    db: State<DbState>,
    session: State<CurrentSession>,
    now_playing_id: i64,
    mode: String,
) -> Result<Option<ScoredTrackDto>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let ranked = score_for(&conn, session.0, now_playing_id, &mode)?;
    let best: Vec<ScoredTrackDto> = ranked
        .into_iter()
        .filter(|t| !t.badges.iter().any(|b| b == "UNSCORED"))
        .take(10)
        .collect();
    if best.is_empty() {
        return Ok(None);
    }
    Ok(Some(best[random_index(best.len())].clone()))
}

/// A number in 0..len that changes from call to call, with no extra dependency:
/// the standard library seeds each hasher randomly.
fn random_index(len: usize) -> usize {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let value = RandomState::new().build_hasher().finish();
    (value % len as u64) as usize
}

/// Keeps the top `limit - reserved` results exactly as scored (so the best,
/// safest matches — same key, same tempo — still lead), then fills the
/// remaining `reserved` slots from further down the list with the
/// best-scoring tracks that are in a *different* key but still close in
/// tempo, so a different key always gets a chance to show up rather than
/// being buried under however many exact-key matches the library has.
fn widen_with_variety(
    mut results: Vec<ScoredTrack>,
    now_playing: &TrackFeatures,
    candidates_by_id: &HashMap<i64, &TrackFeatures>,
    limit: usize,
    reserved: usize,
) -> Vec<ScoredTrack> {
    let primary_len = limit.saturating_sub(reserved).min(results.len());
    let rest = results.split_off(primary_len);
    let mut primary = results;

    let (Some(now_key), Some(now_bpm)) = (now_playing.camelot_key.as_deref(), now_playing.bpm) else {
        primary.extend(rest.into_iter().take(limit - primary.len()));
        return primary;
    };

    // Split what's left into "a different key at a similar tempo" (the
    // variety this is for) and everything else, keeping each group's
    // existing score order. If there isn't enough variety to fill all the
    // reserved slots, the leftover slots are backfilled from the rest by
    // score, same as before this existed — the list is never left short
    // just because variety happened to be scarce.
    let mut variety = Vec::with_capacity(reserved);
    let mut backfill = Vec::new();
    for candidate in rest {
        let is_variety = variety.len() < reserved
            && candidates_by_id.get(&candidate.track_id).is_some_and(|features| {
                let (Some(key), Some(bpm)) = (features.camelot_key.as_deref(), features.bpm) else {
                    return false;
                };
                key != now_key && ((bpm - now_bpm).abs() / now_bpm) <= VARIETY_BPM_TOLERANCE
            });
        if is_variety {
            variety.push(candidate);
        } else {
            backfill.push(candidate);
        }
    }

    primary.extend(variety);
    let remaining = limit.saturating_sub(primary.len());
    primary.extend(backfill.into_iter().take(remaining));
    primary
}

fn score_for(
    conn: &Connection,
    session_id: i64,
    now_playing_id: i64,
    mode: &str,
) -> Result<Vec<ScoredTrackDto>, String> {
    let now_playing = load_features(conn, now_playing_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "now-playing track not found".to_string())?;

    if now_playing.bpm.is_none() || now_playing.camelot_key.is_none() {
        return Ok(Vec::new());
    }

    let played = plays::played_in_session(conn, session_id).map_err(|e| e.to_string())?;
    let transitions = plays::transition_counts_from(conn, now_playing_id).map_err(|e| e.to_string())?;

    let mut candidates = load_candidate_features(conn, now_playing_id).map_err(|e| e.to_string())?;
    for candidate in candidates.iter_mut() {
        candidate.played_in_session = played.contains_key(&candidate.id);
        candidate.transitions_from_now = transitions.get(&candidate.id).copied().unwrap_or(0);
    }

    // Scoring every track is cheap (tens of milliseconds even for a large
    // library), but sending thousands of rows to the screen and having React
    // build a row for each one is not — that's what made switching modes feel
    // slow. Nothing past the top 100 is ever shown or drawn from.
    let results = score_candidates(&now_playing, &candidates, Mode::parse(mode));
    let candidates_by_id: HashMap<i64, &TrackFeatures> = candidates.iter().map(|c| (c.id, c)).collect();
    let results = widen_with_variety(results, &now_playing, &candidates_by_id, RESULT_LIMIT, VARIETY_RESERVED);

    Ok(results
        .into_iter()
        .map(|scored| ScoredTrackDto {
            track_id: scored.track_id,
            score_percent: scored.score_percent,
            components: scored
                .components
                .into_iter()
                .map(|c| ScoreComponentDto {
                    name: c.name.to_string(),
                    weight: c.weight,
                    value: c.value,
                    explanation: c.explanation.to_string(),
                })
                .collect(),
            badges: scored.badges.into_iter().map(str::to_string).collect(),
            played_at: played.get(&scored.track_id).cloned(),
        })
        .collect())
}

fn load_features(conn: &Connection, track_id: i64) -> rusqlite::Result<Option<TrackFeatures>> {
    conn.query_row(
        "SELECT id, bpm, camelot_key, energy, genre, year FROM tracks WHERE id = ?1",
        params![track_id],
        map_features,
    )
    .map(Some)
    .or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(e) })
}

/// Every other track, excluding the one playing now and — since the library
/// can hold more than one file of the same song (a duplicate download, a
/// re-rip) — anything that shares its title and artist. Suggesting a
/// duplicate of what's already playing is the same mistake as suggesting the
/// track itself.
fn load_candidate_features(conn: &Connection, exclude_id: i64) -> rusqlite::Result<Vec<TrackFeatures>> {
    let mut stmt = conn.prepare(
        "SELECT id, bpm, camelot_key, energy, genre, year FROM tracks \
         WHERE id != ?1 AND missing_file = 0 \
         AND title IS NOT NULL AND title != '' \
         AND NOT ( \
             title IS NOT NULL AND artist IS NOT NULL \
             AND lower(trim(title)) = (SELECT lower(trim(title)) FROM tracks WHERE id = ?1) \
             AND lower(trim(artist)) = (SELECT lower(trim(artist)) FROM tracks WHERE id = ?1) \
         )",
    )?;
    let rows = stmt.query_map(params![exclude_id], map_features)?.collect();
    rows
}

fn map_features(row: &rusqlite::Row) -> rusqlite::Result<TrackFeatures> {
    Ok(TrackFeatures {
        id: row.get(0)?,
        bpm: row.get::<_, Option<f64>>(1)?.map(|v| v as f32),
        camelot_key: row.get(2)?,
        energy: row.get::<_, Option<f64>>(3)?.map(|v| v as f32),
        genre: row.get(4)?,
        year: row.get(5)?,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn features(id: i64, bpm: f32, key: &str) -> TrackFeatures {
        TrackFeatures {
            id,
            bpm: Some(bpm),
            camelot_key: Some(key.to_string()),
            ..Default::default()
        }
    }

    fn scored(track_id: i64, score_percent: f32) -> ScoredTrack {
        ScoredTrack { track_id, score_percent, components: vec![], badges: vec![] }
    }

    #[test]
    fn a_different_key_at_a_similar_tempo_still_gets_a_chance() {
        let now = features(0, 128.0, "8B");
        // Five same-key "perfect" matches that would fill the whole list on
        // score alone, plus one different-key track at a close tempo that
        // would otherwise be pushed out entirely, and one different-key
        // track too far off tempo to belong in the "widen" slots at all.
        let library = vec![
            features(1, 128.0, "8B"),
            features(2, 128.0, "8B"),
            features(3, 128.0, "8B"),
            features(4, 128.0, "8B"),
            features(5, 128.0, "8B"),
            features(6, 129.0, "3A"),  // different key, close tempo
            features(7, 170.0, "5A"),  // different key, nowhere near tempo
        ];
        let by_id: HashMap<i64, &TrackFeatures> = library.iter().map(|f| (f.id, f)).collect();
        let results = vec![
            scored(1, 100.0),
            scored(2, 100.0),
            scored(3, 100.0),
            scored(4, 100.0),
            scored(5, 100.0),
            scored(6, 40.0),
            scored(7, 10.0),
        ];

        // limit 6, with 2 reserved: the top 4 by score lead, then the
        // reserved slots should prefer the close-tempo different key over
        // more same-key tracks or the too-far-off one.
        let widened = widen_with_variety(results, &now, &by_id, 6, 2);
        let ids: Vec<i64> = widened.iter().map(|s| s.track_id).collect();
        assert_eq!(ids.len(), 6);
        assert!(ids.contains(&6), "the close-tempo different key should get a reserved slot: {ids:?}");
        assert!(!ids.contains(&7), "the far-off-tempo track shouldn't take a reserved slot: {ids:?}");
    }

    #[test]
    fn with_no_variety_available_the_reserved_slots_backfill_from_the_rest() {
        let now = features(0, 128.0, "8B");
        let library: Vec<TrackFeatures> = (1..=6).map(|i| features(i, 128.0, "8B")).collect();
        let by_id: HashMap<i64, &TrackFeatures> = library.iter().map(|f| (f.id, f)).collect();
        let results: Vec<ScoredTrack> = (1..=6).map(|i| scored(i, 100.0 - i as f32)).collect();

        let widened = widen_with_variety(results, &now, &by_id, 6, 2);
        assert_eq!(widened.len(), 6, "no variety candidates exist, so the list still reaches the limit");
    }

    fn insert(conn: &Connection, path: &str, title: &str, artist: &str) -> i64 {
        conn.execute(
            "INSERT INTO tracks (file_path, title, artist, bpm, camelot_key) VALUES (?1, ?2, ?3, 128.0, '8B')",
            params![path, title, artist],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn a_title_less_track_is_not_suggested() {
        let conn = mixflow_db::connect_in_memory().unwrap();
        let now_playing = insert(&conn, "/a.mp3", "Saturday Night", "Whigfield");
        conn.execute(
            "INSERT INTO tracks (file_path, title, artist, bpm, camelot_key) VALUES ('/b.mp3', NULL, NULL, 128.0, '8B')",
            [],
        )
        .unwrap();
        let untitled = conn.last_insert_rowid();
        let titled = insert(&conn, "/c.mp3", "Real Track", "Real Artist");

        let candidates = load_candidate_features(&conn, now_playing).unwrap();
        let ids: Vec<i64> = candidates.iter().map(|t| t.id).collect();
        assert!(!ids.contains(&untitled), "a track with no title has nothing to search for in Traktor");
        assert!(ids.contains(&titled));
    }

    #[test]
    fn a_duplicate_of_the_now_playing_track_is_not_suggested() {
        let conn = mixflow_db::connect_in_memory().unwrap();
        let now_playing = insert(&conn, "/a.mp3", "Saturday Night", "Whigfield");
        let duplicate = insert(&conn, "/b.mp3", "Saturday Night", "Whigfield");
        let other = insert(&conn, "/c.mp3", "Other Track", "Someone Else");

        let candidates = load_candidate_features(&conn, now_playing).unwrap();
        let ids: Vec<i64> = candidates.iter().map(|t| t.id).collect();
        assert!(!ids.contains(&duplicate), "a same-title-and-artist duplicate should be excluded");
        assert!(ids.contains(&other));
    }

    #[test]
    fn results_never_exceed_the_limit_even_with_a_huge_library() {
        let conn = mixflow_db::connect_in_memory().unwrap();
        let now_playing = insert(&conn, "/now.mp3", "Now Playing", "Someone");
        for i in 0..(RESULT_LIMIT + 50) {
            insert(&conn, &format!("/{i}.mp3"), &format!("Track {i}"), "Various");
        }

        let results = score_for(&conn, 1, now_playing, "KEEP").unwrap();
        assert_eq!(results.len(), RESULT_LIMIT);
    }

    #[test]
    fn a_different_mix_of_the_same_song_is_still_suggested() {
        let conn = mixflow_db::connect_in_memory().unwrap();
        let now_playing = insert(&conn, "/a.mp3", "Saturday Night", "Whigfield");
        let remix = insert(&conn, "/b.mp3", "Saturday Night - Radio Mix", "Whigfield");

        let candidates = load_candidate_features(&conn, now_playing).unwrap();
        let ids: Vec<i64> = candidates.iter().map(|t| t.id).collect();
        assert!(ids.contains(&remix), "a differently-titled version is a different track");
    }
}

