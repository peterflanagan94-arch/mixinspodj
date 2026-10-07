//! AI Suggestions: asks Claude what it knows about the track playing now —
//! genre, era, mood, the kind of thing BPM and key alone can't capture — to
//! pick from a real sample of the DJ's own library. Separate from NEXT: this
//! is the one part of the app that calls out to the internet, so it only
//! runs when the DJ asks for it.
//!
//! Two ways to pay for the call: a DJ's own Anthropic API key (Settings →
//! AI Suggestions), used directly — or, when none is set, MixInspo's own
//! shared proxy (see `server/ai-proxy/`), which holds the real key
//! server-side so a beta tester never needs their own account. Either way
//! the request body is identical; only where it's sent differs.
//!
//! The model is asked to answer with candidate *numbers*, not track names —
//! tested live, it paraphrases titles (drops remix tags, "ft." vs "Feat"),
//! so matching its answer back to a track by text would be unreliable.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, State};

use crate::{settings, DbState};

const MODEL: &str = "claude-haiku-5-5";
const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// MixInspo's own shared proxy (`server/ai-proxy/`), used when the DJ hasn't
/// set their own key.
const PROXY_URL: &str = "https://mixinspo-ai-proxy.mixinspodjsoftware.workers.dev";
/// Enough for real variety without an oversized prompt; validated live at
/// ~5,700 input tokens for 150 candidates, a fraction of a cent per call.
const CANDIDATE_POOL: usize = 150;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSuggestion {
    pub track_id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub bpm: Option<f64>,
    pub camelot_key: Option<String>,
    /// How good a fit Claude judged it, 0-100 — shown as the row's score,
    /// the same glance-at number as NEXT's local score.
    pub fit: u8,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiSuggestionResult {
    pub suggestions: Vec<AiSuggestion>,
    /// True when this came from a past request for this song, not a fresh
    /// call to Claude — so the screen can say so, and offer to ask again.
    pub from_cache: bool,
}

#[derive(Debug, Clone)]
struct Candidate {
    track_id: i64,
    title: String,
    artist: Option<String>,
    bpm: Option<f64>,
    camelot_key: Option<String>,
    genre: Option<String>,
}

#[tauri::command]
pub fn has_ai_api_key(app: AppHandle) -> bool {
    settings::load(&app).anthropic_api_key.is_some_and(|k| !k.trim().is_empty())
}

/// Whether AI Suggestions can be used at all right now — a personal key, or
/// the shared proxy being configured. Settings uses `has_ai_api_key`
/// specifically to show the personal-key override state; the AI Suggest
/// screen uses this to decide whether to show the "get set up" message.
#[tauri::command]
pub fn ai_suggestions_available(app: AppHandle) -> bool {
    has_ai_api_key(app) || !PROXY_URL.is_empty()
}

#[tauri::command]
pub fn set_ai_api_key(app: AppHandle, key: String) -> Result<(), String> {
    let mut current = settings::load(&app);
    current.anthropic_api_key = Some(key.trim().to_string());
    settings::save(&app, &current)
}

#[tauri::command]
pub fn clear_ai_api_key(app: AppHandle) -> Result<(), String> {
    let mut current = settings::load(&app);
    current.anthropic_api_key = None;
    settings::save(&app, &current)
}

/// Asks Claude for suggestions, or — unless `force` is set — reuses the
/// answer from the last time this exact song was asked about on this
/// machine. The cache is keyed by title and artist, not a library row, so
/// it's shared across duplicate copies of the same track, and still applies
/// after a rescan changes a track's id.
#[tauri::command(async)]
pub async fn ai_suggestions(
    app: AppHandle,
    db: State<'_, DbState>,
    now_playing_id: i64,
    force: Option<bool>,
) -> Result<AiSuggestionResult, String> {
    let (now_title, now_artist) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        now_playing_names(&conn, now_playing_id)?
    };
    let cache_artist = now_artist.as_deref().unwrap_or("");

    if !force.unwrap_or(false) {
        let cached = {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            mixflow_db::ai_cache::get(&conn, &now_title, cache_artist).map_err(|e| e.to_string())?
        };
        if let Some(json) = cached {
            if let Ok(suggestions) = serde_json::from_str::<Vec<AiSuggestion>>(&json) {
                return Ok(AiSuggestionResult { suggestions, from_cache: true });
            }
        }
    }

    let api_key = settings::load(&app).anthropic_api_key.filter(|k| !k.trim().is_empty());
    if api_key.is_none() && PROXY_URL.is_empty() {
        return Err(
            "AI Suggestions isn't set up yet. Add your own Anthropic API key in Settings → \
             AI Suggestions, or ask whoever runs MixInspo to finish the shared setup."
                .to_string(),
        );
    }

    let candidates = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_candidates(&conn, now_playing_id)?
    };
    if candidates.is_empty() {
        return Err("No other tracks with BPM and key yet to suggest from.".to_string());
    }

    let reply = call_claude(api_key.as_deref(), &now_title, now_artist.as_deref(), &candidates).await?;
    let suggestions = resolve_picks(&reply, &candidates);

    if let Ok(json) = serde_json::to_string(&suggestions) {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let _ = mixflow_db::ai_cache::set(&conn, &now_title, cache_artist, &json);
    }

    Ok(AiSuggestionResult { suggestions, from_cache: false })
}

fn now_playing_names(conn: &Connection, track_id: i64) -> Result<(String, Option<String>), String> {
    conn.query_row(
        "SELECT title, artist FROM tracks WHERE id = ?1",
        params![track_id],
        |row| {
            let title: Option<String> = row.get(0)?;
            let artist: Option<String> = row.get(1)?;
            Ok((title.unwrap_or_else(|| "Untitled".to_string()), artist))
        },
    )
    .map_err(|e| e.to_string())
}

/// Same duplicate-of-now-playing exclusion as NEXT (`recommend.rs`): a second
/// copy of the song already playing is not a suggestion.
fn load_candidates(conn: &Connection, now_playing_id: i64) -> Result<Vec<Candidate>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, artist, bpm, camelot_key, genre FROM tracks \
             WHERE id != ?1 AND missing_file = 0 AND title IS NOT NULL AND title != '' \
             AND bpm IS NOT NULL AND camelot_key IS NOT NULL \
             AND NOT ( \
                 artist IS NOT NULL \
                 AND lower(trim(title)) = (SELECT lower(trim(title)) FROM tracks WHERE id = ?1) \
                 AND lower(trim(artist)) = (SELECT lower(trim(artist)) FROM tracks WHERE id = ?1) \
             ) \
             ORDER BY RANDOM() LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![now_playing_id, CANDIDATE_POOL as i64], |row| {
            Ok(Candidate {
                track_id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                bpm: row.get(3)?,
                camelot_key: row.get(4)?,
                genre: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[derive(Debug, Deserialize)]
struct ClaudePick {
    n: usize,
    fit: u8,
    reason: String,
}

async fn call_claude(
    api_key: Option<&str>,
    now_title: &str,
    now_artist: Option<&str>,
    candidates: &[Candidate],
) -> Result<Vec<ClaudePick>, String> {
    let list = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            format!(
                "{}. {} — {} ({} BPM, {}{})",
                i + 1,
                c.title,
                c.artist.as_deref().unwrap_or("Unknown artist"),
                c.bpm.map(|b| format!("{}", b.round() as i64)).unwrap_or_default(),
                c.camelot_key.as_deref().unwrap_or(""),
                c.genre.as_deref().map(|g| format!(", {g}")).unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system = "You are helping a DJ pick what to mix in next. You'll be given the track \
        playing now (title and artist only — no audio data) and a numbered list of real tracks \
        from the DJ's own library, each with its actual BPM, Camelot key and genre. Pick only \
        from this list. Use what you know about each song's genre, era, energy and mood to \
        judge which ones would work well played right after the now-playing track for a general \
        crowd. Respond with ONLY a JSON array, no other text, of your top 8 picks, each \
        {\"n\": <candidate number>, \"fit\": <0-100, how good a fit>, \"reason\": \"<one short \
        sentence>\"}, best first.";

    let user = format!(
        "Now playing: \"{now_title}\" by {}\n\nCandidates:\n{list}",
        now_artist.unwrap_or("Unknown artist")
    );

    let client = reqwest::Client::new();
    // A personal key goes straight to Anthropic, authenticated as normal.
    // With none, the request goes to MixInspo's own proxy instead, which
    // holds the real key server-side — the app never sends it.
    let request = match api_key {
        Some(key) => client
            .post(ANTHROPIC_URL)
            .header("x-api-key", key)
            .header("anthropic-version", ANTHROPIC_VERSION),
        None => client.post(PROXY_URL),
    };
    let response = request
        .header("content-type", "application/json")
        .json(&json!({
            "model": MODEL,
            "max_tokens": 2048,
            "system": system,
            "messages": [{"role": "user", "content": user}],
        }))
        .send()
        .await
        .map_err(|e| format!("Could not reach {}: {e}", if api_key.is_some() { "Anthropic" } else { "the AI Suggestions service" }))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("AI Suggestions error ({status}): {body}"));
    }

    let body: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let text = body["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find(|b| b["type"] == "text"))
        .and_then(|b| b["text"].as_str())
        .ok_or("The AI's reply had no text in it.")?;

    parse_picks(text)
}

/// Pulls the JSON array out of the model's reply, tolerating a stray code
/// fence or a sentence before/after it — asked not to add one, but not relied
/// on to obey perfectly.
fn parse_picks(text: &str) -> Result<Vec<ClaudePick>, String> {
    let start = text.find('[').ok_or("Could not find the AI's picks in its reply.")?;
    let end = text.rfind(']').ok_or("Could not find the AI's picks in its reply.")?;
    serde_json::from_str(&text[start..=end]).map_err(|e| format!("Could not read the AI's picks: {e}"))
}

fn resolve_picks(picks: &[ClaudePick], candidates: &[Candidate]) -> Vec<AiSuggestion> {
    picks
        .iter()
        .filter_map(|pick| {
            let candidate = candidates.get(pick.n.checked_sub(1)?)?;
            Some(AiSuggestion {
                track_id: candidate.track_id,
                title: candidate.title.clone(),
                artist: candidate.artist.clone(),
                bpm: candidate.bpm,
                camelot_key: candidate.camelot_key.clone(),
                fit: pick.fit,
                reason: pick.reason.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: i64, title: &str) -> Candidate {
        Candidate {
            track_id: id,
            title: title.to_string(),
            artist: Some("Someone".to_string()),
            bpm: Some(128.0),
            camelot_key: Some("8B".to_string()),
            genre: None,
        }
    }

    #[test]
    fn picks_resolve_to_the_candidate_at_that_position() {
        let candidates = vec![candidate(10, "First"), candidate(20, "Second"), candidate(30, "Third")];
        let picks = vec![
            ClaudePick { n: 2, fit: 88, reason: "fits the vibe".into() },
            ClaudePick { n: 1, fit: 95, reason: "same energy".into() },
        ];
        let resolved = resolve_picks(&picks, &candidates);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].track_id, 20);
        assert_eq!(resolved[0].reason, "fits the vibe");
        assert_eq!(resolved[0].fit, 88);
        assert_eq!(resolved[1].track_id, 10);
    }

    #[test]
    fn an_out_of_range_pick_is_dropped_not_crashed_on() {
        let candidates = vec![candidate(10, "Only One")];
        let picks = vec![ClaudePick { n: 5, fit: 50, reason: "x".into() }, ClaudePick { n: 1, fit: 70, reason: "y".into() }];
        let resolved = resolve_picks(&picks, &candidates);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].track_id, 10);
    }

    #[test]
    fn a_zero_pick_is_dropped_not_crashed_on() {
        let candidates = vec![candidate(10, "Only One")];
        let picks = vec![ClaudePick { n: 0, fit: 50, reason: "x".into() }];
        assert_eq!(resolve_picks(&picks, &candidates).len(), 0);
    }

    #[test]
    fn json_is_parsed_out_of_a_reply_with_surrounding_text() {
        let text = "Here are my picks:\n[{\"n\": 3, \"fit\": 90, \"reason\": \"good match\"}]\nHope that helps!";
        let picks = parse_picks(text).unwrap();
        assert_eq!(picks.len(), 1);
        assert_eq!(picks[0].n, 3);
    }

    #[test]
    fn unparsable_text_is_a_clear_error_not_a_panic() {
        assert!(parse_picks("sorry, I can't help with that").is_err());
    }
}


