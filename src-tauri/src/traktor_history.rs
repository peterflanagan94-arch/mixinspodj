//! Imports Traktor's own play history (`History/*.nml`, one file per Traktor
//! session). Each play records how long the track actually played, so reports
//! built from it are exact rather than estimated. Read only.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use mixflow_db::history::{self, HistoryPlay};
use mixflow_db::library;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use walkdir::WalkDir;

use crate::traktor::{attr, tag};
use crate::DbState;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPlay {
    pub location: String,
    pub start_code: i64,
    pub start_seconds: i64,
    pub played_seconds: f64,
    pub deck: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct HistoryImportSummary {
    pub files: usize,
    pub imported: usize,
    pub unchanged: usize,
    pub plays: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraktorSetDto {
    pub set_id: i64,
    pub file_name: String,
    pub plays: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraktorPlayDto {
    pub title: String,
    pub artist: Option<String>,
    pub track_id: Option<i64>,
    pub start_seconds: i64,
    pub played_seconds: f64,
    pub deck: i64,
}

/// Every history file under the usual Traktor folders.
fn history_files() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let home = PathBuf::from(home);
    ["Documents/Native Instruments", "Music/Native Instruments"]
        .iter()
        .flat_map(|sub| {
            WalkDir::new(home.join(sub))
                .max_depth(4)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path().extension().is_some_and(|x| x == "nml")
                        && e.path()
                            .parent()
                            .and_then(|p| p.file_name())
                            .is_some_and(|name| name == "History")
                })
                .map(|e| e.into_path())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Reads every play entry in a Traktor history file. Entries without timing
/// data are not plays and are skipped.
pub fn parse_history(xml: &str) -> Vec<ParsedPlay> {
    xml.split("<ENTRY>")
        .skip(1)
        .filter_map(|entry| {
            let key = tag(entry, "<PRIMARYKEY ").and_then(|t| attr(t, "KEY"))?;
            let timing = tag(entry, "<EXTENDEDDATA ")?;
            Some(ParsedPlay {
                location: traktor_key_to_path(&key),
                start_code: attr(timing, "STARTDATE")?.parse().ok()?,
                start_seconds: attr(timing, "STARTTIME")?.parse().ok()?,
                played_seconds: attr(timing, "DURATION")?.parse().ok()?,
                deck: attr(timing, "DECK").and_then(|d| d.parse().ok()).unwrap_or(0),
            })
        })
        .collect()
}

/// Turns a history key such as "Macintosh/:Users/:me/:Music/:a.mp3" into
/// "/Users/me/Music/a.mp3". The part before the first "/:" is the volume.
fn traktor_key_to_path(key: &str) -> String {
    let (volume, rest) = match key.find("/:") {
        Some(split) => (&key[..split], &key[split + 2..]),
        None => ("", key),
    };
    let path = rest.replace("/:", "/");
    match volume {
        "" | "Macintosh" | "Macintosh HD" => format!("/{path}"),
        other => format!("/Volumes/{other}/{path}"),
    }
}

fn file_mtime(path: &Path) -> Option<i64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

/// Imports every history file that is new or has changed since it was last imported.
pub fn import_history(app: &AppHandle) -> HistoryImportSummary {
    let mut summary = HistoryImportSummary::default();
    for path in history_files() {
        summary.files += 1;
        let Some(file_name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let Some(mtime) = file_mtime(&path) else { continue };
        let db = app.state::<DbState>();
        let Ok(mut conn) = db.0.lock() else { continue };
        if history::stored_mtime(&conn, &file_name).ok().flatten() == Some(mtime) {
            summary.unchanged += 1;
            continue;
        }
        let Ok(xml) = fs::read_to_string(&path) else { continue };
        let plays: Vec<HistoryPlay> = parse_history(&xml)
            .into_iter()
            .map(|p| {
                let track_id = library::find_track_id_by_path(&conn, &p.location).ok().flatten();
                HistoryPlay {
                    start_code: p.start_code,
                    start_seconds: p.start_seconds,
                    played_seconds: p.played_seconds,
                    deck: p.deck,
                    location: p.location,
                    track_id,
                }
            })
            .collect();
        if history::replace_set(&mut conn, &file_name, mtime, &plays).is_ok() {
            summary.imported += 1;
            summary.plays += plays.len();
        }
    }
    summary
}

#[tauri::command(async)]
pub fn import_traktor_history(app: AppHandle) -> HistoryImportSummary {
    import_history(&app)
}

#[tauri::command]
pub fn list_traktor_sets(db: tauri::State<DbState>) -> Result<Vec<TraktorSetDto>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let sets = history::list_sets(&conn).map_err(|e| e.to_string())?;
    Ok(sets
        .into_iter()
        .map(|s| TraktorSetDto {
            set_id: s.id,
            file_name: s.file_name,
            plays: s.plays,
        })
        .collect())
}

#[tauri::command]
pub fn traktor_set_report(db: tauri::State<DbState>, set_id: i64) -> Result<Vec<TraktorPlayDto>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    report_rows(&conn, set_id)
}

/// The plays in one imported Traktor session, in order, with titles from the
/// library where a track matched. Shared by the Sets screen and the PDF export.
pub fn report_rows(conn: &rusqlite::Connection, set_id: i64) -> Result<Vec<TraktorPlayDto>, String> {
    let plays = history::set_plays(conn, set_id).map_err(|e| e.to_string())?;
    plays
        .into_iter()
        .map(|p| {
            let (title, artist) = match p.track_id {
                Some(id) => conn
                    .query_row(
                        "SELECT title, artist FROM tracks WHERE id = ?1",
                        rusqlite::params![id],
                        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
                    )
                    .map_err(|e| e.to_string())?,
                None => (None, None),
            };
            let file_title = Path::new(&p.location)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(TraktorPlayDto {
                title: title.unwrap_or(file_title),
                artist,
                track_id: p.track_id,
                start_seconds: p.start_seconds,
                played_seconds: p.played_seconds,
                deck: p.deck,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<HEAD COMPANY="x"><PLAYLISTS></PLAYLISTS></HEAD><ENTRY>
<PRIMARYKEY TYPE="TRACK" KEY="Macintosh/:Users/:peterflanagan/:DJ Library 2021/:9 - Earth_ Wind &amp; Fire - September.mp3"></PRIMARYKEY>
<EXTENDEDDATA DECK="0" DURATION="216.064423" EXTENDEDTYPE="HistoryData" PLAYEDPUBLIC="0" STARTDATE="132778502" STARTTIME="29293"></EXTENDEDDATA>
</ENTRY><ENTRY><PRIMARYKEY TYPE="TRACK" KEY="Macintosh/:Users/:me/:x.mp3"></PRIMARYKEY></ENTRY>"#;

    #[test]
    fn plays_carry_exact_playing_time_and_start() {
        let plays = parse_history(SAMPLE);
        assert_eq!(plays.len(), 1, "an entry with no timing is not a play");
        assert_eq!(plays[0].played_seconds, 216.064423);
        assert_eq!(plays[0].start_seconds, 29293);
        assert_eq!(plays[0].location, "/Users/peterflanagan/DJ Library 2021/9 - Earth_ Wind & Fire - September.mp3");
    }

    #[test]
    fn external_volumes_are_under_volumes() {
        assert_eq!(traktor_key_to_path("Live Drive/:Music/:a.mp3"), "/Volumes/Live Drive/Music/a.mp3");
        assert_eq!(traktor_key_to_path("Macintosh HD/:Users/:me/:b.mp3"), "/Users/me/b.mp3");
    }
}
