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
    pub errors: usize,
}

#[tauri::command]
pub fn pick_music_folder() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Choose a music folder")
        .pick_folder()
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
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

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn extract_track(path: &Path) -> Result<NewTrack, Box<dyn std::error::Error>> {
    let file_hash = hash_file(path)?;

    let tagged_file = Probe::open(path)?.read()?;
    let properties = tagged_file.properties();
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let title = tag.and_then(|t| t.get_string(&ItemKey::TrackTitle)).map(str::to_owned);
    let artist = tag.and_then(|t| t.get_string(&ItemKey::TrackArtist)).map(str::to_owned);
    let album = tag.and_then(|t| t.get_string(&ItemKey::AlbumTitle)).map(str::to_owned);
    let genre = tag.and_then(|t| t.get_string(&ItemKey::Genre)).map(str::to_owned);
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
        duration_ms: Some(properties.duration().as_millis() as i64),
        bitrate: properties.audio_bitrate().map(|b| b as i32),
        sample_rate: properties.sample_rate().map(|s| s as i32),
        file_format: path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()),
    })
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
