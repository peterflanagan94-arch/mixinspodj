//! Local SQLite access for MixInspo DJ. The database is the source of truth
//! for everything MixInspo DJ infers about a library — source music files
//! are never modified unless the user explicitly opts in elsewhere in the
//! app.

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use std::path::Path;

pub mod backup;
pub mod library;
pub mod plays;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Migration(#[from] rusqlite_migration::Error),
    #[error("io error: {0}")]
    Io(String),
}

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("migrations/0001_init.sql")),
        M::up(include_str!("migrations/0002_tempo_source.sql")),
    ])
}

/// Opens (creating if necessary) the MixInspo DJ database at `path` and brings
/// its schema up to date.
pub fn connect(path: impl AsRef<Path>) -> Result<Connection, DbError> {
    let mut conn = Connection::open(path)?;
    // Wait briefly for a competing reader or writer (another tool, or the app's
    // own background threads) instead of failing the write straight away.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}

/// Opens an in-memory database with the schema applied. Used by tests and by
/// anything that wants a disposable database.
pub fn connect_in_memory() -> Result<Connection, DbError> {
    let mut conn = Connection::open_in_memory()?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        migrations().validate().expect("migration set should be internally consistent");
    }

    #[test]
    fn fresh_database_has_expected_tables() {
        let conn = connect_in_memory().expect("should open and migrate");

        let expected_tables = [
            "library_folders",
            "tracks",
            "audio_features",
            "duplicates",
            "gigs",
            "gig_tracklist",
            "played_tracks",
            "transitions",
            "requests",
            "scoring_weights",
            "feature_flags",
            "acquisition_log",
        ];

        for table in expected_tables {
            let count: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "expected table `{table}` to exist");
        }
    }

    #[test]
    fn can_insert_and_read_back_a_track() {
        let conn = connect_in_memory().unwrap();
        conn.execute(
            "INSERT INTO tracks (file_path, title, artist, bpm, camelot_key) \
             VALUES ('/music/song.flac', 'September', 'Earth, Wind & Fire', 126.0, '4A')",
            [],
        )
        .unwrap();

        let title: String = conn
            .query_row(
                "SELECT title FROM tracks WHERE file_path = '/music/song.flac'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "September");
    }
}
