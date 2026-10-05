use mixflow_db::library::{self, MetadataEdit};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::DbState;

#[derive(Debug, Clone, Serialize)]
pub struct TrackSummary {
    pub id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub duration_ms: Option<i64>,
    pub bpm: Option<f64>,
    pub camelot_key: Option<String>,
    pub energy: Option<f64>,
    pub danceability: Option<f64>,
    pub file_format: Option<String>,
    pub file_path: String,
    pub missing_file: bool,
}

const SELECT_COLUMNS: &str = "id, title, artist, album, genre, year, duration_ms, bpm, \
     camelot_key, energy, danceability, file_format, file_path, missing_file FROM tracks";

/// Returns the whole library (no pagination yet — fine for the library
/// sizes this MVP has been exercised with; a real cap/virtualized table is
/// future work once someone tests with a many-thousand-track collection).
#[tauri::command]
pub fn list_tracks(db: State<DbState>) -> Result<Vec<TrackSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!("SELECT {SELECT_COLUMNS} ORDER BY artist IS NULL, artist, title"))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[tauri::command]
pub fn search_tracks(db: State<DbState>, query: String) -> Result<Vec<TrackSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let pattern = format!("%{}%", query.trim());
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} WHERE title LIKE ?1 OR artist LIKE ?1 \
             ORDER BY artist IS NULL, artist, title LIMIT 50"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![pattern], map_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackMetadataInput {
    pub track_id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
}

/// Manual metadata edit. Deliberately never touches the file on disk —
/// the database is the source of truth, source files are left alone
/// unless the user explicitly opts into writing tags back (not built).
#[tauri::command]
pub fn update_track_metadata(db: State<DbState>, edit: TrackMetadataInput) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    library::update_metadata(
        &conn,
        &MetadataEdit {
            track_id: edit.track_id,
            title: edit.title,
            artist: edit.artist,
            album: edit.album,
            genre: edit.genre,
            year: edit.year,
        },
    )
    .map_err(|e| e.to_string())
}

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<TrackSummary> {
    Ok(TrackSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        album: row.get(3)?,
        genre: row.get(4)?,
        year: row.get(5)?,
        duration_ms: row.get(6)?,
        bpm: row.get(7)?,
        camelot_key: row.get(8)?,
        energy: row.get(9)?,
        danceability: row.get(10)?,
        file_format: row.get(11)?,
        file_path: row.get(12)?,
        missing_file: row.get::<_, i64>(13)? != 0,
    })
}
