#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use embedded_graphics::{geometry::Size, mono_font::ascii::FONT_10X20, prelude::Point};

pub const TITLE_TEXT: &str = "WiFi Scan";
pub const MAX_VISIBLE_SSIDS: usize = 5;
pub const MAX_SSID_CHARS: usize = 27;
pub const HIDDEN_SSID_LABEL: &str = "<hidden>";
pub const DISPLAY_WIDTH: u16 = 160;
pub const DISPLAY_HEIGHT: u16 = 80;
pub const DIAGNOSTIC_MARKER_TEXT: &str = "L2";

pub fn display_size() -> Size {
    Size::new(u32::from(DISPLAY_WIDTH), u32::from(DISPLAY_HEIGHT))
}

pub fn centered_top_left(display: Size, content: Size) -> Point {
    let x = display.width.saturating_sub(content.width) / 2;
    let y = display.height.saturating_sub(content.height) / 2;

    Point::new(x as i32, y as i32)
}

pub fn diagnostic_marker_top_left() -> Point {
    let marker_size = Size::new(
        FONT_10X20.character_size.width * DIAGNOSTIC_MARKER_TEXT.chars().count() as u32,
        FONT_10X20.character_size.height,
    );

    centered_top_left(display_size(), marker_size)
}

pub fn ascii_safe(input: &str) -> String {
    let mut sanitized = String::with_capacity(input.len());

    // 第二课先只保证内置 ASCII 字体可稳定显示，后续若要支持中文再单独引入字库。
    for ch in input.chars() {
        if ch.is_ascii() && (!ch.is_ascii_control() || ch == ' ') {
            sanitized.push(ch);
        } else {
            sanitized.push('?');
        }
    }

    sanitized
}

pub fn truncate_with_dots(input: &str, max_chars: usize) -> String {
    let char_count = input.chars().count();
    if char_count <= max_chars {
        return String::from(input);
    }

    if max_chars <= 3 {
        let mut dots = String::with_capacity(max_chars);
        for _ in 0..max_chars {
            dots.push('.');
        }
        return dots;
    }

    let keep = max_chars - 3;
    let mut truncated = String::with_capacity(max_chars);
    for ch in input.chars().take(keep) {
        truncated.push(ch);
    }
    truncated.push_str("...");
    truncated
}

pub fn display_ssid(input: &str, max_chars: usize) -> String {
    if input.is_empty() {
        return String::from(HIDDEN_SSID_LABEL);
    }

    let sanitized = ascii_safe(input);
    truncate_with_dots(&sanitized, max_chars)
}

pub fn format_ssid_lines(ssids: &[&str]) -> Vec<String> {
    let mut lines = Vec::with_capacity(usize::min(ssids.len(), MAX_VISIBLE_SSIDS));

    for (index, ssid) in ssids.iter().take(MAX_VISIBLE_SSIDS).enumerate() {
        let rendered_ssid = display_ssid(ssid, MAX_SSID_CHARS);
        let mut line = String::with_capacity(rendered_ssid.len() + 3);
        line.push(char::from_digit((index + 1) as u32, 10).unwrap_or('?'));
        line.push('.');
        line.push(' ');
        line.push_str(&rendered_ssid);
        lines.push(line);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn ascii_safe_keeps_plain_ascii() {
        assert_eq!(ascii_safe("Cafe_WiFi-5G"), "Cafe_WiFi-5G");
    }

    #[test]
    fn display_size_matches_board_panel() {
        assert_eq!(DISPLAY_WIDTH, 160);
        assert_eq!(DISPLAY_HEIGHT, 80);
        assert_eq!(display_size(), Size::new(160, 80));
    }

    #[test]
    fn diagnostic_marker_top_left_stays_inside_screen() {
        let top_left = diagnostic_marker_top_left();
        let marker_size = Size::new(
            FONT_10X20.character_size.width * DIAGNOSTIC_MARKER_TEXT.chars().count() as u32,
            FONT_10X20.character_size.height,
        );
        let screen = display_size();

        assert_eq!(top_left, Point::new(70, 30));
        assert!(top_left.x >= 0);
        assert!(top_left.y >= 0);
        assert!(top_left.x as u32 + marker_size.width <= screen.width);
        assert!(top_left.y as u32 + marker_size.height <= screen.height);
    }

    #[test]
    fn ascii_safe_replaces_non_ascii_with_question_mark() {
        assert_eq!(ascii_safe("咖啡WiFi"), "??WiFi");
    }

    #[test]
    fn truncate_with_dots_keeps_short_text() {
        assert_eq!(truncate_with_dots("wifi", 8), "wifi");
    }

    #[test]
    fn truncate_with_dots_appends_ascii_dots_when_too_long() {
        assert_eq!(truncate_with_dots("abcdefghij", 8), "abcde...");
    }

    #[test]
    fn display_ssid_uses_hidden_label_for_empty_string() {
        assert_eq!(display_ssid("", MAX_SSID_CHARS), HIDDEN_SSID_LABEL);
    }

    #[test]
    fn format_ssid_lines_limits_item_count_and_numbers_every_row() {
        let lines = format_ssid_lines(&["one", "two", "three", "four", "five", "six"]);

        assert_eq!(
            lines,
            vec![
                String::from("1. one"),
                String::from("2. two"),
                String::from("3. three"),
                String::from("4. four"),
                String::from("5. five")
            ]
        );
    }

    #[test]
    fn format_ssid_lines_sanitizes_and_truncates_each_row() {
        let lines = format_ssid_lines(&["咖啡馆ABCDEFGHIJKLMNOPQRSTUVWXY"]);

        assert_eq!(lines, vec![String::from("1. ???ABCDEFGHIJKLMNOPQRSTU...")]);
    }
}
