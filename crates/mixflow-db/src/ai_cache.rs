//! AI Suggestions results, cached by song so the same track is never sent to
//! the AI twice on this machine. The cache stores whatever JSON the caller
//! gives it — this crate doesn't know the shape of an AI suggestion, only
//! how to keep one.

use rusqlite::{params, Connection, OptionalExtension};

use crate::DbError;

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

/// The cached response for a song, if one's been asked for before.
pub fn get(conn: &Connection, title: &str, artist: &str) -> Result<Option<String>, DbError> {
    conn.query_row(
        "SELECT response_json FROM ai_suggestion_cache WHERE title_key = ?1 AND artist_key = ?2",
        params![normalize(title), normalize(artist)],
        |row| row.get(0),
    )
    .optional()
    .map_err(DbError::from)
}

/// Stores (or replaces) the response for a song.
pub fn set(conn: &Connection, title: &str, artist: &str, response_json: &str) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO ai_suggestion_cache (title_key, artist_key, response_json) VALUES (?1, ?2, ?3) \
         ON CONFLICT(title_key, artist_key) DO UPDATE SET \
            response_json = excluded.response_json, created_at = datetime('now')",
        params![normalize(title), normalize(artist), response_json],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect_in_memory;

    #[test]
    fn a_song_with_no_cached_response_reads_as_none() {
        let conn = connect_in_memory().unwrap();
        assert_eq!(get(&conn, "Waka Waka", "Shakira").unwrap(), None);
    }

    #[test]
    fn a_stored_response_is_read_back() {
        let conn = connect_in_memory().unwrap();
        set(&conn, "Waka Waka", "Shakira", "[1,2,3]").unwrap();
        assert_eq!(get(&conn, "Waka Waka", "Shakira").unwrap().as_deref(), Some("[1,2,3]"));
    }

    #[test]
    fn lookup_ignores_case_and_surrounding_space() {
        let conn = connect_in_memory().unwrap();
        set(&conn, "  Waka Waka  ", "Shakira", "[1]").unwrap();
        assert_eq!(get(&conn, "waka waka", "SHAKIRA").unwrap().as_deref(), Some("[1]"));
    }

    #[test]
    fn storing_again_for_the_same_song_replaces_the_old_response() {
        let conn = connect_in_memory().unwrap();
        set(&conn, "Waka Waka", "Shakira", "[1]").unwrap();
        set(&conn, "Waka Waka", "Shakira", "[2]").unwrap();
        assert_eq!(get(&conn, "Waka Waka", "Shakira").unwrap().as_deref(), Some("[2]"));
    }
}
