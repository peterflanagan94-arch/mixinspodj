//! Reads Traktor's own collection file (`collection.nml`) so MixInspo DJ can
//! use the BPM and key the DJ already has in Traktor, and find tracks that
//! are in Traktor but not yet in a watched folder.
//!
//! Traktor keeps its collection in memory and writes this file on save or
//! quit, so the import reflects the last save. The file is only ever read.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use mixflow_analysis::key_for;
use mixflow_db::library::{self, UpsertOutcome};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use walkdir::WalkDir;

use crate::library_scan::{extract_track, is_audio_file, is_cloud_only};
use crate::{settings, DbState};

/// The collection file's modified time at the last import, so an unchanged
/// file is not parsed again on every watch tick.
static LAST_IMPORT: Mutex<Option<SystemTime>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Default)]
pub struct ImportSummary {
    pub entries: usize,
    /// Tracks whose BPM and key were set from Traktor.
    pub tempo_applied: usize,
    /// Files Traktor knows about that weren't in the library and were added.
    pub indexed: usize,
    /// Entries whose file isn't on disk (moved, or on an unplugged drive).
    pub missing_files: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CollectionEntry {
    pub path: Option<String>,
    pub bpm: Option<f32>,
    /// Traktor's key name, such as "Ebm" or "F#".
    pub key: Option<String>,
}

/// The collection to import from: the one the user chose, if it still exists,
/// otherwise the newest `collection.nml` under the usual Traktor folders.
pub fn find_collection(app: &AppHandle) -> Option<PathBuf> {
    if let Some(chosen) = settings::load(app).traktor_collection {
        let path = PathBuf::from(chosen);
        if path.is_file() {
            return Some(path);
        }
    }
    newest_default_collection()
}

fn newest_default_collection() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    ["Documents/Native Instruments", "Music/Native Instruments"]
        .iter()
        .map(|sub| home.join(sub))
        .flat_map(|root| {
            WalkDir::new(root)
                .max_depth(3)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file() && e.file_name() == "collection.nml")
                .filter_map(|e| {
                    let modified = e.metadata().ok()?.modified().ok()?;
                    Some((modified, e.into_path()))
                })
                .collect::<Vec<_>>()
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

/// Imports the collection if it has changed since the last import. Returns
/// the summary when an import ran, so the caller can refresh the UI.
pub fn import_if_changed(app: &AppHandle) -> Option<ImportSummary> {
    let nml = find_collection(app)?;
    let modified = fs::metadata(&nml).ok()?.modified().ok()?;
    if *LAST_IMPORT.lock().ok()? == Some(modified) {
        return None;
    }
    let summary = import_collection(app, &nml).ok()?;
    *LAST_IMPORT.lock().ok()? = Some(modified);
    Some(summary)
}

/// The collection file the app is importing from, chosen or automatic.
#[tauri::command]
pub fn active_traktor_collection(app: AppHandle) -> Option<String> {
    find_collection(&app).map(|p| p.to_string_lossy().into_owned())
}

/// Lets the user pick a Traktor collection file, remembers it, and resets the
/// import so the new file is read on the next import. Runs on the main thread,
/// because the file dialog has to.
#[tauri::command]
pub fn choose_traktor_collection(app: AppHandle) -> Result<Option<String>, String> {
    let Some(picked) = rfd::FileDialog::new()
        .set_title("Choose your Traktor collection")
        .add_filter("Traktor collection", &["nml"])
        .pick_file()
    else {
        return Ok(None);
    };
    let mut current = settings::load(&app);
    current.traktor_collection = Some(picked.to_string_lossy().into_owned());
    settings::save(&app, &current)?;
    if let Ok(mut last) = LAST_IMPORT.lock() {
        *last = None;
    }
    Ok(current.traktor_collection)
}

#[tauri::command(async)]
pub fn import_traktor_collection(app: AppHandle) -> Result<ImportSummary, String> {
    let nml = find_collection(&app).ok_or_else(|| "No Traktor collection file found".to_string())?;
    let summary = import_collection(&app, &nml)?;
    if let Ok(modified) = fs::metadata(&nml).and_then(|m| m.modified()) {
        if let Ok(mut last) = LAST_IMPORT.lock() {
            *last = Some(modified);
        }
    }
    Ok(summary)
}

fn import_collection(app: &AppHandle, nml: &Path) -> Result<ImportSummary, String> {
    let xml = fs::read_to_string(nml).map_err(|e| e.to_string())?;
    let entries = parse_collection(&xml);
    let mut summary = ImportSummary {
        entries: entries.len(),
        ..Default::default()
    };

    eprintln!("traktor import: {} entries from {}", entries.len(), nml.display());
    for entry in entries {
        let Some(path) = entry.path else { continue };
        if !Path::new(&path).is_file() {
            summary.missing_files += 1;
            continue;
        }

        let track_id = {
            let db = app.state::<DbState>();
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            match library::find_track_id_by_path(&conn, &path) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("traktor import: lookup failed for {path}: {e}");
                    return Err(e.to_string());
                }
            }
        };
        let track_id = match track_id {
            Some(id) => id,
            None => {
                // Only index files that are already on this Mac; a cloud-only
                // file gets indexed once it's downloaded.
                if !is_audio_file(Path::new(&path)) || is_cloud_only(Path::new(&path)) {
                    continue;
                }
                let new_track = match extract_track(Path::new(&path)) {
                    Ok(track) => track,
                    Err(e) => {
                        eprintln!("traktor import: could not read {path}: {e}");
                        continue;
                    }
                };
                let db = app.state::<DbState>();
                let conn = db.0.lock().map_err(|e| e.to_string())?;
                match library::upsert_track(&conn, &new_track) {
                    Ok(UpsertOutcome::Inserted { track_id })
                    | Ok(UpsertOutcome::InsertedDuplicate { track_id, .. }) => {
                        summary.indexed += 1;
                        track_id
                    }
                    _ => continue,
                }
            }
        };

        let Some(bpm) = entry.bpm else { continue };
        let key = entry
            .key
            .as_deref()
            .and_then(pitch_for_traktor_key)
            .map(|(pitch_class, is_major)| key_for(pitch_class, is_major));
        let db = app.state::<DbState>();
        // One track failing to update shouldn't stop the rest of the collection.
        let Ok(conn) = db.0.lock() else { continue };
        let key_ref = key.as_ref().map(|(name, camelot)| (name.as_str(), camelot.as_str()));
        if library::apply_traktor_tempo(&conn, track_id, bpm, key_ref).is_ok() {
            summary.tempo_applied += 1;
        }
    }

    eprintln!("traktor import done: {summary:?}");
    Ok(summary)
}

/// Parses the track entries of a Traktor `collection.nml`. Only the fields
/// the import needs are read; everything else in the file is ignored.
pub fn parse_collection(xml: &str) -> Vec<CollectionEntry> {
    xml.split("<ENTRY ")
        .skip(1)
        .map(|entry| {
            let path = tag(entry, "<LOCATION ").and_then(location_to_path);
            let bpm = tag(entry, "<TEMPO ")
                .and_then(|t| attr(t, "BPM"))
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|bpm| *bpm > 0.0);
            let key = tag(entry, "<INFO ").and_then(|t| attr(t, "KEY"));
            CollectionEntry { path, bpm, key }
        })
        .collect()
}

/// Turns Traktor's LOCATION (volume, folder and file, with ':' as the folder
/// separator) into a POSIX path. The boot disk is "Macintosh" in Traktor.
fn location_to_path(location: &str) -> Option<String> {
    let file = attr(location, "FILE")?;
    let dir = attr(location, "DIR").unwrap_or_default().replace("/:", "/");
    let volume = attr(location, "VOLUME").unwrap_or_default();
    let base = match volume.as_str() {
        "" | "Macintosh" | "Macintosh HD" => String::new(),
        other => format!("/Volumes/{other}"),
    };
    Some(format!("{base}{dir}{file}"))
}

/// A Traktor key as (pitch class 0=C..11=B, is_major). Traktor writes keys
/// as note names ("Ebm", "F#"), in Open Key notation ("6m", "4d"), or directly
/// as Camelot codes ("8A", "11B").
pub fn pitch_for_traktor_key(name: &str) -> Option<(usize, bool)> {
    let name = name.trim();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        return match name.chars().last()? {
            'A' | 'B' => camelot_pitch(name),
            _ => open_key_pitch(name),
        };
    }
    note_name_pitch(name)
}

/// The pitch for a Camelot code such as "8A" or "11B".
fn camelot_pitch(code: &str) -> Option<(usize, bool)> {
    let is_major = match code.chars().last()? {
        'B' => true,
        'A' => false,
        _ => return None,
    };
    let number: u32 = code[..code.len() - 1].parse().ok()?;
    if !(1..=12).contains(&number) {
        return None;
    }
    let camelot = format!("{number}{}", if is_major { 'B' } else { 'A' });
    (0..12).find(|&pc| key_for(pc, is_major).1 == camelot).map(|pc| (pc, is_major))
}

/// Open Key numbers run 7 steps ahead of Camelot: "1d" is 8B (C major) and
/// "1m" is 8A (A minor). The pitch is the key whose Camelot code matches.
fn open_key_pitch(name: &str) -> Option<(usize, bool)> {
    let split = name.find(|c: char| !c.is_ascii_digit())?;
    let number: u32 = name[..split].parse().ok()?;
    if !(1..=12).contains(&number) {
        return None;
    }
    let (is_major, letter) = match &name[split..] {
        "d" => (true, 'B'),
        "m" => (false, 'A'),
        _ => return None,
    };
    let camelot = format!("{}{}", (number + 6) % 12 + 1, letter);
    (0..12).find(|&pc| key_for(pc, is_major).1 == camelot).map(|pc| (pc, is_major))
}

fn note_name_pitch(name: &str) -> Option<(usize, bool)> {
    let mut chars = name.chars();
    let base: i32 = match chars.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = chars.collect();
    let (accidental, suffix) = if let Some(r) = rest.strip_prefix('#') {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('b') {
        (-1, r)
    } else {
        (0, rest.as_str())
    };
    let is_major = match suffix {
        "" => true,
        "m" => false,
        _ => return None,
    };
    Some(((base + accidental).rem_euclid(12) as usize, is_major))
}

/// The text of the first `open` tag (from its name up to the closing '>').
fn tag<'a>(s: &'a str, open: &str) -> Option<&'a str> {
    let start = s.find(open)?;
    let rest = &s[start..];
    let end = rest.find('>').unwrap_or(rest.len());
    Some(&rest[..end])
}

/// An XML attribute's value from a tag's text, with the basic entities decoded.
fn attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let len = tag[start..].find('"')?;
    Some(
        tag[start..start + len]
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<COLLECTION ENTRIES="2"><ENTRY MODIFIED_DATE="2026/10/5" TITLE="Bumpy" ARTIST="Bumpin Flava"><LOCATION DIR="/:Library/:Application Support/:Traktor Pro 4/:Factory Sounds/:" FILE="Bumpin Flava - Bumpy.mp3" VOLUME="Macintosh" VOLUMEID="Macintosh"></LOCATION><ALBUM TITLE="Maschine Expansion"></ALBUM><INFO BITRATE="299672" KEY="Ebm" PLAYTIME="142"></INFO><TEMPO BPM="136.000000" BPM_QUALITY="100.000000"></TEMPO></ENTRY><ENTRY TITLE="Rock &amp; Roll" ARTIST="X"><LOCATION DIR="/:Music/:DJ Library/:" FILE="rock.mp3" VOLUME="Live Drive"></LOCATION><INFO KEY="F#"></INFO><TEMPO BPM="0.000000"></TEMPO></ENTRY></COLLECTION>"#;

    #[test]
    fn entries_give_path_tempo_and_key() {
        let entries = parse_collection(SAMPLE);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            CollectionEntry {
                path: Some("/Library/Application Support/Traktor Pro 4/Factory Sounds/Bumpin Flava - Bumpy.mp3".into()),
                bpm: Some(136.0),
                key: Some("Ebm".into()),
            }
        );
    }

    #[test]
    fn external_volumes_are_under_volumes_and_zero_tempo_is_ignored() {
        let entries = parse_collection(SAMPLE);
        assert_eq!(entries[1].path.as_deref(), Some("/Volumes/Live Drive/Music/DJ Library/rock.mp3"));
        assert_eq!(entries[1].bpm, None);
        assert_eq!(entries[1].key.as_deref(), Some("F#"));
    }

    #[test]
    fn traktor_note_names_map_to_pitch_classes() {
        assert_eq!(pitch_for_traktor_key("Ebm"), Some((3, false)));
        assert_eq!(pitch_for_traktor_key("Bbm"), Some((10, false)));
        assert_eq!(pitch_for_traktor_key("Bb"), Some((10, true)));
        assert_eq!(pitch_for_traktor_key("F#"), Some((6, true)));
        assert_eq!(pitch_for_traktor_key("Cm"), Some((0, false)));
        assert_eq!(pitch_for_traktor_key("Xq"), None);
    }

    #[test]
    fn camelot_codes_are_read_directly() {
        assert_eq!(pitch_for_traktor_key("8A"), Some((9, false)));
        assert_eq!(pitch_for_traktor_key("11B"), Some((9, true)));
        assert_eq!(pitch_for_traktor_key("13A"), None);
    }

    #[test]
    fn open_key_notation_matches_camelot_numbering() {
        // 6m is 1A (G# minor) and 4d is 11B (A major), as seen in the user's collection.
        assert_eq!(pitch_for_traktor_key("6m"), Some((8, false)));
        assert_eq!(pitch_for_traktor_key("4d"), Some((9, true)));
        assert_eq!(pitch_for_traktor_key("1d"), Some((0, true)));
        assert_eq!(pitch_for_traktor_key("13m"), None);
    }

    #[test]
    fn entities_in_attributes_are_decoded() {
        assert_eq!(attr(r#"<X NAME="A &amp; B">"#, "NAME").as_deref(), Some("A & B"));
    }
}
