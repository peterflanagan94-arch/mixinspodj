/// Reassembles Ogg packets from a byte stream that can be split anywhere.
///
/// Ogg wraps packets in pages. A packet can span several segments, and can
/// continue across pages, so the partial packet is carried between pages.
#[derive(Default)]
pub struct OggStream {
    buffer: Vec<u8>,
    partial: Vec<u8>,
}

const PAGE_HEADER_LEN: usize = 27;

impl OggStream {
    /// Adds bytes and returns every packet completed by them.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(bytes);
        let mut packets = Vec::new();

        loop {
            let Some(start) = find_capture_pattern(&self.buffer) else {
                // Keep only a possible partial "OggS" at the end.
                let keep = self.buffer.len().min(3);
                let drop_to = self.buffer.len() - keep;
                self.buffer.drain(..drop_to);
                break;
            };
            if start > 0 {
                self.buffer.drain(..start);
            }
            if self.buffer.len() < PAGE_HEADER_LEN {
                break;
            }
            let segment_count = self.buffer[26] as usize;
            let header_end = PAGE_HEADER_LEN + segment_count;
            if self.buffer.len() < header_end {
                break;
            }
            let segment_table = &self.buffer[PAGE_HEADER_LEN..header_end];
            let data_len: usize = segment_table.iter().map(|&s| s as usize).sum();
            let page_end = header_end + data_len;
            if self.buffer.len() < page_end {
                break;
            }

            let mut offset = header_end;
            for &segment_len in segment_table {
                let segment_len = segment_len as usize;
                self.partial.extend_from_slice(&self.buffer[offset..offset + segment_len]);
                offset += segment_len;
                if segment_len < 255 {
                    packets.push(std::mem::take(&mut self.partial));
                }
            }

            self.buffer.drain(..page_end);
        }

        packets
    }
}

fn find_capture_pattern(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"OggS")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds one Ogg page holding the given packets, each packet fully contained.
    fn page(packets: &[&[u8]]) -> Vec<u8> {
        let mut segments = Vec::new();
        let mut data = Vec::new();
        for packet in packets {
            let mut remaining = packet.len();
            while remaining >= 255 {
                segments.push(255u8);
                remaining -= 255;
            }
            segments.push(remaining as u8);
            data.extend_from_slice(packet);
        }
        raw_page(&segments, &data)
    }

    /// Builds a page from an explicit segment table, so a packet can be left open.
    fn raw_page(segments: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"OggS");
        out.push(0);
        out.push(0);
        out.extend_from_slice(&[0u8; 8]);
        out.extend_from_slice(&[0u8; 4]);
        out.extend_from_slice(&[0u8; 4]);
        out.extend_from_slice(&[0u8; 4]);
        out.push(segments.len() as u8);
        out.extend_from_slice(segments);
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn splits_a_page_into_its_packets() {
        let bytes = page(&[b"first", b"second"]);
        let packets = OggStream::default().push(&bytes);
        assert_eq!(packets, vec![b"first".to_vec(), b"second".to_vec()]);
    }

    #[test]
    fn a_packet_longer_than_one_segment_is_reassembled() {
        let long = vec![7u8; 600];
        let packets = OggStream::default().push(&page(&[&long]));
        assert_eq!(packets, vec![long]);
    }

    #[test]
    fn a_packet_split_across_two_pages_is_joined() {
        let long = vec![9u8; 300];
        let (head, tail) = long.split_at(255);
        // First page: one full 255-byte segment, no terminator, so the packet stays open.
        let mut bytes = raw_page(&[255], head);
        // Second page: the 45-byte remainder, which terminates the packet.
        bytes.extend_from_slice(&raw_page(&[tail.len() as u8], tail));
        let packets = OggStream::default().push(&bytes);
        assert_eq!(packets, vec![long]);
    }

    #[test]
    fn leading_junk_before_a_page_is_skipped() {
        let mut bytes = b"junk junk ".to_vec();
        bytes.extend_from_slice(&page(&[b"ok"]));
        assert_eq!(OggStream::default().push(&bytes), vec![b"ok".to_vec()]);
    }
}
