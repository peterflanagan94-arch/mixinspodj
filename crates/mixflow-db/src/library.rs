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

/// Marks every track previously recorded under `folder` as missing. Call
/// this before re-scanning that folder — each file actually found during
/// the scan clears the flag again via [`upsert_track`], so whatever's
/// still marked missing afterward genuinely wasn't found this time
/// (moved, renamed, or deleted outside the app).
pub fn mark_folder_tracks_missing(conn: &Connection, folder: &str) -> Result<(), DbError> {
    let prefix = format!("{folder}%");
    conn.execute(
        "UPDATE tracks SET missing_file = 1 WHERE file_path LIKE ?1",
        params![prefix],
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

pub fn find_track_id_by_path(conn: &Connection, file_path: &str) -> Result<Option<i64>, DbError> {
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

/// Folders the app watches for new music, in the order they were added.
pub fn list_library_folders(conn: &Connection) -> Result<Vec<String>, DbError> {
    let mut stmt = conn.prepare("SELECT path FROM library_folders ORDER BY id")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Stops watching a folder. Music files and their library entries are left as they are.
pub fn remove_library_folder(conn: &Connection, path: &str) -> Result<(), DbError> {
    conn.execute("DELETE FROM library_folders WHERE path = ?1", params![path])?;
    Ok(())
}

/// Every file path already in the library, so a watcher can spot new files cheaply.
pub fn known_track_paths(conn: &Connection) -> Result<std::collections::HashSet<String>, DbError> {
    let mut stmt = conn.prepare("SELECT file_path FROM tracks")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<std::collections::HashSet<_>, _>>()?;
    Ok(rows)
}

pub fn count_tracks(conn: &Connection) -> Result<i64, DbError> {
    conn.query_row("SELECT count(*) FROM tracks", [], |row| row.get(0))
        .map_err(DbError::from)
}

#[derive(Debug, Clone, Default)]
pub struct MetadataEdit {
    pub track_id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
}

/// Manual metadata correction. The caller is expected to send the full
/// current+edited field set (the UI pre-fills a form with current values)
/// — a `None` here means "clear this field," not "leave unchanged."
pub fn update_metadata(conn: &Connection, edit: &MetadataEdit) -> Result<(), DbError> {
    conn.execute(
        "UPDATE tracks SET title = ?1, artist = ?2, album = ?3, genre = ?4, year = ?5 \
         WHERE id = ?6",
        params![edit.title, edit.artist, edit.album, edit.genre, edit.year, edit.track_id],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct TrackForAnalysis {
    pub id: i64,
    pub file_path: String,
}

/// Tracks that have never been analyzed (`analyzed_at IS NULL`) and whose
/// file wasn't flagged missing on a previous scan.
pub fn tracks_needing_analysis(conn: &Connection) -> Result<Vec<TrackForAnalysis>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, file_path FROM tracks WHERE analyzed_at IS NULL AND missing_file = 0 \
         ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TrackForAnalysis {
                id: row.get(0)?,
                file_path: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Sets BPM, key and Camelot code from Traktor's collection. These replace
/// any estimate and are marked so later analysis leaves them alone.
/// `key` is (key name, Camelot code) when Traktor has one; a track with only a
/// BPM keeps analysis's key until analysis fills it in.
pub fn apply_traktor_tempo(
    conn: &Connection,
    track_id: i64,
    bpm: f32,
    key: Option<(&str, &str)>,
) -> Result<(), DbError> {
    match key {
        Some((key_name, camelot_key)) => conn.execute(
            "UPDATE tracks SET bpm = ?1, key = ?2, camelot_key = ?3, tempo_source = 'traktor' \
             WHERE id = ?4",
            params![bpm, key_name, camelot_key, track_id],
        )?,
        None => conn.execute(
            "UPDATE tracks SET bpm = ?1, tempo_source = 'traktor' WHERE id = ?2",
            params![bpm, track_id],
        )?,
    };
    Ok(())
}

/// The next track still waiting for analysis, skipping any ids in `exclude`
/// (tracks that already failed this session, so they aren't retried forever).
pub fn next_track_needing_analysis(
    conn: &Connection,
    exclude: &std::collections::HashSet<i64>,
) -> Result<Option<TrackForAnalysis>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, file_path FROM tracks WHERE analyzed_at IS NULL AND missing_file = 0 \
         ORDER BY id",
    )?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        if !exclude.contains(&id) {
            return Ok(Some(TrackForAnalysis {
                id,
                file_path: row.get(1)?,
            }));
        }
    }
    Ok(None)
}

#[derive(Debug, Clone)]
pub struct AnalysisUpdate {
    pub track_id: i64,
    pub bpm: f32,
    pub bpm_confidence: f32,
    pub key: String,
    pub camelot_key: String,
    pub energy: f32,
    pub danceability: f32,
    pub loudness_rms: f32,
    pub spectral_centroid: f32,
    pub onset_density: f32,
}

/// Persists a completed analysis: the headline fields on `tracks`, plus
/// the raw components on `audio_features` for future tuning/debugging.
/// Deliberately takes plain primitives rather than a `mixflow-analysis`
/// type — this crate has no DSP dependency, and shouldn't need one.
pub fn save_analysis(conn: &Connection, update: &AnalysisUpdate) -> Result<(), DbError> {
    // Tempo and key already imported from Traktor stay put: the DJ's own
    // analysis is only an estimate, and Traktor's values are the ones the
    // DJ actually mixes by.
    conn.execute(
        "UPDATE tracks SET \
            bpm = CASE WHEN tempo_source = 'traktor' THEN bpm ELSE ?1 END, \
            bpm_confidence = ?2, \
            key = CASE WHEN tempo_source = 'traktor' AND key IS NOT NULL THEN key ELSE ?3 END, \
            camelot_key = CASE WHEN tempo_source = 'traktor' AND camelot_key IS NOT NULL \
                THEN camelot_key ELSE ?4 END, \
            energy = ?5, danceability = ?6, analyzed_at = datetime('now') \
         WHERE id = ?7",
        params![
            update.bpm,
            update.bpm_confidence,
            update.key,
            update.camelot_key,
            update.energy,
            update.danceability,
            update.track_id,
        ],
    )?;
    conn.execute(
        "INSERT INTO audio_features (track_id, loudness_rms, spectral_centroid, onset_density) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(track_id) DO UPDATE SET \
            loudness_rms = excluded.loudness_rms, \
            spectral_centroid = excluded.spectral_centroid, \
            onset_density = excluded.onset_density",
        params![
            update.track_id,
            update.loudness_rms,
            update.spectral_centroid,
            update.onset_density,
        ],
    )?;
    Ok(())
}

/// Finds a library track by artist and title, ignoring case and surrounding
/// whitespace. Used to match what Traktor reports against the user's own files.
pub fn find_track_by_artist_title(conn: &Connection, artist: &str, title: &str) -> Result<Option<i64>, DbError> {
    conn.query_row(
        "SELECT id FROM tracks WHERE lower(trim(artist)) = lower(trim(?1)) \
         AND lower(trim(title)) = lower(trim(?2)) AND missing_file = 0 LIMIT 1",
        params![artist, title],
        |row| row.get(0),
    )
    .optional()
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

    #[test]
    fn unanalyzed_tracks_are_listed_for_analysis() {
        let conn = connect_in_memory().unwrap();
        upsert_track(&conn, &sample("/music/a.flac", "hash-a")).unwrap();
        upsert_track(&conn, &sample("/music/b.flac", "hash-b")).unwrap();

        let pending = tracks_needing_analysis(&conn).unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn saved_analysis_is_excluded_from_future_pending_lists() {
        let conn = connect_in_memory().unwrap();
        let track_id = match upsert_track(&conn, &sample("/music/a.flac", "hash-a")).unwrap() {
            UpsertOutcome::Inserted { track_id } => track_id,
            other => panic!("expected Inserted, got {other:?}"),
        };

        save_analysis(
            &conn,
            &AnalysisUpdate {
                track_id,
                bpm: 126.0,
                bpm_confidence: 0.8,
                key: "C".into(),
                camelot_key: "8B".into(),
                energy: 7.0,
                danceability: 0.75,
                loudness_rms: 0.2,
                spectral_centroid: 2000.0,
                onset_density: 3.0,
            },
        )
        .unwrap();

        let pending = tracks_needing_analysis(&conn).unwrap();
        assert!(pending.is_empty());

        let (bpm, camelot_key): (f64, String) = conn
            .query_row(
                "SELECT bpm, camelot_key FROM tracks WHERE id = ?1",
                params![track_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(bpm, 126.0);
        assert_eq!(camelot_key, "8B");

        let feature_rows: i64 = conn
            .query_row("SELECT count(*) FROM audio_features", [], |row| row.get(0))
            .unwrap();
        assert_eq!(feature_rows, 1);
    }

    #[test]
    fn manual_edit_overwrites_scanned_metadata() {
        let conn = connect_in_memory().unwrap();
        let track_id = match upsert_track(&conn, &sample("/music/a.flac", "hash-a")).unwrap() {
            UpsertOutcome::Inserted { track_id } => track_id,
            other => panic!("expected Inserted, got {other:?}"),
        };

        update_metadata(
            &conn,
            &MetadataEdit {
                track_id,
                title: Some("September (Corrected)".into()),
                artist: Some("Earth, Wind & Fire".into()),
                album: Some("The Best Of".into()),
                genre: Some("Disco".into()),
                year: Some(1978),
            },
        )
        .unwrap();

        let (title, genre, year): (String, String, i32) = conn
            .query_row(
                "SELECT title, genre, year FROM tracks WHERE id = ?1",
                params![track_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(title, "September (Corrected)");
        assert_eq!(genre, "Disco");
        assert_eq!(year, 1978);
    }

    #[test]
    fn rescanning_a_folder_flags_files_no_longer_found_as_missing() {
        let conn = connect_in_memory().unwrap();
        upsert_track(&conn, &sample("/music/folder/a.flac", "hash-a")).unwrap();
        upsert_track(&conn, &sample("/music/folder/b.flac", "hash-b")).unwrap();

        // Simulate a re-scan where only a.flac is found this time (b.flac
        // was deleted/moved outside the app): mark everything missing,
        // then "find" only a.flac again.
        mark_folder_tracks_missing(&conn, "/music/folder").unwrap();
        upsert_track(&conn, &sample("/music/folder/a.flac", "hash-a")).unwrap();

        let missing_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM tracks WHERE missing_file = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(missing_count, 1);

        let a_missing: i64 = conn
            .query_row(
                "SELECT missing_file FROM tracks WHERE file_path = '/music/folder/a.flac'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(a_missing, 0);
    }

    #[test]
    fn a_detected_track_is_matched_ignoring_case_and_spacing() {
        let conn = connect_in_memory().unwrap();
        let track_id = match upsert_track(
            &conn,
            &NewTrack {
                file_path: "/music/diafonie.flac".into(),
                file_hash: "hash-d".into(),
                title: Some("Diafonie".into()),
                artist: Some("Higher Place".into()),
                ..Default::default()
            },
        )
        .unwrap()
        {
            UpsertOutcome::Inserted { track_id } => track_id,
            other => panic!("expected Inserted, got {other:?}"),
        };

        assert_eq!(find_track_by_artist_title(&conn, " higher place ", "DIAFONIE").unwrap(), Some(track_id));
        assert_eq!(find_track_by_artist_title(&conn, "Someone Else", "Diafonie").unwrap(), None);
    }

    #[test]
    fn watched_folders_can_be_listed_and_removed_without_touching_tracks() {
        let conn = connect_in_memory().unwrap();
        upsert_library_folder(&conn, "/music/a").unwrap();
        upsert_library_folder(&conn, "/music/b").unwrap();
        upsert_track(&conn, &sample("/music/a/x.flac", "hash-x")).unwrap();

        assert_eq!(list_library_folders(&conn).unwrap(), vec!["/music/a", "/music/b"]);
        remove_library_folder(&conn, "/music/a").unwrap();
        assert_eq!(list_library_folders(&conn).unwrap(), vec!["/music/b"]);
        assert_eq!(count_tracks(&conn).unwrap(), 1);
        assert!(known_track_paths(&conn).unwrap().contains("/music/a/x.flac"));
    }
}

#[cfg(test)]
mod tempo_tests {
    use super::*;
    use crate::connect_in_memory;
    use std::collections::HashSet;

    fn insert(conn: &Connection, path: &str, hash: &str) -> i64 {
        let track = NewTrack {
            file_path: path.to_string(),
            file_hash: hash.to_string(),
            title: None,
            artist: None,
            album: None,
            genre: None,
            year: None,
            duration_ms: None,
            bitrate: None,
            sample_rate: None,
            file_format: Some("mp3".to_string()),
        };
        match upsert_track(conn, &track).unwrap() {
            UpsertOutcome::Inserted { track_id } => track_id,
            other => panic!("expected Inserted, got {other:?}"),
        }
    }

    fn analysis(track_id: i64) -> AnalysisUpdate {
        AnalysisUpdate {
            track_id,
            bpm: 126.0,
            bpm_confidence: 0.8,
            key: "C".to_string(),
            camelot_key: "8B".to_string(),
            energy: 6.0,
            danceability: 0.5,
            loudness_rms: 0.1,
            spectral_centroid: 2000.0,
            onset_density: 3.0,
        }
    }

    #[test]
    fn traktor_tempo_survives_later_analysis() {
        let conn = connect_in_memory().unwrap();
        let id = insert(&conn, "/music/a.mp3", "h1");
        apply_traktor_tempo(&conn, id, 136.0, Some(("Ebm", "2A"))).unwrap();

        save_analysis(&conn, &analysis(id)).unwrap();

        let (bpm, key, camelot): (f64, String, String) = conn
            .query_row(
                "SELECT bpm, key, camelot_key FROM tracks WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(bpm, 136.0);
        assert_eq!(key, "Ebm");
        assert_eq!(camelot, "2A");
    }

    #[test]
    fn analysis_still_fills_tracks_without_traktor_tempo() {
        let conn = connect_in_memory().unwrap();
        let id = insert(&conn, "/music/b.mp3", "h2");
        save_analysis(&conn, &analysis(id)).unwrap();
        let bpm: f64 = conn
            .query_row("SELECT bpm FROM tracks WHERE id = ?1", params![id], |r| r.get(0))
            .unwrap();
        assert_eq!(bpm, 126.0);
    }

    #[test]
    fn excluded_tracks_are_skipped_when_picking_the_next_one() {
        let conn = connect_in_memory().unwrap();
        let first = insert(&conn, "/music/c.mp3", "h3");
        let second = insert(&conn, "/music/d.mp3", "h4");

        let mut exclude = HashSet::new();
        assert_eq!(next_track_needing_analysis(&conn, &exclude).unwrap().unwrap().id, first);
        exclude.insert(first);
        assert_eq!(next_track_needing_analysis(&conn, &exclude).unwrap().unwrap().id, second);
        exclude.insert(second);
        assert!(next_track_needing_analysis(&conn, &exclude).unwrap().is_none());
    }
}

#[cfg(test)]
mod traktor_bpm_only_tests {
    use super::*;
    use crate::connect_in_memory;

    #[test]
    fn a_bpm_without_a_key_keeps_the_key_analysis_later_finds() {
        let conn = connect_in_memory().unwrap();
        let track = NewTrack {
            file_path: "/music/e.mp3".to_string(),
            file_hash: "h5".to_string(),
            title: None,
            artist: None,
            album: None,
            genre: None,
            year: None,
            duration_ms: None,
            bitrate: None,
            sample_rate: None,
            file_format: Some("mp3".to_string()),
        };
        let id = match upsert_track(&conn, &track).unwrap() {
            UpsertOutcome::Inserted { track_id } => track_id,
            other => panic!("expected Inserted, got {other:?}"),
        };
        apply_traktor_tempo(&conn, id, 128.0, None).unwrap();
        save_analysis(
            &conn,
            &AnalysisUpdate {
                track_id: id,
                bpm: 120.0,
                bpm_confidence: 0.5,
                key: "C".to_string(),
                camelot_key: "8B".to_string(),
                energy: 5.0,
                danceability: 0.5,
                loudness_rms: 0.1,
                spectral_centroid: 2000.0,
                onset_density: 3.0,
            },
        )
        .unwrap();

        let (bpm, camelot): (f64, String) = conn
            .query_row(
                "SELECT bpm, camelot_key FROM tracks WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(bpm, 128.0, "Traktor's BPM stays");
        assert_eq!(camelot, "8B", "analysis fills the missing key");
    }
}
