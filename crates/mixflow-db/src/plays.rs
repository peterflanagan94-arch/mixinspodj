//! Play history for the current live session: which tracks were played, and
//! which transitions were made between them. This is the learning data the
//! recommendation engine draws on, so it's recorded from the start.

use crate::DbError;
use rusqlite::{params, Connection};
use std::collections::HashMap;

/// Starts a new live session (a `gigs` row marked in progress) and closes
/// any session still marked in progress from a previous launch.
pub fn start_session(conn: &Connection) -> Result<i64, DbError> {
    conn.execute(
        "UPDATE gigs SET status = 'completed' WHERE status = 'in_progress'",
        [],
    )?;
    conn.execute(
        "INSERT INTO gigs (name, date, status) VALUES ('Live session', datetime('now', 'localtime'), 'in_progress')",
        [],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Records a track becoming NOW PLAYING. `rank` is the track's position in
/// the recommendations the DJ was looking at, if it came from there — that's
/// what makes a play "recommended" rather than "manual," and it's the signal
/// the learning side needs to see whether suggestions get used.
pub fn record_play(
    conn: &Connection,
    session_id: i64,
    track_id: i64,
    previous_track_id: Option<i64>,
    mode: &str,
    rank: Option<i64>,
    detected: bool,
) -> Result<(), DbError> {
    let existing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM played_tracks WHERE gig_id = ?1",
        params![session_id],
        |row| row.get(0),
    )?;
    let position = existing + 1;
    // A track Traktor reported playing was never picked here, so it's
    // "detected" even if it also sat in the suggestions.
    let source = if detected {
        "detected"
    } else if rank.is_some() {
        "recommended"
    } else {
        "manual"
    };
    let recommended = rank.is_some();

    conn.execute(
        "INSERT INTO played_tracks (gig_id, track_id, played_at, position_in_set, source, mode) \
         VALUES (?1, ?2, datetime('now', 'localtime'), ?3, ?4, ?5)",
        params![session_id, track_id, position, source, mode],
    )?;

    if let Some(from_track_id) = previous_track_id {
        conn.execute(
            "INSERT INTO transitions \
             (from_track_id, to_track_id, gig_id, played_at, event_type, was_recommended, \
              recommendation_rank, was_selected_recommendation, mode) \
             VALUES (?1, ?2, ?3, datetime('now', 'localtime'), NULL, ?4, ?5, ?4, ?6)",
            params![from_track_id, track_id, session_id, recommended, rank, mode],
        )?;
    }
    Ok(())
}

/// Tracks played in this session, mapped to when they were last played (local time).
pub fn played_in_session(conn: &Connection, session_id: i64) -> Result<HashMap<i64, String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT track_id, MAX(played_at) FROM played_tracks WHERE gig_id = ?1 GROUP BY track_id",
    )?;
    let rows = stmt
        .query_map(params![session_id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(rows)
}

/// How many times, across all sessions, each track has followed `from_track_id`.
pub fn transition_counts_from(conn: &Connection, from_track_id: i64) -> Result<HashMap<i64, u32>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT to_track_id, COUNT(*) FROM transitions WHERE from_track_id = ?1 GROUP BY to_track_id",
    )?;
    let rows = stmt
        .query_map(params![from_track_id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, u32>(1)?)))?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect_in_memory;
    use crate::library::{upsert_track, NewTrack, UpsertOutcome};

    fn add_track(conn: &Connection, name: &str) -> i64 {
        let track = NewTrack {
            file_path: format!("/music/{name}.flac"),
            file_hash: format!("hash-{name}"),
            ..Default::default()
        };
        match upsert_track(conn, &track).unwrap() {
            UpsertOutcome::Inserted { track_id } => track_id,
            other => panic!("expected Inserted, got {other:?}"),
        }
    }

    #[test]
    fn starting_a_session_closes_any_previous_one() {
        let conn = connect_in_memory().unwrap();
        let first = start_session(&conn).unwrap();
        let second = start_session(&conn).unwrap();
        assert_ne!(first, second);

        let first_status: String = conn
            .query_row("SELECT status FROM gigs WHERE id = ?1", params![first], |r| r.get(0))
            .unwrap();
        assert_eq!(first_status, "completed");
    }

    #[test]
    fn a_play_is_recorded_with_its_position_and_source() {
        let conn = connect_in_memory().unwrap();
        let session = start_session(&conn).unwrap();
        let a = add_track(&conn, "a");
        let b = add_track(&conn, "b");

        record_play(&conn, session, a, None, "KEEP", None, false).unwrap();
        record_play(&conn, session, b, Some(a), "BUILD", Some(2), false).unwrap();

        let (position, source): (i64, String) = conn
            .query_row(
                "SELECT position_in_set, source FROM played_tracks WHERE track_id = ?1",
                params![b],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(position, 2);
        assert_eq!(source, "recommended");
    }

    #[test]
    fn a_track_traktor_reports_is_recorded_as_detected() {
        let conn = connect_in_memory().unwrap();
        let session = start_session(&conn).unwrap();
        let a = add_track(&conn, "a");

        record_play(&conn, session, a, None, "KEEP", Some(1), true).unwrap();

        let source: String = conn
            .query_row("SELECT source FROM played_tracks WHERE track_id = ?1", params![a], |r| r.get(0))
            .unwrap();
        assert_eq!(source, "detected");
    }

    #[test]
    fn a_transition_is_recorded_between_consecutive_plays() {
        let conn = connect_in_memory().unwrap();
        let session = start_session(&conn).unwrap();
        let a = add_track(&conn, "a");
        let b = add_track(&conn, "b");

        record_play(&conn, session, a, None, "KEEP", None, false).unwrap();
        record_play(&conn, session, b, Some(a), "KEEP", Some(1), false).unwrap();

        let counts = transition_counts_from(&conn, a).unwrap();
        assert_eq!(counts.get(&b), Some(&1));
    }

    #[test]
    fn played_in_session_reports_each_track_once_with_its_time() {
        let conn = connect_in_memory().unwrap();
        let session = start_session(&conn).unwrap();
        let a = add_track(&conn, "a");
        record_play(&conn, session, a, None, "KEEP", None, false).unwrap();
        record_play(&conn, session, a, None, "KEEP", None, false).unwrap();

        let played = played_in_session(&conn, session).unwrap();
        assert_eq!(played.len(), 1);
        assert!(played.contains_key(&a));
    }
}
