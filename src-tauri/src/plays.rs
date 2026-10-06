use mixflow_db::plays;
use tauri::State;

use crate::DbState;

/// The live session every play is recorded against, created at launch.
pub struct CurrentSession(pub i64);

/// Called whenever a track becomes NOW PLAYING. `rank` is its position in the
/// recommendations on screen when it was chosen, if it came from there.
/// `detected` is set when Traktor reported the track playing.
#[tauri::command]
pub fn play_track(
    db: State<DbState>,
    session: State<CurrentSession>,
    track_id: i64,
    previous_track_id: Option<i64>,
    mode: String,
    rank: Option<i64>,
    detected: Option<bool>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    plays::record_play(
        &conn,
        session.0,
        track_id,
        previous_track_id,
        &mode,
        rank,
        detected.unwrap_or(false),
    )
    .map_err(|e| e.to_string())
}
