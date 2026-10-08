use mixflow_db::plays;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{play_sharing, DbState};

/// The live session every play is recorded against, created at launch.
pub struct CurrentSession(pub i64);

/// Called whenever a track becomes NOW PLAYING. `rank` is its position in the
/// recommendations on screen when it was chosen, if it came from there.
/// `detected` is set when Traktor reported the track playing.
#[tauri::command]
pub fn play_track(
    app: AppHandle,
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
    .map_err(|e| e.to_string())?;

    if let Ok((title, artist, bpm, camelot_key, genre)) = share_details(&conn, track_id) {
        play_sharing::share_play(&app, title, artist, bpm, camelot_key, genre);
    }
    Ok(())
}

type ShareDetails = (Option<String>, Option<String>, Option<f64>, Option<String>, Option<String>);

fn share_details(conn: &Connection, track_id: i64) -> rusqlite::Result<ShareDetails> {
    conn.query_row(
        "SELECT title, artist, bpm, camelot_key, genre FROM tracks WHERE id = ?1",
        params![track_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )
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

#[derive(Debug, Clone, Serialize)]
pub struct SetSummaryDto {
    pub session_id: i64,
    pub started_at: String,
    pub last_play_at: Option<String>,
    pub plays: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetPlayDto {
    pub position: i64,
    pub track_id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub bpm: Option<f64>,
    pub camelot_key: Option<String>,
    pub played_at: String,
    pub source: String,
    pub mode: String,
    pub est_seconds: Option<i64>,
}

/// Every app session, newest first, for the Sets panel.
#[tauri::command]
pub fn list_sets(db: State<DbState>) -> Result<Vec<SetSummaryDto>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let sessions = plays::list_sessions(&conn).map_err(|e| e.to_string())?;
    Ok(sessions
        .into_iter()
        .map(|s| SetSummaryDto {
            session_id: s.session_id,
            started_at: s.started_at,
            last_play_at: s.last_play_at,
            plays: s.plays,
        })
        .collect())
}

/// The tracks played in one session, in order, with estimated playtimes.
#[tauri::command]
pub fn set_report(db: State<DbState>, session_id: i64) -> Result<Vec<SetPlayDto>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let plays = plays::session_plays(&conn, session_id).map_err(|e| e.to_string())?;
    Ok(plays
        .into_iter()
        .map(|p| SetPlayDto {
            position: p.position,
            track_id: p.track_id,
            title: p.title,
            artist: p.artist,
            bpm: p.bpm,
            camelot_key: p.camelot_key,
            played_at: p.played_at,
            source: p.source,
            mode: p.mode,
            est_seconds: p.est_seconds,
        })
        .collect())
}
