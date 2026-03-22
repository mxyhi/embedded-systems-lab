#![cfg_attr(not(test), no_std)]

use embedded_graphics::{geometry::Size, mono_font::ascii::FONT_10X20, prelude::Point};

pub const HELLO_WORLD_TEXT: &str = "Hello World";
pub const DISPLAY_WIDTH: u16 = 160;
pub const DISPLAY_HEIGHT: u16 = 80;

pub fn display_size() -> Size {
    Size::new(u32::from(DISPLAY_WIDTH), u32::from(DISPLAY_HEIGHT))
}

pub fn hello_world_text_size() -> Size {
    let glyph_width = FONT_10X20.character_size.width;
    let glyph_height = FONT_10X20.character_size.height;
    let text_width = glyph_width * HELLO_WORLD_TEXT.len() as u32;

    Size::new(text_width, glyph_height)
}

pub fn centered_top_left(display: Size, content: Size) -> Point {
    // 内容比屏幕大时直接钳到 0，避免生成负坐标。
    let x = display.width.saturating_sub(content.width) / 2;
    let y = display.height.saturating_sub(content.height) / 2;

    Point::new(x as i32, y as i32)
}

pub fn hello_world_top_left() -> Point {
    centered_top_left(display_size(), hello_world_text_size())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_size_matches_board_panel() {
        assert_eq!(DISPLAY_WIDTH, 160);
        assert_eq!(DISPLAY_HEIGHT, 80);
        assert_eq!(display_size(), Size::new(160, 80));
    }

    #[test]
    fn hello_world_text_size_matches_font_metrics() {
        let expected_width =
            FONT_10X20.character_size.width * HELLO_WORLD_TEXT.chars().count() as u32;
        let expected_height = FONT_10X20.character_size.height;

        assert_eq!(
            hello_world_text_size(),
            Size::new(expected_width, expected_height)
        );
    }

    #[test]
    fn centered_origin_for_hello_world_stays_inside_screen() {
        let top_left = hello_world_top_left();
        let text_size = hello_world_text_size();
        let screen = display_size();

        assert_eq!(top_left, Point::new(25, 30));
        assert!(top_left.x >= 0);
        assert!(top_left.y >= 0);
        assert!(top_left.x as u32 + text_size.width <= screen.width);
        assert!(top_left.y as u32 + text_size.height <= screen.height);
    }
}
