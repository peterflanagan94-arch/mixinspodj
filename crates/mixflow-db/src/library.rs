//! Repository functions for library folders and tracks. Scanning and
//! metadata extraction themselves live in the Tauri app (they need
//! filesystem + tag-parsing crates that don't belong in a DB-access crate);
//! this module is just the data layer they write through.

use crate::DbError;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, Default)]
pub struct NewTrack {
    pub file_path: String,
    pub file_hash: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i32>,
    pub sample_rate: Option<i32>,
    pub file_format: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpsertOutcome {
    /// New file, no existing track shares its content hash.
    Inserted { track_id: i64 },
    /// A track already existed at this exact path; its metadata was refreshed.
    Updated { track_id: i64 },
    /// New file, but another track (different path) has identical content —
    /// recorded in `duplicates` for the user to resolve.
    InsertedDuplicate { track_id: i64, duplicate_of_path: String },
}

pub fn upsert_library_folder(conn: &Connection, path: &str) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO library_folders (path, last_scanned_at) VALUES (?1, datetime('now'))
         ON CONFLICT(path) DO UPDATE SET last_scanned_at = excluded.last_scanned_at",
        params![path],
    )?;
    Ok(())
}

pub fn upsert_track(conn: &Connection, track: &NewTrack) -> Result<UpsertOutcome, DbError> {
    if let Some(existing_id) = find_track_id_by_path(conn, &track.file_path)? {
        conn.execute(
            "UPDATE tracks SET file_hash = ?1, title = ?2, artist = ?3, album = ?4, \
             genre = ?5, year = ?6, duration_ms = ?7, bitrate = ?8, sample_rate = ?9, \
             file_format = ?10, missing_file = 0 \
             WHERE id = ?11",
            params![
                track.file_hash,
                track.title,
                track.artist,
                track.album,
                track.genre,
                track.year,
                track.duration_ms,
                track.bitrate,
                track.sample_rate,
                track.file_format,
                existing_id,
            ],
        )?;
        return Ok(UpsertOutcome::Updated { track_id: existing_id });
    }

    let duplicate_of_path = find_track_path_by_hash(conn, &track.file_hash)?;

    conn.execute(
        "INSERT INTO tracks \
         (file_path, file_hash, title, artist, album, genre, year, duration_ms, \
          bitrate, sample_rate, file_format) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            track.file_path,
            track.file_hash,
            track.title,
            track.artist,
            track.album,
            track.genre,
            track.year,
            track.duration_ms,
            track.bitrate,
            track.sample_rate,
            track.file_format,
        ],
    )?;
    let new_id = conn.last_insert_rowid();

    if let Some(duplicate_of_path) = duplicate_of_path {
        if let Some(existing_id) = find_track_id_by_path(conn, &duplicate_of_path)? {
            conn.execute(
                "INSERT INTO duplicates (track_id_a, track_id_b, similarity_score, status) \
                 VALUES (?1, ?2, 1.0, 'possible')",
                params![existing_id, new_id],
            )?;
        }
        return Ok(UpsertOutcome::InsertedDuplicate {
            track_id: new_id,
            duplicate_of_path,
        });
    }

    Ok(UpsertOutcome::Inserted { track_id: new_id })
}

fn find_track_id_by_path(conn: &Connection, file_path: &str) -> Result<Option<i64>, DbError> {
    conn.query_row(
        "SELECT id FROM tracks WHERE file_path = ?1",
        params![file_path],
        |row| row.get(0),
    )
    .optional()
    .map_err(DbError::from)
}

fn find_track_path_by_hash(conn: &Connection, file_hash: &str) -> Result<Option<String>, DbError> {
    conn.query_row(
        "SELECT file_path FROM tracks WHERE file_hash = ?1 LIMIT 1",
        params![file_hash],
        |row| row.get(0),
    )
    .optional()
    .map_err(DbError::from)
}

pub fn count_tracks(conn: &Connection) -> Result<i64, DbError> {
    conn.query_row("SELECT count(*) FROM tracks", [], |row| row.get(0))
        .map_err(DbError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect_in_memory;

    fn sample(path: &str, hash: &str) -> NewTrack {
        NewTrack {
            file_path: path.into(),
            file_hash: hash.into(),
            title: Some("September".into()),
            artist: Some("Earth, Wind & Fire".into()),
            ..Default::default()
        }
    }

    #[test]
    fn first_insert_is_inserted() {
        let conn = connect_in_memory().unwrap();
        let outcome = upsert_track(&conn, &sample("/music/a.flac", "hash-a")).unwrap();
        assert!(matches!(outcome, UpsertOutcome::Inserted { .. }));
        assert_eq!(count_tracks(&conn).unwrap(), 1);
    }

    #[test]
    fn rescanning_same_path_updates_instead_of_duplicating() {
        let conn = connect_in_memory().unwrap();
        upsert_track(&conn, &sample("/music/a.flac", "hash-a")).unwrap();
        let outcome = upsert_track(&conn, &sample("/music/a.flac", "hash-a-changed")).unwrap();
        assert!(matches!(outcome, UpsertOutcome::Updated { .. }));
        assert_eq!(count_tracks(&conn).unwrap(), 1);
    }

    #[test]
    fn same_content_at_new_path_is_flagged_as_duplicate() {
        let conn = connect_in_memory().unwrap();
        upsert_track(&conn, &sample("/music/a.flac", "hash-a")).unwrap();
        let outcome = upsert_track(&conn, &sample("/music/backup/a.flac", "hash-a")).unwrap();
        match outcome {
            UpsertOutcome::InsertedDuplicate { duplicate_of_path, .. } => {
                assert_eq!(duplicate_of_path, "/music/a.flac");
            }
            other => panic!("expected InsertedDuplicate, got {other:?}"),
        }
        assert_eq!(count_tracks(&conn).unwrap(), 2);

        let dup_rows: i64 = conn
            .query_row("SELECT count(*) FROM duplicates", [], |row| row.get(0))
            .unwrap();
        assert_eq!(dup_rows, 1);
    }
}
