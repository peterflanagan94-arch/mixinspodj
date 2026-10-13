//! Imports a public Spotify playlist's track list and matches it against
//! the DJ's own library by title and artist. MixInspo DJ never streams or
//! sources new audio — this only ever finds tracks the DJ already owns,
//! and says plainly which ones it couldn't, the same honesty as the
//! Traktor and VirtualDJ imports.
//!
//! Needs the DJ's own Spotify API credentials (Settings → Spotify): a free
//! Developer app using the Client Credentials flow, which only needs a
//! Spotify *developer* account, not a personal login — it can read any
//! public playlist's track list without anyone signing in.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{settings, DbState};

const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const API_BASE: &str = "https://api.spotify.com/v1";
/// However long the playlist, matching stops considering more entries —
/// keeps a pathological "my entire liked songs" link from hammering the
/// Spotify API or taking a set builder step for every single one.
const MAX_TRACKS: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct SpotifyTrackDto {
    pub title: String,
    pub artist: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchedTrackDto {
    pub track_id: i64,
    pub spotify_title: String,
    pub spotify_artist: String,
    pub library_title: String,
    pub library_artist: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SpotifyImportSummary {
    pub playlist_name: String,
    pub matched: Vec<MatchedTrackDto>,
    pub unmatched: Vec<SpotifyTrackDto>,
}

#[tauri::command]
pub fn has_spotify_credentials(app: AppHandle) -> bool {
    let s = settings::load(&app);
    s.spotify_client_id.is_some_and(|v| !v.trim().is_empty())
        && s.spotify_client_secret.is_some_and(|v| !v.trim().is_empty())
}

#[tauri::command]
pub fn set_spotify_credentials(app: AppHandle, client_id: String, client_secret: String) -> Result<(), String> {
    let mut current = settings::load(&app);
    current.spotify_client_id = Some(client_id.trim().to_string());
    current.spotify_client_secret = Some(client_secret.trim().to_string());
    settings::save(&app, &current)
}

#[tauri::command]
pub fn clear_spotify_credentials(app: AppHandle) -> Result<(), String> {
    let mut current = settings::load(&app);
    current.spotify_client_id = None;
    current.spotify_client_secret = None;
    settings::save(&app, &current)
}

/// Fetches a public playlist's track list from Spotify and matches it
/// against the library. Network happens before any database lock is taken
/// — the lock is only ever held for the quick, synchronous matching pass.
#[tauri::command(async)]
pub async fn import_spotify_playlist(
    app: AppHandle,
    db: State<'_, DbState>,
    url: String,
) -> Result<SpotifyImportSummary, String> {
    let settings = settings::load(&app);
    let client_id = settings
        .spotify_client_id
        .filter(|v| !v.trim().is_empty())
        .ok_or("Add your Spotify Client ID and Secret in Settings → Spotify first.")?;
    let client_secret = settings
        .spotify_client_secret
        .filter(|v| !v.trim().is_empty())
        .ok_or("Add your Spotify Client ID and Secret in Settings → Spotify first.")?;
    let playlist_id = parse_playlist_id(&url)
        .ok_or("That doesn't look like a Spotify playlist link — paste the share link for a playlist.")?;

    let client = reqwest::Client::new();
    let token = get_access_token(&client, &client_id, &client_secret).await?;
    let (playlist_name, spotify_tracks) = fetch_playlist_tracks(&client, &token, &playlist_id).await?;

    let library = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_library_index(&conn).map_err(|e| e.to_string())?
    };

    let (matched, unmatched) = match_tracks(&spotify_tracks, &library);
    Ok(SpotifyImportSummary { playlist_name, matched, unmatched })
}

/// A playlist share link looks like `https://open.spotify.com/playlist/<id>?si=...`,
/// a Spotify URI looks like `spotify:playlist:<id>` — either way, the id is
/// the run of base62 characters right after `playlist/` or `playlist:`.
fn parse_playlist_id(input: &str) -> Option<String> {
    let input = input.trim();
    let after = input.split("playlist/").nth(1).or_else(|| input.split("playlist:").nth(1))?;
    let id: String = after.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
    (!id.is_empty()).then_some(id)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

async fn get_access_token(client: &reqwest::Client, client_id: &str, client_secret: &str) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let credentials = STANDARD.encode(format!("{client_id}:{client_secret}"));

    let response = client
        .post(TOKEN_URL)
        .header("Authorization", format!("Basic {credentials}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials")
        .send()
        .await
        .map_err(|e| format!("Could not reach Spotify: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(format!(
            "Spotify rejected those credentials ({status}). Check the Client ID and Secret in Settings → Spotify."
        ));
    }
    response
        .json::<TokenResponse>()
        .await
        .map(|t| t.access_token)
        .map_err(|e| format!("Unexpected response from Spotify: {e}"))
}

#[derive(Deserialize)]
struct PlaylistMeta {
    name: String,
}

#[derive(Deserialize)]
struct TracksPage {
    items: Vec<PlaylistItem>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct PlaylistItem {
    track: Option<TrackObject>,
}

#[derive(Deserialize)]
struct TrackObject {
    name: Option<String>,
    #[serde(default)]
    artists: Vec<ArtistObject>,
}

#[derive(Deserialize)]
struct ArtistObject {
    name: String,
}

async fn get_json<T: serde::de::DeserializeOwned>(client: &reqwest::Client, url: &str, token: &str) -> Result<T, String> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Could not reach Spotify: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Spotify error ({status}): {}", spotify_error_message(&body)));
    }
    response.json::<T>().await.map_err(|e| format!("Unexpected response from Spotify: {e}"))
}

/// Spotify's own error responses look like `{"error": {"message": "..."}}` —
/// that message is the real reason (a Premium-only restriction, a private
/// or collaborative playlist, a bad id), so it's shown as-is rather than
/// guessed at. Falls back to the raw body only if that shape isn't there.
fn spotify_error_message(body: &str) -> String {
    #[derive(Deserialize)]
    struct ErrorBody {
        error: ErrorDetail,
    }
    #[derive(Deserialize)]
    struct ErrorDetail {
        message: String,
    }
    if let Ok(e) = serde_json::from_str::<ErrorBody>(body) {
        return e.error.message;
    }
    // Not the shape Spotify's playlist API normally returns — rather than
    // a guessed explanation that might be wrong, show what it actually
    // sent back (trimmed, in case it's an HTML error page).
    let trimmed = body.trim();
    if trimmed.is_empty() {
        "the playlist may be private, or the link may be wrong (Spotify sent no further detail)".to_string()
    } else {
        trimmed.chars().take(300).collect()
    }
}

async fn fetch_playlist_tracks(
    client: &reqwest::Client,
    token: &str,
    playlist_id: &str,
) -> Result<(String, Vec<SpotifyTrackDto>), String> {
    let meta: PlaylistMeta = get_json(client, &format!("{API_BASE}/playlists/{playlist_id}?fields=name"), token).await?;

    let mut url = format!(
        "{API_BASE}/playlists/{playlist_id}/tracks?fields=items(track(name,artists(name))),next&limit=100"
    );
    let mut tracks = Vec::new();
    loop {
        let page: TracksPage = get_json(client, &url, token).await?;
        for item in page.items {
            if tracks.len() >= MAX_TRACKS {
                break;
            }
            let Some(t) = item.track else { continue };
            let Some(name) = t.name else { continue };
            let artist = t.artists.first().map(|a| a.name.clone()).unwrap_or_default();
            tracks.push(SpotifyTrackDto { title: name, artist });
        }
        match page.next {
            Some(next) if tracks.len() < MAX_TRACKS => url = next,
            _ => break,
        }
    }
    Ok((meta.name, tracks))
}

struct LibraryTrack {
    id: i64,
    title: String,
    artist: Option<String>,
}

fn load_library_index(conn: &Connection) -> rusqlite::Result<Vec<LibraryTrack>> {
    let mut stmt = conn.prepare("SELECT id, title, artist FROM tracks WHERE missing_file = 0 AND title IS NOT NULL AND title != ''")?;
    let rows = stmt.query_map([], |row| {
        Ok(LibraryTrack { id: row.get(0)?, title: row.get(1)?, artist: row.get(2)? })
    })?;
    rows.collect()
}

/// Strips the kind of bracketed noise ("(feat. X)", "(Remastered 2011)")
/// and trailing " - Radio Edit"-style suffixes that commonly differ
/// between a Spotify listing and a DJ's own file tags for what's
/// otherwise the same song, then lowercases and trims for comparison.
/// Heuristic, not exact — titles that genuinely contain " - " will lose
/// the part after it, which is the right trade for matching real-world
/// tag differences rather than demanding an exact string match that
/// would almost never hit.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth: i32 = 0;
    for c in s.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split(" - ").next().unwrap_or(&out).trim().to_lowercase()
}

fn match_tracks(spotify_tracks: &[SpotifyTrackDto], library: &[LibraryTrack]) -> (Vec<MatchedTrackDto>, Vec<SpotifyTrackDto>) {
    let mut exact: HashMap<(String, String), &LibraryTrack> = HashMap::new();
    for track in library {
        let key = (normalize(&track.title), normalize(track.artist.as_deref().unwrap_or("")));
        exact.entry(key).or_insert(track);
    }

    let mut matched = Vec::new();
    let mut unmatched = Vec::new();
    let mut used_ids = HashSet::new();

    for s in spotify_tracks {
        let title_n = normalize(&s.title);
        let artist_n = normalize(&s.artist);

        let found = exact
            .get(&(title_n.clone(), artist_n.clone()))
            .copied()
            // A compound credited artist ("Daft Punk, Pharrell Williams") on one
            // side and a single name on the other is still the same song — match
            // on title plus either artist string containing the other.
            .or_else(|| {
                library.iter().find(|t| {
                    normalize(&t.title) == title_n && {
                        let lib_artist_n = normalize(t.artist.as_deref().unwrap_or(""));
                        !lib_artist_n.is_empty()
                            && !artist_n.is_empty()
                            && (lib_artist_n.contains(&artist_n) || artist_n.contains(&lib_artist_n))
                    }
                })
            })
            .filter(|t| !used_ids.contains(&t.id));

        match found {
            Some(t) => {
                used_ids.insert(t.id);
                matched.push(MatchedTrackDto {
                    track_id: t.id,
                    spotify_title: s.title.clone(),
                    spotify_artist: s.artist.clone(),
                    library_title: t.title.clone(),
                    library_artist: t.artist.clone(),
                });
            }
            None => unmatched.push(s.clone()),
        }
    }

    (matched, unmatched)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_id_parses_from_a_share_link() {
        assert_eq!(
            parse_playlist_id("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc123"),
            Some("37i9dQZF1DXcBWIGoYBM5M".to_string())
        );
    }

    #[test]
    fn playlist_id_parses_from_a_bare_link() {
        assert_eq!(
            parse_playlist_id("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M"),
            Some("37i9dQZF1DXcBWIGoYBM5M".to_string())
        );
    }

    #[test]
    fn playlist_id_parses_from_a_spotify_uri() {
        assert_eq!(parse_playlist_id("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M"), Some("37i9dQZF1DXcBWIGoYBM5M".to_string()));
    }

    #[test]
    fn a_non_playlist_link_fails_to_parse_rather_than_guessing() {
        assert_eq!(parse_playlist_id("https://open.spotify.com/track/abc123"), None);
        assert_eq!(parse_playlist_id("not a link at all"), None);
    }

    #[test]
    fn spotify_s_own_error_message_is_shown_rather_than_a_guess() {
        let body = r#"{"error": {"status": 403, "message": "Your application is blocked from accessing the Web API since you do not have a Spotify Premium subscription."}}"#;
        assert_eq!(
            spotify_error_message(body),
            "Your application is blocked from accessing the Web API since you do not have a Spotify Premium subscription."
        );
    }

    #[test]
    fn an_unexpected_body_shape_is_shown_verbatim_rather_than_guessed() {
        assert_eq!(spotify_error_message("not json"), "not json");
    }

    #[test]
    fn an_empty_body_gets_a_plain_explanation_rather_than_nothing() {
        assert_eq!(
            spotify_error_message(""),
            "the playlist may be private, or the link may be wrong (Spotify sent no further detail)"
        );
    }

    #[test]
    fn normalize_strips_bracketed_noise_and_dash_suffixes() {
        assert_eq!(normalize("One More Time (Radio Edit)"), "one more time");
        assert_eq!(normalize("One More Time - Remastered 2011"), "one more time");
        assert_eq!(normalize("Get Lucky [feat. Pharrell Williams]"), "get lucky");
    }

    fn lib(id: i64, title: &str, artist: &str) -> LibraryTrack {
        LibraryTrack { id, title: title.to_string(), artist: Some(artist.to_string()) }
    }

    fn sp(title: &str, artist: &str) -> SpotifyTrackDto {
        SpotifyTrackDto { title: title.to_string(), artist: artist.to_string() }
    }

    #[test]
    fn matches_an_exact_title_and_artist() {
        let library = vec![lib(1, "One More Time", "Daft Punk")];
        let (matched, unmatched) = match_tracks(&[sp("One More Time", "Daft Punk")], &library);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].track_id, 1);
        assert!(unmatched.is_empty());
    }

    #[test]
    fn matches_despite_tag_noise_differences() {
        let library = vec![lib(1, "Get Lucky - Radio Edit", "Daft Punk, Pharrell Williams")];
        let (matched, _) = match_tracks(&[sp("Get Lucky", "Daft Punk")], &library);
        assert_eq!(matched.len(), 1, "compound artist credit and a dash suffix shouldn't block the match");
    }

    #[test]
    fn a_track_not_in_the_library_is_reported_as_unmatched_not_skipped_silently() {
        let library = vec![lib(1, "One More Time", "Daft Punk")];
        let (matched, unmatched) = match_tracks(&[sp("Harder Better Faster Stronger", "Daft Punk")], &library);
        assert!(matched.is_empty());
        assert_eq!(unmatched.len(), 1);
        assert_eq!(unmatched[0].title, "Harder Better Faster Stronger");
    }

    #[test]
    fn the_same_library_track_is_never_matched_to_two_different_playlist_entries() {
        let library = vec![lib(1, "One More Time", "Daft Punk")];
        let (matched, unmatched) = match_tracks(&[sp("One More Time", "Daft Punk"), sp("One More Time", "Daft Punk")], &library);
        assert_eq!(matched.len(), 1);
        assert_eq!(unmatched.len(), 1, "the second listing of the same song has nothing left to match");
    }
}
