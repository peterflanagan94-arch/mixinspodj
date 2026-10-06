use std::path::Path;
use std::time::Duration;

use mixflow_db::library::{self, UpsertOutcome};
use tauri::{AppHandle, Emitter, Manager};
use walkdir::WalkDir;

use crate::library_scan::{extract_track, is_audio_file, is_cloud_only};
use crate::DbState;

pub const CHECK_INTERVAL_SECS: u64 = 30;

/// Indexes audio files in every watched folder that aren't in the library yet.
/// Returns how many were added. Files already known are never re-read, so
/// checks stay cheap even for a large library.
pub fn index_watched_folders(app: &AppHandle) -> usize {
    let folders = {
        let db = app.state::<DbState>();
        let conn = match db.0.lock() {
            Ok(conn) => conn,
            Err(_) => return 0,
        };
        library::list_library_folders(&conn).unwrap_or_default()
    };

    folders.iter().map(|folder| index_folder(app, folder)).sum()
}

fn index_folder(app: &AppHandle, folder: &str) -> usize {
    let known = {
        let db = app.state::<DbState>();
        let conn = match db.0.lock() {
            Ok(conn) => conn,
            Err(_) => return 0,
        };
        library::known_track_paths(&conn).unwrap_or_default()
    };

    let mut added = 0;
    for entry in WalkDir::new(folder).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !entry.file_type().is_file() || !is_audio_file(path) {
            continue;
        }
        let path_string = path.to_string_lossy().into_owned();
        if known.contains(&path_string) {
            continue;
        }
        if index_file(app, path) {
            added += 1;
        }
    }
    added
}

fn index_file(app: &AppHandle, path: &Path) -> bool {
    // Cloud-only files are picked up once they've been downloaded locally.
    if is_cloud_only(path) {
        return false;
    }
    let Ok(new_track) = extract_track(path) else {
        return false;
    };
    let db = app.state::<DbState>();
    let Ok(conn) = db.0.lock() else {
        return false;
    };
    matches!(
        library::upsert_track(&conn, &new_track),
        Ok(UpsertOutcome::Inserted { .. }) | Ok(UpsertOutcome::InsertedDuplicate { .. })
    )
}

/// Keeps the library in step with the watched folders, in the background.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(CHECK_INTERVAL_SECS));
        // Traktor first: its import is quick, while indexing a large or
        // cloud-synced folder can take minutes.
        let mut changed = false;
        if let Some(summary) = crate::traktor::import_if_changed(&app) {
            changed |= summary.indexed > 0 || summary.tempo_applied > 0;
        }
        changed |= index_watched_folders(&app) > 0;
        if changed {
            let _ = app.emit("library-updated", ());
        }
    });
}

#[tauri::command]
pub fn list_watched_folders(db: tauri::State<DbState>) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    library::list_library_folders(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_watched_folder(db: tauri::State<DbState>, path: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    library::remove_library_folder(&conn, &path).map_err(|e| e.to_string())
}
