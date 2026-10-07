use std::io::Read;
use std::path::Path;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::ItemKey;
use lofty::probe::Probe;
use mixflow_db::library::{self, NewTrack, UpsertOutcome};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use walkdir::WalkDir;

use crate::DbState;

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "m4a", "mp4", "aac", "ogg", "aif", "aiff"];

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ScanSummary {
    pub folder: String,
    pub files_found: usize,
    pub inserted: usize,
    pub updated: usize,
    pub duplicates_found: usize,
    /// Files that exist only in the cloud, skipped so they aren't downloaded.
    pub cloud_only: usize,
    pub errors: usize,
}

#[tauri::command]
pub fn pick_music_folder() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Choose a music folder")
        .pick_folder()
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command(async)]
pub fn scan_library_folder(
    app: AppHandle,
    db: State<DbState>,
    folder: String,
) -> Result<ScanSummary, String> {
    let audio_files: Vec<_> = WalkDir::new(&folder)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| is_audio_file(entry.path()))
        .collect();

    let total = audio_files.len();
    let mut summary = ScanSummary {
        folder: folder.clone(),
        ..Default::default()
    };

    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        library::upsert_library_folder(&conn, &folder).map_err(|e| e.to_string())?;
        // Pre-mark everything under this folder missing; each file actually
        // found below clears the flag again via upsert_track, so whatever's
        // still marked afterward genuinely wasn't found this scan.
        library::mark_folder_tracks_missing(&conn, &folder).map_err(|e| e.to_string())?;
    }

    for (index, entry) in audio_files.iter().enumerate() {
        let path = entry.path();

        let _ = app.emit(
            "scan-progress",
            ScanProgress {
                current: index + 1,
                total,
                current_file: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            },
        );

        if is_cloud_only(path) {
            summary.cloud_only += 1;
            continue;
        }

        match extract_track(path) {
            Ok(new_track) => {
                let conn = db.0.lock().map_err(|e| e.to_string())?;
                match library::upsert_track(&conn, &new_track) {
                    Ok(UpsertOutcome::Inserted { .. }) => summary.inserted += 1,
                    Ok(UpsertOutcome::Updated { .. }) => summary.updated += 1,
                    Ok(UpsertOutcome::InsertedDuplicate { .. }) => {
                        summary.inserted += 1;
                        summary.duplicates_found += 1;
                    }
                    Err(_) => summary.errors += 1,
                }
            }
            Err(_) => summary.errors += 1,
        }
    }

    summary.files_found = total;
    Ok(summary)
}

/// macOS sets this flag on files that exist only in the cloud (Dropbox,
/// iCloud "optimise storage" and so on). Reading such a file makes the OS
/// download it, so MixInspo DJ never reads them on its own.
#[cfg(target_os = "macos")]
const SF_DATALESS: u32 = 0x4000_0000;

pub(crate) fn is_cloud_only(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        std::fs::metadata(path)
            .map(|m| m.st_flags() & SF_DATALESS != 0)
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}

pub(crate) fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Fills in genre for already-scanned tracks whose album title names a
/// genre but whose own genre tag is empty — the same inference `extract_track`
/// applies to new scans, run once over the existing library at startup.
pub(crate) fn backfill_genre_from_album(conn: &rusqlite::Connection) -> usize {
    let Ok(candidates) = library::tracks_missing_genre(conn) else {
        return 0;
    };
    let mut updated = 0;
    for (track_id, album) in candidates {
        if let Some(genre) = infer_genre_from_album(&album) {
            if library::set_inferred_genre(conn, track_id, &genre).is_ok() {
                updated += 1;
            }
        }
    }
    updated
}

pub(crate) fn extract_track(path: &Path) -> Result<NewTrack, Box<dyn std::error::Error>> {
    let file_hash = hash_file(path)?;

    // Some WAV files have INFO chunks the tag reader rejects. Those still get
    // indexed, from their file name, rather than being skipped.
    let tagged_file = Probe::open(path).ok().and_then(|probe| probe.read().ok());
    let properties = tagged_file.as_ref().map(|f| f.properties().clone());
    let tag = tagged_file
        .as_ref()
        .and_then(|f| f.primary_tag().or_else(|| f.first_tag()));

    let tagged_title = tag.and_then(|t| t.get_string(&ItemKey::TrackTitle)).map(str::to_owned);
    let tagged_artist = tag.and_then(|t| t.get_string(&ItemKey::TrackArtist)).map(str::to_owned);
    // Untagged downloads are common, so fall back to the file name for
    // whichever of artist/title is missing. Tags always win when present.
    let (name_artist, name_title) = names_from_filename(path);
    let title = tagged_title.or(name_title);
    let artist = tagged_artist.or(name_artist);
    let album = tag.and_then(|t| t.get_string(&ItemKey::AlbumTitle)).map(str::to_owned);
    // Compilation albums often name their genre ("NOW That's What I Call
    // Country") even when the file's own genre tag is empty, which is
    // common for tracks ripped or downloaded without full tagging.
    let genre = tag
        .and_then(|t| t.get_string(&ItemKey::Genre))
        .map(str::to_owned)
        .or_else(|| album.as_deref().and_then(infer_genre_from_album));
    let year = tag
        .and_then(|t| t.get_string(&ItemKey::Year).or_else(|| t.get_string(&ItemKey::RecordingDate)))
        .and_then(|s| s.get(0..4))
        .and_then(|s| s.parse::<i32>().ok());

    Ok(NewTrack {
        file_path: path.to_string_lossy().into_owned(),
        file_hash,
        title,
        artist,
        album,
        genre,
        year,
        duration_ms: properties.as_ref().map(|p| p.duration().as_millis() as i64),
        bitrate: properties.as_ref().and_then(|p| p.audio_bitrate()).map(|b| b as i32),
        sample_rate: properties.as_ref().and_then(|p| p.sample_rate()).map(|s| s as i32),
        file_format: path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()),
    })
}

/// Reads "Artist - Title" (or just "Title") from a file name, dropping a
/// leading track number such as "01 " or "07. ". Returns (artist, title).
/// Common genre names, longest first so a multi-word genre matches before a
/// shorter word inside it. Recognising a genre's own name in an album title
/// ("NOW That's What I Call Country") is a much smaller claim than guessing
/// which genres are compatible with which — it only fires when the word is
/// actually there.
const GENRE_NAMES: &[&str] = &[
    "drum and bass",
    "easy listening",
    "spoken word",
    "deep house",
    "tech house",
    "tropical house",
    "future house",
    "progressive house",
    "children's",
    "hip-hop",
    "hip hop",
    "r&b",
    "k-pop",
    "afrobeats",
    "afrobeat",
    "amapiano",
    "dancehall",
    "reggaeton",
    "drill",
    "grime",
    "garage",
    "trance",
    "techno",
    "house",
    "disco",
    "funk",
    "soul",
    "reggae",
    "country",
    "classical",
    "jazz",
    "blues",
    "metal",
    "punk",
    "indie",
    "gospel",
    "latin",
    "soundtrack",
    "ambient",
    "opera",
    "musical",
    "comedy",
    "folk",
    "pop",
    "rock",
    "dance",
    "electronic",
];

/// Finds a known genre's name as a whole word in `album`, case-insensitively.
pub(crate) fn infer_genre_from_album(album: &str) -> Option<String> {
    let lower = album.to_lowercase();
    GENRE_NAMES
        .iter()
        .find(|name| contains_word(&lower, name))
        .map(|name| title_case(name))
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    let is_boundary = |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric());
    haystack.match_indices(needle).any(|(i, _)| {
        is_boundary(haystack[..i].chars().next_back()) && is_boundary(haystack[i + needle.len()..].chars().next())
    })
}

fn title_case(name: &str) -> String {
    name.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn names_from_filename(path: &Path) -> (Option<String>, Option<String>) {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return (None, None);
    };
    let stem = strip_track_number(stem);
    if stem.is_empty() {
        return (None, None);
    }
    match stem.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
            (Some(artist.trim().to_string()), Some(title.trim().to_string()))
        }
        _ => (None, Some(stem.to_string())),
    }
}

/// Removes "07. ", "01 ", "3-" and similar from the start of a name.
fn strip_track_number(stem: &str) -> &str {
    let digits = stem.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 3 {
        return stem.trim();
    }
    let rest = &stem[digits..];
    let separators = rest.len() - rest.trim_start_matches(|c: char| c == '.' || c == '-' || c == '_' || c.is_whitespace()).len();
    if separators == 0 {
        return stem.trim();
    }
    rest[separators..].trim()
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::{infer_genre_from_album, names_from_filename};
    use std::path::Path;

    #[test]
    fn genre_is_recognised_in_a_compilation_album_title() {
        assert_eq!(
            infer_genre_from_album("NOW That's What I Call Country #1s").as_deref(),
            Some("Country")
        );
        assert_eq!(infer_genre_from_album("Pop Party 2013").as_deref(), Some("Pop"));
    }

    #[test]
    fn multi_word_genres_match_before_a_shorter_word_inside_them() {
        assert_eq!(infer_genre_from_album("Deep House Sessions").as_deref(), Some("Deep House"));
    }

    #[test]
    fn a_genre_word_inside_another_word_does_not_match() {
        // "pop" inside "Popular", not the genre.
        assert_eq!(infer_genre_from_album("Popular Hits 2020"), None);
    }

    #[test]
    fn an_album_with_no_recognised_genre_word_gives_nothing() {
        assert_eq!(infer_genre_from_album("Maschine Expansion"), None);
    }

    #[test]
    fn artist_and_title_come_from_dash_separated_name() {
        let (artist, title) = names_from_filename(Path::new("/m/Higher Place - Diafonie.mp3"));
        assert_eq!(artist.as_deref(), Some("Higher Place"));
        assert_eq!(title.as_deref(), Some("Diafonie"));
    }

    #[test]
    fn leading_track_number_is_dropped() {
        let (artist, title) = names_from_filename(Path::new("/m/07. Burial - Archangel.flac"));
        assert_eq!(artist.as_deref(), Some("Burial"));
        assert_eq!(title.as_deref(), Some("Archangel"));
    }

    #[test]
    fn name_without_separator_is_only_a_title() {
        let (artist, title) = names_from_filename(Path::new("/m/Untitled Groove.wav"));
        assert_eq!(artist, None);
        assert_eq!(title.as_deref(), Some("Untitled Groove"));
    }
}
