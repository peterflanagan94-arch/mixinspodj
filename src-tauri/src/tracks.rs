use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::DbState;

#[derive(Debug, Clone, Serialize)]
pub struct TrackSummary {
    pub id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub bpm: Option<f64>,
    pub camelot_key: Option<String>,
    pub energy: Option<f64>,
    pub file_path: String,
}

const SELECT_COLUMNS: &str =
    "id, title, artist, bpm, camelot_key, energy, file_path FROM tracks";

#[tauri::command]
pub fn list_tracks(db: State<DbState>) -> Result<Vec<TrackSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} ORDER BY artist IS NULL, artist, title LIMIT 200"
        ))
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

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<TrackSummary> {
    Ok(TrackSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        bpm: row.get(3)?,
        camelot_key: row.get(4)?,
        energy: row.get(5)?,
        file_path: row.get(6)?,
    })
}
