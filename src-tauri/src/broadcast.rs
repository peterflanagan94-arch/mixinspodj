use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use mixflow_broadcast::{body_start, TrackDetector, TrackTags};
use mixflow_db::library;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::DbState;

/// The address and port to enter in Traktor's Broadcasting settings.
pub const LISTEN_ADDRESS: &str = "127.0.0.1";
pub const LISTEN_PORT: u16 = 8765;

#[derive(Debug, Clone, Serialize)]
pub struct TraktorTrack {
    pub artist: Option<String>,
    pub title: Option<String>,
    /// The matching track in the library, if there is one.
    pub track_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraktorStatus {
    /// "listening", "connected", or "port-busy"
    pub state: String,
    pub address: String,
    pub port: u16,
}

/// Starts listening for Traktor on a background thread, so it never blocks the UI.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let listener = match TcpListener::bind((LISTEN_ADDRESS, LISTEN_PORT)) {
            Ok(listener) => listener,
            Err(_) => {
                emit_status(&app, "port-busy");
                return;
            }
        };
        emit_status(&app, "listening");
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            std::thread::spawn(move || handle(stream, app));
        }
    });
}

fn handle(mut stream: TcpStream, app: AppHandle) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    emit_status(&app, "connected");

    let mut detector = TrackDetector::new();
    let mut headers = Vec::new();
    let mut headers_done = false;
    let mut buf = [0u8; 8192];

    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };

        if headers_done {
            for tags in detector.feed(&buf[..n]) {
                report(&app, tags);
            }
            continue;
        }

        headers.extend_from_slice(&buf[..n]);
        if let Some(end) = body_start(&headers) {
            let _ = stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n");
            headers_done = true;
            let body = headers.split_off(end);
            for tags in detector.feed(&body) {
                report(&app, tags);
            }
            headers.clear();
        }
    }

    emit_status(&app, "listening");
}

fn report(app: &AppHandle, tags: TrackTags) {
    let track_id = match (tags.artist.as_deref(), tags.title.as_deref()) {
        (Some(artist), Some(title)) => {
            let db = app.state::<DbState>();
            let found = match db.0.lock() {
                Ok(conn) => library::find_track_by_artist_title(&conn, artist, title).ok().flatten(),
                Err(_) => None,
            };
            found
        }
        _ => None,
    };

    let _ = app.emit(
        "traktor-track",
        TraktorTrack {
            artist: tags.artist,
            title: tags.title,
            track_id,
        },
    );
}

fn emit_status(app: &AppHandle, state: &str) {
    let _ = app.emit(
        "traktor-status",
        TraktorStatus {
            state: state.to_string(),
            address: LISTEN_ADDRESS.to_string(),
            port: LISTEN_PORT,
        },
    );
}
