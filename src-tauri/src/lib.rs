use std::sync::Mutex;
use tauri::Manager;

mod library_scan;

pub(crate) struct DbState(pub(crate) Mutex<rusqlite::Connection>);

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

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

            let db_path = data_dir.join("mixmedj.sqlite3");
            let conn = mixflow_db::connect(&db_path).expect("database should open and migrate");
            app.manage(DbState(Mutex::new(conn)));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            db_health_check,
            library_scan::pick_music_folder,
            library_scan::scan_library_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
