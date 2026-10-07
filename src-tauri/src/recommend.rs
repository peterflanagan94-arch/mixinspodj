use mixflow_db::plays;
use mixflow_recommender::{score_candidates, Mode, TrackFeatures};
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::plays::CurrentSession;
use crate::DbState;

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

    let results = score_candidates(&now_playing, &candidates, Mode::parse(mode));

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

    fn insert(conn: &Connection, path: &str, title: &str, artist: &str) -> i64 {
        conn.execute(
            "INSERT INTO tracks (file_path, title, artist, bpm, camelot_key) VALUES (?1, ?2, ?3, 128.0, '8B')",
            params![path, title, artist],
        )
        .unwrap();
        conn.last_insert_rowid()
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
    fn a_different_mix_of_the_same_song_is_still_suggested() {
        let conn = mixflow_db::connect_in_memory().unwrap();
        let now_playing = insert(&conn, "/a.mp3", "Saturday Night", "Whigfield");
        let remix = insert(&conn, "/b.mp3", "Saturday Night - Radio Mix", "Whigfield");

        let candidates = load_candidate_features(&conn, now_playing).unwrap();
        let ids: Vec<i64> = candidates.iter().map(|t| t.id).collect();
        assert!(ids.contains(&remix), "a differently-titled version is a different track");
    }
}
