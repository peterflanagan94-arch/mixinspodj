use mixflow_db::plays;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::DbState;

/// The live session every play is recorded against, created at launch.
pub struct CurrentSession(pub i64);

/// Called whenever a track becomes NOW PLAYING. `rank` is its position in the
/// recommendations on screen when it was chosen, if it came from there.
/// `detected` is set when Traktor reported the track playing.
#[tauri::command]
pub fn play_track(
    db: State<DbState>,
    session: State<CurrentSession>,
    track_id: i64,
    previous_track_id: Option<i64>,
    mode: String,
    rank: Option<i64>,
    detected: Option<bool>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    plays::record_play(
        &conn,
        session.0,
        track_id,
        previous_track_id,
        &mode,
        rank,
        detected.unwrap_or(false),
    )
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct TopPlayedDto {
    pub track_id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub bpm: Option<f64>,
    pub camelot_key: Option<String>,
    pub plays: i64,
}

/// The tracks this DJ has played most, across every session, for the
/// "Top played" panel.
#[tauri::command]
pub fn top_played_tracks(db: State<DbState>, limit: Option<i64>) -> Result<Vec<TopPlayedDto>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let ranked = plays::top_played(&conn, limit.unwrap_or(20)).map_err(|e| e.to_string())?;
    ranked
        .into_iter()
        .map(|(track_id, count)| track_details(&conn, track_id, count))
        .collect()
}

fn track_details(conn: &Connection, track_id: i64, plays: i64) -> Result<TopPlayedDto, String> {
    conn.query_row(
        "SELECT title, artist, bpm, camelot_key FROM tracks WHERE id = ?1",
        params![track_id],
        |row| {
            Ok(TopPlayedDto {
                track_id,
                title: row.get(0)?,
                artist: row.get(1)?,
                bpm: row.get(2)?,
                camelot_key: row.get(3)?,
                plays,
            })
        },
    )
    .map_err(|e| e.to_string())
}
