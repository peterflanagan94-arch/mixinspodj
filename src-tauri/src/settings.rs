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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            traktor_collection: None,
            anthropic_api_key: None,
            share_plays: true,
        }
    }
}

fn default_true() -> bool {
    true
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
