//! Choices the user makes once and the app remembers, kept in a small JSON
//! file in the app's data folder.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// The Traktor collection file to import from. When unset, the newest
    /// `collection.nml` under the usual Traktor folders is used.
    #[serde(default)]
    pub traktor_collection: Option<String>,
    /// The DJ's own Anthropic API key, for AI Suggestions. There's no
    /// MixInspo server to call the API through yet, so each DJ's own key
    /// pays for their own use. Kept only in this local file, never sent
    /// anywhere but straight to Anthropic's API.
    #[serde(default)]
    pub anthropic_api_key: Option<String>,
    /// Sends this DJ's plays — track, artist, BPM, key, genre; nothing that
    /// identifies them — to MixInspo's shared, anonymous play log, used to
    /// build a real "popular with MixInspo DJs" list from actual usage. On
    /// by default; a Settings toggle turns it off at any time.
    #[serde(default = "default_true")]
    pub share_plays: bool,
    /// What the app opens to: "live" (Now Playing, the normal gig view) or
    /// "set_prepare" (straight to the Set Builder). Everything else still
    /// runs either way — the Traktor listener, Now Playing — this only
    /// decides which screen greets the DJ.
    #[serde(default = "default_startup_view")]
    pub startup_view: String,
    /// The DJ's own Spotify API credentials (Settings → Spotify), for
    /// reading a public playlist's track list to match against their
    /// library. A free Spotify Developer app — Client Credentials flow,
    /// no personal Spotify login. Kept only in this local file.
    #[serde(default)]
    pub spotify_client_id: Option<String>,
    #[serde(default)]
    pub spotify_client_secret: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            traktor_collection: None,
            anthropic_api_key: None,
            share_plays: true,
            startup_view: default_startup_view(),
            spotify_client_id: None,
            spotify_client_secret: None,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_startup_view() -> String {
    "live".to_string()
}

fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> AppSettings {
    settings_path(app)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app).ok_or("No app data folder available")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_startup_view(app: AppHandle) -> String {
    load(&app).startup_view
}

#[tauri::command]
pub fn set_startup_view(app: AppHandle, view: String) -> Result<(), String> {
    let mut settings = load(&app);
    settings.startup_view = view;
    save(&app, &settings)
}
