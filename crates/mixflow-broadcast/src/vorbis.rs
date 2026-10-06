/// Artist and title as Traktor writes them into the Ogg Vorbis comment header.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrackTags {
    pub artist: Option<String>,
    pub title: Option<String>,
}

/// Reads a Vorbis comment header packet (type 3, "vorbis" signature) and returns
/// its ARTIST and TITLE comments. Returns `None` for any other packet.
pub fn parse_comment_header(packet: &[u8]) -> Option<TrackTags> {
    if packet.len() < 7 || packet[0] != 0x03 || &packet[1..7] != b"vorbis" {
        return None;
    }
    let mut pos = 7;

    let vendor_len = read_u32(packet, &mut pos)? as usize;
    pos = pos.checked_add(vendor_len)?;
    if pos > packet.len() {
        return None;
    }

    let count = read_u32(packet, &mut pos)?;
    let mut tags = TrackTags::default();
    for _ in 0..count {
        let len = read_u32(packet, &mut pos)? as usize;
        let end = pos.checked_add(len)?;
        let comment = packet.get(pos..end)?;
        pos = end;

        let text = String::from_utf8_lossy(comment);
        if let Some((key, value)) = text.split_once('=') {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            if key.eq_ignore_ascii_case("ARTIST") && tags.artist.is_none() {
                tags.artist = Some(value.to_string());
            } else if key.eq_ignore_ascii_case("TITLE") && tags.title.is_none() {
                tags.title = Some(value.to_string());
            }
        }
    }
    Some(tags)
}

fn read_u32(buf: &[u8], pos: &mut usize) -> Option<u32> {
    let bytes = buf.get(*pos..*pos + 4)?;
    *pos += 4;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comment_packet(vendor: &str, comments: &[&str]) -> Vec<u8> {
        let mut out = vec![0x03];
        out.extend_from_slice(b"vorbis");
        out.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        out.extend_from_slice(vendor.as_bytes());
        out.extend_from_slice(&(comments.len() as u32).to_le_bytes());
        for c in comments {
            out.extend_from_slice(&(c.len() as u32).to_le_bytes());
            out.extend_from_slice(c.as_bytes());
        }
        out
    }

    #[test]
    fn reads_artist_and_title() {
        let packet = comment_packet("Native Instruments", &["ARTIST=Higher Place", "TITLE=Diafonie"]);
        assert_eq!(
            parse_comment_header(&packet),
            Some(TrackTags {
                artist: Some("Higher Place".into()),
                title: Some("Diafonie".into()),
            })
        );
    }

    #[test]
    fn key_names_are_case_insensitive_and_other_comments_are_ignored() {
        let packet = comment_packet("x", &["encoder=abc", "artist=Someone", "Title=Something"]);
        let tags = parse_comment_header(&packet).unwrap();
        assert_eq!(tags.artist.as_deref(), Some("Someone"));
        assert_eq!(tags.title.as_deref(), Some("Something"));
    }

    #[test]
    fn audio_packets_are_not_comment_headers() {
        assert_eq!(parse_comment_header(b"\x01vorbis-identification"), None);
        assert_eq!(parse_comment_header(b"random audio bytes"), None);
    }

    #[test]
    fn a_truncated_packet_is_rejected_rather_than_panicking() {
        let mut packet = comment_packet("x", &["TITLE=Long title here"]);
        packet.truncate(packet.len() - 5);
        assert_eq!(parse_comment_header(&packet), None);
    }
}
