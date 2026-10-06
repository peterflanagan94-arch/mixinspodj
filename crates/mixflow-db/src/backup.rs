//! Local backups and crash recovery for the library database.
//!
//! Backups use `VACUUM INTO`, which writes a consistent copy of a live
//! database without stopping the app. On startup the database is checked
//! with `quick_check`; if it's damaged, the damaged file is set aside and the
//! newest backup is restored in its place, so the app never starts on a
//! broken database without a trace of what happened.

use crate::{connect, DbError};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BACKUP_PREFIX: &str = "backup-";
const BACKUP_SUFFIX: &str = ".sqlite3";

#[derive(Debug)]
pub enum OpenOutcome {
    Opened,
    RestoredFromBackup { from: PathBuf, damaged_moved_to: PathBuf },
    RecreatedEmpty { damaged_moved_to: PathBuf },
}

/// Writes a consistent copy of `conn` into `backup_dir` and returns its path.
pub fn write_backup(conn: &Connection, backup_dir: &Path) -> Result<PathBuf, DbError> {
    std::fs::create_dir_all(backup_dir).map_err(|e| DbError::Io(e.to_string()))?;
    let path = unused_backup_path(backup_dir);
    conn.execute("VACUUM INTO ?1", [path.to_string_lossy().as_ref()])?;
    Ok(path)
}

/// Keeps the newest `keep` backups and deletes the rest.
pub fn prune_backups(backup_dir: &Path, keep: usize) -> Result<(), DbError> {
    let backups = list_backups(backup_dir);
    let excess = backups.len().saturating_sub(keep);
    for old in backups.iter().take(excess) {
        std::fs::remove_file(old).map_err(|e| DbError::Io(e.to_string()))?;
    }
    Ok(())
}

pub fn latest_backup(backup_dir: &Path) -> Option<PathBuf> {
    list_backups(backup_dir).pop()
}

/// Opens the database at `db_path`. If it fails `quick_check`, moves it aside
/// and restores the newest backup (or starts empty if there isn't one).
pub fn open_with_recovery(db_path: &Path, backup_dir: &Path) -> Result<(Connection, OpenOutcome), DbError> {
    if db_path.exists() && !is_healthy_file(db_path) {
        let damaged = db_path.with_file_name(format!("damaged-{}.sqlite3", timestamp_millis()));
        std::fs::rename(db_path, &damaged).map_err(|e| DbError::Io(e.to_string()))?;

        if let Some(backup) = latest_backup(backup_dir) {
            std::fs::copy(&backup, db_path).map_err(|e| DbError::Io(e.to_string()))?;
            let conn = connect(db_path)?;
            return Ok((
                conn,
                OpenOutcome::RestoredFromBackup {
                    from: backup,
                    damaged_moved_to: damaged,
                },
            ));
        }

        let conn = connect(db_path)?;
        return Ok((conn, OpenOutcome::RecreatedEmpty { damaged_moved_to: damaged }));
    }

    Ok((connect(db_path)?, OpenOutcome::Opened))
}

fn is_healthy_file(db_path: &Path) -> bool {
    match Connection::open(db_path) {
        Ok(conn) => is_healthy(&conn),
        Err(_) => false,
    }
}

fn is_healthy(conn: &Connection) -> bool {
    conn.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .map(|result| result == "ok")
        .unwrap_or(false)
}

fn list_backups(backup_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(backup_dir) else {
        return Vec::new();
    };
    let mut backups: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            let name_matches = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(BACKUP_PREFIX) && n.ends_with(BACKUP_SUFFIX))
                .unwrap_or(false);
            // A zero-byte file can't be a backup, and restoring one would wipe the library.
            name_matches && std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false)
        })
        .collect();
    // Names carry a fixed-width millisecond timestamp, so lexical order is time order.
    backups.sort();
    backups
}

fn unused_backup_path(backup_dir: &Path) -> PathBuf {
    loop {
        let candidate = backup_dir.join(format!("{BACKUP_PREFIX}{:013}{BACKUP_SUFFIX}", timestamp_millis()));
        if !candidate.exists() {
            return candidate;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{upsert_track, NewTrack};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mixinspo-backup-test-{name}-{}", timestamp_millis()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn add_track(conn: &Connection, name: &str) {
        upsert_track(
            conn,
            &NewTrack {
                file_path: format!("/music/{name}.flac"),
                file_hash: format!("hash-{name}"),
                ..Default::default()
            },
        )
        .unwrap();
    }

    fn track_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT count(*) FROM tracks", [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn a_backup_contains_the_data_and_is_listed() {
        let dir = temp_dir("write");
        let db_path = dir.join("live.sqlite3");
        let conn = connect(&db_path).unwrap();
        add_track(&conn, "a");

        let backup = write_backup(&conn, &dir.join("backups")).unwrap();
        assert_eq!(latest_backup(&dir.join("backups")), Some(backup.clone()));

        let restored = Connection::open(&backup).unwrap();
        assert_eq!(track_count(&restored), 1);
    }

    #[test]
    fn pruning_keeps_only_the_newest_backups() {
        let dir = temp_dir("prune");
        let backup_dir = dir.join("backups");
        let conn = connect(&dir.join("live.sqlite3")).unwrap();
        for _ in 0..4 {
            write_backup(&conn, &backup_dir).unwrap();
        }
        prune_backups(&backup_dir, 2).unwrap();
        assert_eq!(list_backups(&backup_dir).len(), 2);
    }

    #[test]
    fn an_empty_file_is_never_treated_as_a_backup() {
        let dir = temp_dir("empty-file");
        let backup_dir = dir.join("backups");
        std::fs::create_dir_all(&backup_dir).unwrap();
        std::fs::write(backup_dir.join("backup-*.sqlite3"), b"").unwrap();
        assert!(latest_backup(&backup_dir).is_none());
    }

    #[test]
    fn a_healthy_database_opens_without_recovery() {
        let dir = temp_dir("healthy");
        let db_path = dir.join("live.sqlite3");
        {
            let conn = connect(&db_path).unwrap();
            add_track(&conn, "a");
        }
        let (conn, outcome) = open_with_recovery(&db_path, &dir.join("backups")).unwrap();
        assert!(matches!(outcome, OpenOutcome::Opened));
        assert_eq!(track_count(&conn), 1);
    }

    #[test]
    fn a_damaged_database_is_restored_from_the_latest_backup() {
        let dir = temp_dir("restore");
        let db_path = dir.join("live.sqlite3");
        let backup_dir = dir.join("backups");
        {
            let conn = connect(&db_path).unwrap();
            add_track(&conn, "a");
            write_backup(&conn, &backup_dir).unwrap();
        }
        std::fs::write(&db_path, b"this is not a database at all, just noise").unwrap();

        let (conn, outcome) = open_with_recovery(&db_path, &backup_dir).unwrap();
        match outcome {
            OpenOutcome::RestoredFromBackup { damaged_moved_to, .. } => {
                assert!(damaged_moved_to.exists(), "damaged file should be kept, not deleted");
            }
            other => panic!("expected RestoredFromBackup, got {other:?}"),
        }
        assert_eq!(track_count(&conn), 1);
    }

    #[test]
    fn a_damaged_database_with_no_backup_starts_empty_and_keeps_the_damaged_file() {
        let dir = temp_dir("empty");
        let db_path = dir.join("live.sqlite3");
        std::fs::write(&db_path, b"garbage garbage garbage").unwrap();

        let (conn, outcome) = open_with_recovery(&db_path, &dir.join("backups")).unwrap();
        match outcome {
            OpenOutcome::RecreatedEmpty { damaged_moved_to } => assert!(damaged_moved_to.exists()),
            other => panic!("expected RecreatedEmpty, got {other:?}"),
        }
        assert_eq!(track_count(&conn), 0);
    }
}
