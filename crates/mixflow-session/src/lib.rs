//! Live-gig state: the current NOW PLAYING track, played-track history for
//! the active gig, and transition logging that feeds `transitions` in
//! `mixflow-db` for future learning.
//!
//! Not implemented yet — this is the target of milestones M4 (now playing)
//! and M7 (play tracking / already-played detection).

pub struct NowPlaying {
    pub track_id: i64,
}
