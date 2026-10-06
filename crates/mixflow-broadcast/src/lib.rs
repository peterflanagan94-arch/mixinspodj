//! Reads what Traktor Pro sends over its Broadcasting feature.
//!
//! Traktor connects as an Icecast source, sends a `SOURCE` request, and then
//! streams Ogg Vorbis audio. When a track is loaded it also writes a Vorbis
//! comment header into that stream containing `ARTIST=` and `TITLE=`. This
//! crate reassembles the Ogg pages, finds those comment headers, and reports
//! track changes. It never decodes audio.
//!
//! Pure parsing with no networking, so it's testable against bytes captured
//! from a real Traktor session.

mod ogg;
mod vorbis;

pub use ogg::OggStream;
pub use vorbis::{parse_comment_header, TrackTags};

/// Returns the index just past the HTTP-style header block, if it has fully arrived.
pub fn body_start(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|pos| pos + 4)
}

/// Turns a raw Traktor broadcast byte stream into track-change events.
#[derive(Default)]
pub struct TrackDetector {
    ogg: OggStream,
    last: Option<TrackTags>,
}

impl TrackDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds audio bytes (headers already removed) and returns any track changes
    /// they revealed. A repeat of the track already reported is not returned.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<TrackTags> {
        let mut changes = Vec::new();
        for packet in self.ogg.push(bytes) {
            if let Some(tags) = parse_comment_header(&packet) {
                if tags.title.is_some() && self.last.as_ref() != Some(&tags) {
                    self.last = Some(tags.clone());
                    changes.push(tags);
                }
            }
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_header_block_end() {
        let raw = b"SOURCE /x HTTP/1.0\r\nHost: a\r\n\r\nAUDIO";
        assert_eq!(body_start(raw), Some(raw.len() - 5));
        assert_eq!(body_start(b"SOURCE /x HTTP/1.0\r\n"), None);
    }

    #[test]
    fn reads_track_change_from_a_real_traktor_capture() {
        let bytes = include_bytes!("../tests/fixtures/traktor_track_change.ogg");
        let mut detector = TrackDetector::new();
        let changes = detector.feed(bytes);
        assert_eq!(changes.len(), 1, "one track change expected, got {changes:?}");
        assert_eq!(changes[0].artist.as_deref(), Some("Higher Place"));
        assert_eq!(changes[0].title.as_deref(), Some("Diafonie"));
    }

    #[test]
    fn repeated_identical_metadata_is_reported_once() {
        let bytes = include_bytes!("../tests/fixtures/traktor_track_change.ogg");
        let mut detector = TrackDetector::new();
        assert_eq!(detector.feed(bytes).len(), 1);
        assert_eq!(detector.feed(bytes).len(), 0);
    }

    #[test]
    fn reassembles_when_bytes_arrive_in_small_pieces() {
        let bytes = include_bytes!("../tests/fixtures/traktor_track_change.ogg");
        let mut detector = TrackDetector::new();
        let mut all = Vec::new();
        for piece in bytes.chunks(7) {
            all.extend(detector.feed(piece));
        }
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title.as_deref(), Some("Diafonie"));
    }
}
