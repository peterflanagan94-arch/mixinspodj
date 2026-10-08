//! Sends this DJ's plays to MixInspo's shared, anonymous play log — track,
//! artist, BPM, key, genre, nothing that identifies the DJ — so a real
//! "popular with MixInspo DJs" list can be built from actual usage instead
//! of copying anyone else's chart. On by default; the DJ can turn it off in
//! Settings at any time, and the toggle takes effect immediately.
//!
//! Every send is fire-and-forget: it never blocks the caller, and a failed
//! or dropped request is silently ignored rather than interrupting a DJ
//! mid-set. Uses the same Cloudflare Worker as AI Suggestions
//! (`server/ai-proxy/`), just a different route.

use serde_json::json;
use tauri::AppHandle;

use crate::settings;

const PLAYS_URL: &str = "https://mixinspo-ai-proxy.mixinspodjsoftware.workers.dev/plays";

#[tauri::command]
pub fn has_share_plays_enabled(app: AppHandle) -> bool {
    settings::load(&app).share_plays
}

#[tauri::command]
pub fn set_share_plays(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut current = settings::load(&app);
    current.share_plays = enabled;
    settings::save(&app, &current)
}

/// Shares one play, if the DJ hasn't turned this off. Spawns its own task
/// and returns immediately — the caller never waits on the network.
pub fn share_play(
    app: &AppHandle,
    title: Option<String>,
    artist: Option<String>,
    bpm: Option<f64>,
    camelot_key: Option<String>,
    genre: Option<String>,
) {
    let Some(title) = title else { return };
    if !settings::load(app).share_plays {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let _ = client
            .post(PLAYS_URL)
            .header("content-type", "application/json")
            .json(&json!({
                "title": title,
                "artist": artist,
                "bpm": bpm,
                "key": camelot_key,
                "genre": genre,
            }))
            .send()
            .await;
    });
}
