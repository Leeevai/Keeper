//! The text being typed.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthChar;

use super::Styles;
use crate::settings::{CaretShape, WhitespaceStyle};
use crate::textinput::{Attr, Char};
use crate::unicode::SPACE;

pub struct TextView<'a> {
    pub chars: &'a [Char],
    pub styles: &'a Styles,
    pub whitespace: WhitespaceStyle,
    pub caret: CaretShape,
    /// Lay out the lines from right to left.
    pub rtl: bool,
    /// A wrong key was pressed at the cursor.
    pub typo: bool,
}

/// The display width of a character, at least one column.
fn char_width(cp: u32) -> usize {
    char::from_u32(cp)
        .and_then(|c| c.width())
        .unwrap_or(1)
        .max(1)
}

/// Splits the text into lines of at most `width` columns, breaking after spaces.
pub fn wrap(chars: &[Char], width: usize) -> Vec<Range<usize>> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut start = 0;
    let mut line_width = 0;
    let mut i = 0;
    while i < chars.len() {
        // A word with its trailing spaces.
        let mut end = i;
        while end < chars.len() && chars[end].cp != SPACE {
            end += 1;
        }
        while end < chars.len() && chars[end].cp == SPACE {
            end += 1;
        }
        let word: usize = chars[i..end].iter().map(|c| char_width(c.cp)).sum();
        let trailing: usize = chars[i..end]
            .iter()
            .rev()
            .take_while(|c| c.cp == SPACE)
            .count();
        if line_width > 0 && line_width + word - trailing > width {
            lines.push(start..i);
            start = i;
            line_width = 0;
        }
        if word - trailing > width {
            // A word longer than a line is broken anywhere.
            let mut j = i;
            while j < end {
                let w = char_width(chars[j].cp);
                if line_width + w > width && line_width > 0 {
                    lines.push(start..j);
                    start = j;
                    line_width = 0;
                }
                line_width += w;
                j += 1;
            }
        } else {
            line_width += word;
        }
        i = end;
    }
    if start < chars.len() || lines.is_empty() {
        lines.push(start..chars.len());
    }
    lines
}
