#![cfg_attr(not(test), no_std)]

use core::str;

use embedded_graphics::{geometry::Size, prelude::Point};

pub const DISPLAY_WIDTH: u16 = 160;
pub const DISPLAY_HEIGHT: u16 = 80;
pub const TITLE_TEXT: &str = "Codex Panel";
pub const IDLE_TEXT: &str = "No active chats";
pub const MAX_VISIBLE_TITLES: usize = 4;
pub const MAX_TITLE_BYTES: usize = 40;
pub const LINE_BUFFER_BYTES: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelSnapshot {
    pub active: bool,
    pub count: usize,
    titles: [[u8; MAX_TITLE_BYTES]; MAX_VISIBLE_TITLES],
    lengths: [usize; MAX_VISIBLE_TITLES],
}

impl Default for PanelSnapshot {
    fn default() -> Self {
        Self {
            active: false,
            count: 0,
            titles: [[0; MAX_TITLE_BYTES]; MAX_VISIBLE_TITLES],
            lengths: [0; MAX_VISIBLE_TITLES],
        }
    }
}

impl PanelSnapshot {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn set_title(&mut self, index: usize, title: &[u8]) {
        if index >= MAX_VISIBLE_TITLES {
            return;
        }

        let mut written = 0;
        for &byte in title.iter().take(MAX_TITLE_BYTES) {
            self.titles[index][written] = sanitize_ascii(byte);
            written += 1;
        }

        self.lengths[index] = written;
        if index + 1 > self.count {
            self.count = index + 1;
        }
    }

    pub fn title(&self, index: usize) -> &str {
        if index >= self.count {
            return "";
        }

        str::from_utf8(&self.titles[index][..self.lengths[index]]).unwrap_or("")
    }
}

pub struct ProtocolParser {
    line_buffer: [u8; LINE_BUFFER_BYTES],
    line_len: usize,
    in_frame: bool,
    pending: PanelSnapshot,
}

impl ProtocolParser {
    pub fn new() -> Self {
        Self {
            line_buffer: [0; LINE_BUFFER_BYTES],
            line_len: 0,
            in_frame: false,
            pending: PanelSnapshot::default(),
        }
    }

    pub fn push_byte(&mut self, byte: u8) -> Option<PanelSnapshot> {
        match byte {
            b'\r' => None,
            b'\n' => {
                let snapshot = self.process_line();
                self.line_len = 0;
                snapshot
            }
            _ => {
                if self.line_len < LINE_BUFFER_BYTES {
                    self.line_buffer[self.line_len] = byte;
                    self.line_len += 1;
                }
                None
            }
        }
    }

    fn process_line(&mut self) -> Option<PanelSnapshot> {
        let line = &self.line_buffer[..self.line_len];

        if line == b"SNAP" {
            self.in_frame = true;
            self.pending.clear();
            return None;
        }

        if !self.in_frame || line.is_empty() {
            return None;
        }

        if let Some(value) = strip_prefix(line, b"ACTIVE ") {
            self.pending.active = value == b"1";
            return None;
        }

        if let Some(value) = strip_prefix(line, b"COUNT ") {
            self.pending.count = parse_decimal(value).min(MAX_VISIBLE_TITLES);
            return None;
        }

        if let Some(value) = strip_prefix(line, b"TITLE ") {
            if let Some((index, title)) = split_once_space(value) {
                self.pending.set_title(parse_decimal(index), title);
            }
            return None;
        }

        if line == b"END" {
            self.in_frame = false;
            return Some(self.pending);
        }

        None
    }
}

pub fn display_size() -> Size {
    Size::new(u32::from(DISPLAY_WIDTH), u32::from(DISPLAY_HEIGHT))
}

pub fn sanitize_ascii(byte: u8) -> u8 {
    match byte {
        b' '..=b'~' => byte,
        _ => b'?',
    }
}

fn parse_decimal(bytes: &[u8]) -> usize {
    let mut value = 0usize;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            break;
        }
        value = value.saturating_mul(10).saturating_add((byte - b'0') as usize);
    }
    value
}

fn parse_decimal_exact(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() {
        return None;
    }

    if bytes.len() > 1 && bytes[0] == b'0' {
        return None;
    }

    let mut value = 0usize;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add((byte - b'0') as usize)?;
    }
    Some(value)
}

fn strip_prefix<'a>(input: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    input.strip_prefix(prefix)
}

fn split_once_space(input: &[u8]) -> Option<(&[u8], &[u8])> {
    let split_at = input.iter().position(|&byte| byte == b' ')?;
    Some((&input[..split_at], &input[split_at + 1..]))
}

pub fn centered_top_left(display: Size, content: Size) -> Point {
    let x = display.width.saturating_sub(content.width) / 2;
    let y = display.height.saturating_sub(content.height) / 2;

    Point::new(x as i32, y as i32)
}

pub fn parse_ipv4_address(input: &str) -> Option<[u8; 4]> {
    let mut octets = [0_u8; 4];
    let mut count = 0usize;

    for segment in input.split('.') {
        if count >= octets.len() || segment.is_empty() {
            return None;
        }

        let value = parse_decimal_exact(segment.as_bytes())?;
        if value > u8::MAX as usize {
            return None;
        }

        octets[count] = value as u8;
        count += 1;
    }

    if count == octets.len() {
        Some(octets)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_size_matches_board_panel() {
        assert_eq!(display_size(), Size::new(160, 80));
    }

    #[test]
    fn parser_accepts_complete_snapshot() {
        let mut parser = ProtocolParser::new();
        let frame = b"SNAP\nACTIVE 1\nCOUNT 2\nTITLE 0 first chat\nTITLE 1 second chat\nEND\n";
        let mut snapshot = None;

        for &byte in frame {
            snapshot = parser.push_byte(byte).or(snapshot);
        }

        let snapshot = snapshot.expect("应当成功组帧");
        assert!(snapshot.active);
        assert_eq!(snapshot.count, 2);
        assert_eq!(snapshot.title(0), "first chat");
        assert_eq!(snapshot.title(1), "second chat");
    }

    #[test]
    fn parser_sanitizes_non_ascii_title_bytes() {
        let mut parser = ProtocolParser::new();
        let frame = b"SNAP\nACTIVE 1\nCOUNT 1\nTITLE 0 abc\xffxyz\nEND\n";
        let mut snapshot = None;

        for &byte in frame {
            snapshot = parser.push_byte(byte).or(snapshot);
        }

        let snapshot = snapshot.expect("应当成功组帧");
        assert_eq!(snapshot.title(0), "abc?xyz");
    }

    #[test]
    fn centered_top_left_clamps_inside_screen() {
        let top_left = centered_top_left(display_size(), Size::new(80, 16));
        assert!(top_left.x >= 0);
        assert!(top_left.y >= 0);
    }

    #[test]
    fn parse_ipv4_address_accepts_dotted_decimal() {
        assert_eq!(parse_ipv4_address("192.168.31.52"), Some([192, 168, 31, 52]));
    }

    #[test]
    fn parse_ipv4_address_rejects_invalid_octets() {
        assert_eq!(parse_ipv4_address("192.168.31.999"), None);
        assert_eq!(parse_ipv4_address("192.168.31"), None);
        assert_eq!(parse_ipv4_address("hello"), None);
    }
}
