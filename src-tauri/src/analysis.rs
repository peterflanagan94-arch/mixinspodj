use std::collections::HashSet;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use mixflow_analysis::{analyze_file, AnalysisResult};
use mixflow_db::library::{self, AnalysisUpdate};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::library_scan::is_cloud_only;
use crate::{broadcast, DbState};

/// Pause between background tracks, so analysis never saturates the machine.
const BACKGROUND_GAP: Duration = Duration::from_secs(2);
/// How often to look again while Traktor is broadcasting (a set may be live).
const LIVE_RECHECK: Duration = Duration::from_secs(30);
/// How often to look for new unanalysed tracks once the library is done.
const IDLE_RECHECK: Duration = Duration::from_secs(60);

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

        if is_cloud_only(path) {
            summary.errors += 1;
            continue;
        }

        match analyze_file(path) {
            Ok(result) => {
                let update = update_from(track.id, result);
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

    if is_cloud_only(Path::new(&path)) {
        return Err("This track is only in the cloud. Download it first, then analyse it.".to_string());
    }
    let result = analyze_file(Path::new(&path)).map_err(|e| e.to_string())?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    library::save_analysis(&conn, &update_from(track_id, result)).map_err(|e| e.to_string())
}

fn update_from(track_id: i64, result: AnalysisResult) -> AnalysisUpdate {
    AnalysisUpdate {
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
    }
}

/// Analyses unanalysed tracks one at a time in the background, so the library
/// fills in without anyone pressing Analyze. It pauses while Traktor is
/// broadcasting, so it never competes with a live set. A track that fails is
/// skipped for the rest of this run rather than retried endlessly.
pub fn start_background(app: AppHandle) {
    std::thread::spawn(move || {
        let mut failed: HashSet<i64> = HashSet::new();
        loop {
            if broadcast::is_live() {
                sleep(LIVE_RECHECK);
                continue;
            }

            let next = {
                let db = app.state::<DbState>();
                let Ok(conn) = db.0.lock() else {
                    sleep(IDLE_RECHECK);
                    continue;
                };
                library::next_track_needing_analysis(&conn, &failed)
            };
            let Ok(Some(track)) = next else {
                sleep(IDLE_RECHECK);
                continue;
            };

            if is_cloud_only(Path::new(&track.file_path)) {
                failed.insert(track.id);
                continue;
            }

            match analyze_file(Path::new(&track.file_path)) {
                Ok(result) => {
                    let db = app.state::<DbState>();
                    let saved = match db.0.lock() {
                        Ok(conn) => library::save_analysis(&conn, &update_from(track.id, result)).is_ok(),
                        Err(_) => false,
                    };
                    if !saved {
                        failed.insert(track.id);
                    }
                }
                Err(_) => {
                    failed.insert(track.id);
                }
            }
            sleep(BACKGROUND_GAP);
        }
    });
}
