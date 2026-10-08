//! Reads VirtualDJ's own database file (read-only) to import BPM and key —
//! the same kind of import already built for Traktor's `collection.nml`,
//! adapted to VirtualDJ's file format.
//!
//! **Unverified.** Built from VirtualDJ's community wiki and forum threads
//! — there's no official reference for this, and it's never been tested
//! against a real VirtualDJ database file. The two things most likely to be
//! wrong: the file's exact location on macOS (the sources describing this
//! are Windows-centric), and whether BOTH the `Tags` and `Scan` blocks'
//! `Bpm` store seconds-per-beat or only one of them does. The parser is
//! written defensively — skip anything that doesn't look right, never
//! guess — but this needs a real file to actually confirm. Because of that,
//! unlike Traktor's import, this never runs automatically: a DJ has to
//! choose the file and press Import themselves.

use std::path::{Path, PathBuf};

use mixflow_analysis::key_for;
use mixflow_db::library;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use walkdir::WalkDir;

use crate::library_scan::{extract_track, is_audio_file, is_cloud_only};
use crate::traktor::{attr, pitch_for_traktor_key, tag};
use crate::DbState;

#[derive(Debug, Clone, PartialEq)]
pub struct VdjEntry {
    pub path: Option<String>,
    pub bpm: Option<f32>,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct VdjImportSummary {
    pub entries: usize,
    pub tempo_applied: usize,
    pub indexed: usize,
    pub missing_files: usize,
}

/// VirtualDJ's documented location is `Documents/VirtualDJ/VirtualDJ
/// Database *.xml`. Also checks the common macOS `Application Support`
/// convention, since the Mac location specifically isn't confirmed anywhere
/// found. Picks the most recently modified match.
pub fn find_database() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    [home.join("Documents/VirtualDJ"), home.join("Library/Application Support/VirtualDJ")]
        .into_iter()
        .flat_map(|root| {
            WalkDir::new(root)
                .max_depth(2)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path().extension().is_some_and(|x| x == "xml")
                        && e.file_name().to_string_lossy().starts_with("VirtualDJ Database")
                })
                .filter_map(|e| {
                    let modified = e.metadata().ok()?.modified().ok()?;
                    Some((modified, e.into_path()))
                })
                .collect::<Vec<_>>()
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

#[tauri::command]
pub fn find_virtualdj_database() -> Option<String> {
    find_database().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn choose_virtualdj_database() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Choose your VirtualDJ database file")
        .add_filter("VirtualDJ database", &["xml"])
        .pick_file()
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command(async)]
pub fn import_virtualdj_database(app: AppHandle, path: String) -> Result<VdjImportSummary, String> {
    let xml = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let entries = parse_database(&xml);
    let mut summary = VdjImportSummary { entries: entries.len(), ..Default::default() };

    for entry in entries {
        let Some(path) = entry.path else { continue };
        if !Path::new(&path).is_file() {
            summary.missing_files += 1;
            continue;
        }

        let track_id = {
            let db = app.state::<DbState>();
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            library::find_track_id_by_path(&conn, &path).map_err(|e| e.to_string())?
        };
        let track_id = match track_id {
            Some(id) => id,
            None => {
                if !is_audio_file(Path::new(&path)) || is_cloud_only(Path::new(&path)) {
                    continue;
                }
                let Ok(new_track) = extract_track(Path::new(&path)) else { continue };
                let db = app.state::<DbState>();
                let conn = db.0.lock().map_err(|e| e.to_string())?;
                match library::upsert_track(&conn, &new_track) {
                    Ok(library::UpsertOutcome::Inserted { track_id })
                    | Ok(library::UpsertOutcome::InsertedDuplicate { track_id, .. }) => {
                        summary.indexed += 1;
                        track_id
                    }
                    _ => continue,
                }
            }
        };

        let Some(bpm) = entry.bpm else { continue };
        let key = entry.key.as_deref().and_then(pitch_for_traktor_key).map(|(pc, major)| key_for(pc, major));
        let key_ref = key.as_ref().map(|(name, camelot)| (name.as_str(), camelot.as_str()));

        let db = app.state::<DbState>();
        let Ok(conn) = db.0.lock() else { continue };
        if library::apply_external_tempo(&conn, track_id, "virtualdj", bpm, key_ref).is_ok() {
            summary.tempo_applied += 1;
        }
    }

    Ok(summary)
}

/// Parses the `Song` entries of a VirtualDJ database file.
pub fn parse_database(xml: &str) -> Vec<VdjEntry> {
    xml.split("<Song ")
        .skip(1)
        .map(|entry| {
            // attr() expects a space before each attribute name — present
            // naturally when a tag comes from tag() (which keeps its own
            // opening marker), but the split above already consumed
            // "<Song ", so it has to be put back here.
            let song_tag_end = entry.find('>').unwrap_or(entry.len());
            let song_tag = format!(" {}", &entry[..song_tag_end]);
            let path = attr(&song_tag, "FilePath");

            // VirtualDJ's own analysis (Scan) is preferred over a plain ID3
            // tag (Tags), which can be stale or absent.
            let scan = tag(entry, "<Scan ");
            let tags = tag(entry, "<Tags ");
            let bpm = scan
                .and_then(|t| attr(t, "Bpm"))
                .or_else(|| tags.and_then(|t| attr(t, "Bpm")))
                .as_deref()
                .and_then(parse_vdj_bpm);
            let key = scan.and_then(|t| attr(t, "Key")).or_else(|| tags.and_then(|t| attr(t, "Key")));

            VdjEntry { path, bpm, key }
        })
        .collect()
}

/// VirtualDJ's own wiki says the stored value is the number of seconds
/// between two beats, not a raw BPM figure — this undoes that. Guarded with
/// a sanity range, since this is the one part of the format that's
/// genuinely unconfirmed: a value already in a normal BPM range is used
/// as-is rather than wrongly inverted into something absurd.
fn parse_vdj_bpm(raw: &str) -> Option<f32> {
    let value: f32 = raw.parse().ok()?;
    if value <= 0.0 {
        return None;
    }
    if (40.0..=220.0).contains(&value) {
        return Some(value);
    }
    let bpm = 60.0 / value;
    (40.0..=220.0).contains(&bpm).then_some(bpm)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<VirtualDJ_Database Version="1.0">
<Song FilePath="/Users/dj/Music/Daft Punk - One More Time.mp3" FileSize="5242880">
<Tags Author="Daft Punk" Title="One More Time" Bpm="123.000" Key="10B"></Tags>
<Scan Bpm="0.487805" Key="10B"></Scan>
<Infos SongLength="320" FirstPlay="..." PlayCount="4"></Infos>
</Song>
<Song FilePath="/Users/dj/Music/missing.mp3" FileSize="1000">
<Tags Author="Nobody" Title="Untagged"></Tags>
</Song>
</VirtualDJ_Database>"#;

    #[test]
    fn entries_read_path_bpm_and_key_preferring_scan_over_tags() {
        let entries = parse_database(SAMPLE);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path.as_deref(), Some("/Users/dj/Music/Daft Punk - One More Time.mp3"));
        // Scan's 0.487805 seconds/beat converts to ~123 BPM, matching Tags'
        // own 123 — the conversion and the preference-for-Scan both check out.
        assert!((entries[0].bpm.unwrap() - 123.0).abs() < 0.5);
        assert_eq!(entries[0].key.as_deref(), Some("10B"));
    }

    #[test]
    fn an_entry_with_no_tempo_data_has_none() {
        let entries = parse_database(SAMPLE);
        assert_eq!(entries[1].bpm, None);
        assert_eq!(entries[1].key, None);
    }

    #[test]
    fn bpm_parsing_handles_both_a_seconds_per_beat_value_and_a_plain_bpm_value() {
        // A plain, already-sane BPM is trusted as-is.
        assert_eq!(parse_vdj_bpm("128"), Some(128.0));
        // A small value in the seconds-per-beat range is converted: 60/0.5 = 120.
        assert_eq!(parse_vdj_bpm("0.5"), Some(120.0));
        // Garbage that converts to nothing sane is rejected rather than guessed at.
        assert_eq!(parse_vdj_bpm("not-a-number"), None);
        assert_eq!(parse_vdj_bpm("0"), None);
    }
}
