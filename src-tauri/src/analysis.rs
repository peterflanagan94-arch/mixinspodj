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
/// danceability). Runs as a synchronous command — Tauri schedules those off
/// the main thread, so this never blocks the UI — and processes tracks one
/// at a time rather than in parallel, which keeps CPU use bounded (the
/// product principle here is "never peg the CPU during a live gig," not
/// "finish as fast as possible").
#[tauri::command]
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
