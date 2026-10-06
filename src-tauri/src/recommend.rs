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

    let now_playing = load_features(&conn, now_playing_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "now-playing track not found".to_string())?;

    let played = plays::played_in_session(&conn, session.0).map_err(|e| e.to_string())?;
    let transitions = plays::transition_counts_from(&conn, now_playing_id).map_err(|e| e.to_string())?;

    let mut candidates = load_candidate_features(&conn, now_playing_id).map_err(|e| e.to_string())?;
    for candidate in candidates.iter_mut() {
        candidate.played_in_session = played.contains_key(&candidate.id);
        candidate.transitions_from_now = transitions.get(&candidate.id).copied().unwrap_or(0);
    }

    let results = score_candidates(&now_playing, &candidates, Mode::parse(&mode));

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

fn load_candidate_features(conn: &Connection, exclude_id: i64) -> rusqlite::Result<Vec<TrackFeatures>> {
    let mut stmt = conn.prepare(
        "SELECT id, bpm, camelot_key, energy, genre, year FROM tracks \
         WHERE id != ?1 AND missing_file = 0",
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
