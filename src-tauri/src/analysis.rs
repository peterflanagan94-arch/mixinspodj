use std::path::Path;

use mixflow_analysis::analyze_file;
use mixflow_db::library::{self, AnalysisUpdate};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::DbState;

#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeProgress {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AnalyzeSummary {
    pub total: usize,
    pub analyzed: usize,
    pub errors: usize,
}

/// Analyzes every track that hasn't been analyzed yet (BPM, key, energy,
/// danceability). `async` so it runs on a background thread: a plain Tauri
/// command runs on the main thread and would freeze the window for the whole
/// run. Tracks are processed one at a time, which keeps CPU use bounded.
#[tauri::command(async)]
pub fn analyze_library(app: AppHandle, db: State<DbState>) -> Result<AnalyzeSummary, String> {
    let pending = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        library::tracks_needing_analysis(&conn).map_err(|e| e.to_string())?
    };

    let total = pending.len();
    let mut summary = AnalyzeSummary {
        total,
        ..Default::default()
    };

    for (index, track) in pending.iter().enumerate() {
        let path = Path::new(&track.file_path);
        let current_file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let _ = app.emit(
            "analysis-progress",
            AnalyzeProgress {
                current: index + 1,
                total,
                current_file,
            },
        );

        match analyze_file(path) {
            Ok(result) => {
                let update = AnalysisUpdate {
                    track_id: track.id,
                    bpm: result.bpm,
                    bpm_confidence: result.bpm_confidence,
                    key: result.key,
                    camelot_key: result.camelot_key,
                    energy: result.energy,
                    danceability: result.danceability,
                    loudness_rms: result.loudness_rms,
                    spectral_centroid: result.spectral_centroid,
                    onset_density: result.onset_density,
                };
                let conn = db.0.lock().map_err(|e| e.to_string())?;
                match library::save_analysis(&conn, &update) {
                    Ok(()) => summary.analyzed += 1,
                    Err(_) => summary.errors += 1,
                }
            }
            Err(_) => summary.errors += 1,
        }
    }

    Ok(summary)
}

/// Analyses one track straight away, so a track that starts playing without
/// BPM or key gets suggestions without a full library analysis. A track that's
/// already analysed is left alone.
#[tauri::command(async)]
pub fn analyze_track(db: State<DbState>, track_id: i64) -> Result<(), String> {
    let path: String = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let analysed: bool = conn
            .query_row(
                "SELECT analyzed_at IS NOT NULL FROM tracks WHERE id = ?1",
                rusqlite::params![track_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if analysed {
            return Ok(());
        }
        conn.query_row(
            "SELECT file_path FROM tracks WHERE id = ?1",
            rusqlite::params![track_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?
    };

    let result = analyze_file(Path::new(&path)).map_err(|e| e.to_string())?;
    let update = AnalysisUpdate {
        track_id,
        bpm: result.bpm,
        bpm_confidence: result.bpm_confidence,
        key: result.key,
        camelot_key: result.camelot_key,
        energy: result.energy,
        danceability: result.danceability,
        loudness_rms: result.loudness_rms,
        spectral_centroid: result.spectral_centroid,
        onset_density: result.onset_density,
    };
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    library::save_analysis(&conn, &update).map_err(|e| e.to_string())
}
