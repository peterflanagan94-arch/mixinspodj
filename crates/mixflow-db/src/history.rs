//! Traktor's own play history: each history file is one Traktor session, and
//! each play in it records how long the track actually played.

use rusqlite::{params, Connection, OptionalExtension};

use crate::DbError;

/// One play from a Traktor history file.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryPlay {
    pub start_code: i64,
    pub start_seconds: i64,
    pub played_seconds: f64,
    pub deck: i64,
    /// The file path Traktor recorded, already converted to a POSIX path.
    pub location: String,
    pub track_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct HistorySet {
    pub id: i64,
    pub file_name: String,
    pub plays: i64,
}

/// The stored mtime for a history file, if it has been imported before.
pub fn stored_mtime(conn: &Connection, file_name: &str) -> Result<Option<i64>, DbError> {
    conn.query_row(
        "SELECT file_mtime FROM traktor_history_sets WHERE file_name = ?1",
        params![file_name],
        |row| row.get(0),
    )
    .optional()
    .map_err(DbError::from)
}

/// Replaces a history file's plays with `plays`, in one transaction, so a
/// re-import of a changed file never leaves a mix of old and new plays.
pub fn replace_set(
    conn: &mut Connection,
    file_name: &str,
    file_mtime: i64,
    plays: &[HistoryPlay],
) -> Result<(), DbError> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO traktor_history_sets (file_name, file_mtime) VALUES (?1, ?2) \
         ON CONFLICT(file_name) DO UPDATE SET file_mtime = excluded.file_mtime",
        params![file_name, file_mtime],
    )?;
    let set_id: i64 = tx.query_row(
        "SELECT id FROM traktor_history_sets WHERE file_name = ?1",
        params![file_name],
        |row| row.get(0),
    )?;
    tx.execute("DELETE FROM traktor_history_plays WHERE set_id = ?1", params![set_id])?;
    for play in plays {
        tx.execute(
            "INSERT INTO traktor_history_plays \
             (set_id, start_code, start_seconds, played_seconds, deck, location, track_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                set_id,
                play.start_code,
                play.start_seconds,
                play.played_seconds,
                play.deck,
                play.location,
                play.track_id,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Every imported Traktor session, newest file name first (names carry the time).
pub fn list_sets(conn: &Connection) -> Result<Vec<HistorySet>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.file_name, COUNT(p.id) FROM traktor_history_sets s \
         LEFT JOIN traktor_history_plays p ON p.set_id = s.id \
         GROUP BY s.id ORDER BY s.file_name DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(HistorySet {
                id: row.get(0)?,
                file_name: row.get(1)?,
                plays: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The plays in one history file, in the order they started. Entries are ordered
/// by Traktor's start code (its day) and then by start time within the day.
pub fn set_plays(conn: &Connection, set_id: i64) -> Result<Vec<HistoryPlay>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT start_code, start_seconds, played_seconds, deck, location, track_id \
         FROM traktor_history_plays WHERE set_id = ?1 ORDER BY start_code, start_seconds",
    )?;
    let rows = stmt
        .query_map(params![set_id], |row| {
            Ok(HistoryPlay {
                start_code: row.get(0)?,
                start_seconds: row.get(1)?,
                played_seconds: row.get(2)?,
                deck: row.get(3)?,
                location: row.get(4)?,
                track_id: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect_in_memory;

    fn play(start_seconds: i64, played: f64, location: &str) -> HistoryPlay {
        HistoryPlay {
            start_code: 132778502,
            start_seconds,
            played_seconds: played,
            deck: 0,
            location: location.to_string(),
            track_id: None,
        }
    }

    #[test]
    fn a_set_keeps_its_plays_in_start_order_with_exact_playtimes() {
        let mut conn = connect_in_memory().unwrap();
        replace_set(
            &mut conn,
            "history_a.nml",
            100,
            &[play(200, 64.0, "/music/b.mp3"), play(100, 216.06, "/music/a.mp3")],
        )
        .unwrap();

        let set = list_sets(&conn).unwrap().remove(0);
        assert_eq!(set.plays, 2);
        let plays = set_plays(&conn, set.id).unwrap();
        assert_eq!(plays[0].location, "/music/a.mp3");
        assert_eq!(plays[0].played_seconds, 216.06);
        assert_eq!(plays[1].location, "/music/b.mp3");
    }

    #[test]
    fn reimporting_a_file_replaces_its_plays_instead_of_adding_to_them() {
        let mut conn = connect_in_memory().unwrap();
        replace_set(&mut conn, "history_a.nml", 100, &[play(100, 10.0, "/music/a.mp3")]).unwrap();
        replace_set(
            &mut conn,
            "history_a.nml",
            200,
            &[play(100, 10.0, "/music/a.mp3"), play(150, 20.0, "/music/b.mp3")],
        )
        .unwrap();

        assert_eq!(stored_mtime(&conn, "history_a.nml").unwrap(), Some(200));
        let set = list_sets(&conn).unwrap().remove(0);
        assert_eq!(set.plays, 2);
    }
}
