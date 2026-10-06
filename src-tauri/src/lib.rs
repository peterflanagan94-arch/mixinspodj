use std::sync::Mutex;
use tauri::Manager;

mod analysis;
mod broadcast;
mod library_scan;
mod plays;
mod recommend;
mod tracks;
mod watch;

pub(crate) struct DbState(pub(crate) Mutex<rusqlite::Connection>);

const BACKUP_INTERVAL_SECS: u64 = 30 * 60;
const BACKUPS_TO_KEEP: usize = 10;

/// Proves the SQLite connection + migrations are wired end-to-end from the
/// frontend. Not a long-term API — superseded once real library/gig
/// commands land in later milestones.
#[tauri::command]
fn db_health_check(state: tauri::State<DbState>) -> Result<String, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let sqlite_version: String = conn
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let table_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "connected (sqlite {sqlite_version}, {table_count} tables)"
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir should be resolvable");
            std::fs::create_dir_all(&data_dir).expect("should be able to create app data dir");

            let db_path = data_dir.join("mixinspodj.sqlite3");
            let backup_dir = data_dir.join("backups");
            let (conn, outcome) = mixflow_db::backup::open_with_recovery(&db_path, &backup_dir)
                .expect("database should open, or recover from a backup");
            match &outcome {
                mixflow_db::backup::OpenOutcome::Opened => {}
                mixflow_db::backup::OpenOutcome::RestoredFromBackup { from, damaged_moved_to } => {
                    eprintln!(
                        "library database was damaged; restored from {} (damaged copy kept at {})",
                        from.display(),
                        damaged_moved_to.display()
                    );
                }
                mixflow_db::backup::OpenOutcome::RecreatedEmpty { damaged_moved_to } => {
                    eprintln!(
                        "library database was damaged and no backup existed; starting empty (damaged copy kept at {})",
                        damaged_moved_to.display()
                    );
                }
            }

            mixflow_db::backup::write_backup(&conn, &backup_dir).expect("should write a startup backup");
            mixflow_db::backup::prune_backups(&backup_dir, BACKUPS_TO_KEEP).expect("should prune old backups");

            let session_id = mixflow_db::plays::start_session(&conn).expect("should start a live session");
            app.manage(DbState(Mutex::new(conn)));
            app.manage(plays::CurrentSession(session_id));
            broadcast::start(app.handle().clone());
            watch::start(app.handle().clone());

            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(BACKUP_INTERVAL_SECS));
                let db = handle.state::<DbState>();
                let guard = db.0.lock();
                if let Ok(conn) = guard {
                    let _ = mixflow_db::backup::write_backup(&conn, &backup_dir);
                    let _ = mixflow_db::backup::prune_backups(&backup_dir, BACKUPS_TO_KEEP);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            db_health_check,
            library_scan::pick_music_folder,
            library_scan::scan_library_folder,
            tracks::list_tracks,
            tracks::search_tracks,
            tracks::update_track_metadata,
            analysis::analyze_library,
            watch::list_watched_folders,
            watch::remove_watched_folder,
            recommend::recommend_next,
            plays::play_track
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
