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
/// One app session (every launch is one), as a set: when it started, when
/// its last play happened, and how many tracks were played.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub session_id: i64,
    pub started_at: String,
    pub last_play_at: Option<String>,
    pub plays: i64,
}

pub fn list_sessions(conn: &Connection) -> Result<Vec<SessionSummary>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.date, MAX(p.played_at), COUNT(p.id) FROM gigs g \
         LEFT JOIN played_tracks p ON p.gig_id = g.id \
         WHERE g.name = 'Live session' GROUP BY g.id ORDER BY g.id DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SessionSummary {
                session_id: row.get(0)?,
                started_at: row.get(1)?,
                last_play_at: row.get(2)?,
                plays: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// A play inside a session, in the order it happened. `est_seconds` is how long
/// the track played, estimated as the gap until the next track started, capped
/// at the track's length. It is None for the last track, whose end isn't known.
#[derive(Debug, Clone)]
pub struct SetPlay {
    pub position: i64,
    pub track_id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub bpm: Option<f64>,
    pub camelot_key: Option<String>,
    pub played_at: String,
    pub source: String,
    pub mode: String,
    pub est_seconds: Option<i64>,
}

pub fn session_plays(conn: &Connection, session_id: i64) -> Result<Vec<SetPlay>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT p.position_in_set, p.track_id, t.title, t.artist, t.bpm, t.camelot_key, \
                p.played_at, p.source, p.mode, t.duration_ms, \
                CAST(strftime('%s', LEAD(p.played_at) OVER (ORDER BY p.position_in_set)) \
                     - strftime('%s', p.played_at) AS INTEGER) AS gap \
         FROM played_tracks p JOIN tracks t ON t.id = p.track_id \
         WHERE p.gig_id = ?1 ORDER BY p.position_in_set",
    )?;
    let rows = stmt
        .query_map(params![session_id], |row| {
            let duration_ms: Option<i64> = row.get(9)?;
            let gap: Option<i64> = row.get(10)?;
            let est_seconds = gap.map(|gap| match duration_ms {
                Some(ms) => gap.min(ms / 1000),
                None => gap,
            });
            Ok(SetPlay {
                position: row.get(0)?,
                track_id: row.get(1)?,
                title: row.get(2)?,
                artist: row.get(3)?,
                bpm: row.get(4)?,
                camelot_key: row.get(5)?,
                played_at: row.get(6)?,
                source: row.get(7)?,
                mode: row.get(8)?,
                est_seconds,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The tracks played most often across every session, most played first, as
/// (track_id, number of plays). Ties keep the track that was played first.
pub fn top_played(conn: &Connection, limit: i64) -> Result<Vec<(i64, i64)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT track_id, COUNT(*) AS plays FROM played_tracks \
         GROUP BY track_id ORDER BY plays DESC, MIN(played_at) ASC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

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

    #[test]
    fn top_played_counts_plays_per_track_across_sessions() {
        let conn = connect_in_memory().unwrap();
        let first = start_session(&conn).unwrap();
        let a = add_track(&conn, "a");
        let b = add_track(&conn, "b");
        record_play(&conn, first, a, None, "KEEP", None, false).unwrap();
        record_play(&conn, first, b, None, "KEEP", None, false).unwrap();
        let second = start_session(&conn).unwrap();
        record_play(&conn, second, b, None, "KEEP", None, false).unwrap();
        record_play(&conn, second, b, None, "KEEP", None, false).unwrap();

        let top = top_played(&conn, 10).unwrap();
        assert_eq!(top, vec![(b, 3), (a, 1)]);
        assert_eq!(top_played(&conn, 1).unwrap(), vec![(b, 3)]);
    }

    #[test]
    fn a_track_plays_until_the_next_one_starts_capped_at_its_length() {
        let conn = connect_in_memory().unwrap();
        let session = start_session(&conn).unwrap();
        let a = add_track(&conn, "a");
        let b = add_track(&conn, "b");
        conn.execute("UPDATE tracks SET duration_ms = 300000 WHERE id = ?1", params![a]).unwrap();
        conn.execute("UPDATE tracks SET duration_ms = 100000 WHERE id = ?1", params![b]).unwrap();
        record_play(&conn, session, a, None, "KEEP", None, false).unwrap();
        record_play(&conn, session, b, Some(a), "KEEP", None, false).unwrap();
        conn.execute(
            "UPDATE played_tracks SET played_at = '2026-10-06 20:00:00' WHERE track_id = ?1",
            params![a],
        )
        .unwrap();
        conn.execute(
            "UPDATE played_tracks SET played_at = '2026-10-06 20:02:00' WHERE track_id = ?1",
            params![b],
        )
        .unwrap();

        let plays = session_plays(&conn, session).unwrap();
        assert_eq!(plays[0].est_seconds, Some(120), "gap to the next track");
        assert_eq!(plays[1].est_seconds, None, "last track's end is unknown");

        conn.execute(
            "UPDATE played_tracks SET played_at = '2026-10-06 20:10:00' WHERE track_id = ?1",
            params![b],
        )
        .unwrap();
        let plays = session_plays(&conn, session).unwrap();
        assert_eq!(plays[0].est_seconds, Some(300), "capped at the track's length");
    }

    #[test]
    fn sessions_are_listed_newest_first_with_their_play_counts() {
        let conn = connect_in_memory().unwrap();
        let first = start_session(&conn).unwrap();
        let second = start_session(&conn).unwrap();
        let a = add_track(&conn, "a");
        record_play(&conn, second, a, None, "KEEP", None, false).unwrap();

        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions[0].session_id, second);
        assert_eq!(sessions[0].plays, 1);
        assert_eq!(sessions[1].session_id, first);
        assert_eq!(sessions[1].plays, 0);
    }
}
