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

impl TextView<'_> {
    fn glyph(&self, ch: &Char) -> String {
        if ch.cp == SPACE {
            return self.whitespace.symbol().to_string();
        }
        let c = char::from_u32(ch.cp).unwrap_or('?');
        if c.width() == Some(0) {
            format!("◌{c}")
        } else {
            c.to_string()
        }
    }

    fn style(&self, ch: &Char) -> Style {
        let s = self.styles;
        let space = ch.cp == SPACE;
        match ch.attr {
            Attr::Normal if space => s.special,
            Attr::Normal => s.syntax(ch.cls),
            Attr::Hit => s.hit,
            Attr::Miss => s.miss,
            Attr::Garbage => s.garbage,
            Attr::Cursor => {
                let base = if self.typo {
                    s.miss
                } else if space {
                    s.special
                } else {
                    s.syntax(ch.cls)
                };
                let accent = s.color(s.palette.accent);
                match self.caret {
                    CaretShape::Block => {
                        let fg = if s.themed {
                            s.color(s.palette.background)
                        } else {
                            ratatui::style::Color::Reset
                        };
                        let bg = if s.themed {
                            s.color(s.palette.text)
                        } else {
                            ratatui::style::Color::Reset
                        };
                        if s.themed {
                            Style::new().fg(fg).bg(bg)
                        } else {
                            base.add_modifier(Modifier::REVERSED)
                        }
                    }
                    CaretShape::Box => {
                        let bg = s.color(if s.themed {
                            s.palette.border
                        } else {
                            s.palette.special
                        });
                        base.bg(bg).add_modifier(Modifier::BOLD)
                    }
                    CaretShape::Line => base.add_modifier(Modifier::BOLD),
                    CaretShape::Underline => base
                        .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
                        .underline_color(accent),
                }
            }
        }
    }

    /// The number of lines needed to show the text in the given width.
    pub fn height(&self, width: u16) -> u16 {
        wrap(self.chars, width as usize).len() as u16
    }

    /// Renders the text centered in the area and returns the position of
    /// the cursor, which is used by the line caret.
    pub fn render(&self, area: Rect, buf: &mut Buffer) -> Option<Position> {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return None;
        }
        let lines = wrap(self.chars, area.width as usize);
        let mut cursor = None;
        for (row, range) in lines.iter().enumerate().take(area.height as usize) {
            let y = area.y + row as u16;
            let mut chars: Vec<&Char> = self.chars[range.clone()].iter().collect();
            if self.rtl {
                chars.reverse();
            }
            let line_width: usize = chars.iter().map(|c| char_width(c.cp)).sum();
            let mut x = if self.rtl {
                area.x + (area.width as usize).saturating_sub(line_width) as u16
            } else {
                area.x
            };
            for ch in chars {
                let w = char_width(ch.cp) as u16;
                if x + w > area.x + area.width {
                    break;
                }
                if ch.attr == Attr::Cursor {
                    cursor = Some(Position::new(x, y));
                }
                buf.set_string(x, y, self.glyph(ch), self.style(ch));
                x += w;
            }
        }
        cursor
    }
}

impl TextView<'_> {
    /// Renders the text on a single line without wrapping. When the text
    /// is wider than the area and `follow` is set, it scrolls so that the
    /// cursor stays in view.
    pub fn render_line(&self, area: Rect, buf: &mut Buffer, follow: bool) -> Option<Position> {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return None;
        }
        let width = area.width as usize;
        let widths: Vec<usize> = self.chars.iter().map(|c| char_width(c.cp)).collect();
        let mut start = 0;
        if follow && widths.iter().sum::<usize>() > width {
            let cursor = self
                .chars
                .iter()
                .position(|c| c.attr == Attr::Cursor)
                .unwrap_or(self.chars.len());
            // Keep a few characters of context after the cursor.
            let margin = (width / 4).min(8);
            let cursor_col: usize = widths[..cursor].iter().sum();
            let mut dropped = 0;
            while start < cursor && cursor_col - dropped + margin + 1 > width {
                dropped += widths[start];
                start += 1;
            }
        }
        let mut cursor = None;
        let mut x = area.x;
        for (ch, w) in self.chars[start..].iter().zip(&widths[start..]) {
            let w = *w as u16;
            if x + w > area.x + area.width {
                break;
            }
            if ch.attr == Attr::Cursor {
                cursor = Some(Position::new(x, area.y));
            }
            buf.set_string(x, area.y, self.glyph(ch), self.style(ch));
            x += w;
        }
        cursor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ColorMode;
    use crate::textinput::{TextInput, TextInputSettings, plain};
    use crate::theme::Theme;

    fn chars(s: &str) -> Vec<Char> {
        s.chars()
            .map(|c| Char {
                cp: u32::from(c),
                attr: Attr::Normal,
                cls: None,
            })
            .collect()
    }

    fn lines(s: &str, width: usize) -> Vec<String> {
        let c = chars(s);
        wrap(&c, width)
            .into_iter()
            .map(|r| s.chars().skip(r.start).take(r.len()).collect())
            .collect()
    }

    #[test]
    fn wraps_after_spaces() {
        assert_eq!(lines("aaa bbb ccc", 7), vec!["aaa bbb ", "ccc"]);
        assert_eq!(lines("aaa bbb ccc", 8), vec!["aaa bbb ", "ccc"]);
        assert_eq!(lines("aaa bbb ccc", 11), vec!["aaa bbb ccc"]);
        assert_eq!(lines("aaaaaaaaaa b", 4), vec!["aaaa", "aaaa", "aa b"]);
        assert_eq!(lines("", 10), vec![""]);
    }

    #[test]
    fn renders_attributes_and_caret() {
        let styles = Styles::new(Theme::get("light"), ColorMode::Truecolor);
        let mut input = TextInput::new(plain("ab cd"), TextInputSettings::default());
        input.append_char(0.0, u32::from('a'), 0.0);
        input.append_char(1.0, u32::from('x'), 0.0);
        let output = input.output();
        for (caret, cursor) in [
            (CaretShape::Line, true),
            (CaretShape::Block, true),
            (CaretShape::Underline, true),
        ] {
            let view = TextView {
                chars: &output,
                styles: &styles,
                whitespace: WhitespaceStyle::Bullet,
                caret,
                rtl: false,
                typo: false,
            };
            let mut buf = Buffer::empty(Rect::new(0, 0, 10, 2));
            let pos = view.render(buf.area, &mut buf);
            assert_eq!(pos.is_some(), cursor);
            assert_eq!(pos.unwrap(), Position::new(1, 0));
            let row: String = (0..5).map(|x| buf[(x, 0)].symbol().to_string()).collect();
            assert_eq!(row, "ab·cd");
            assert_eq!(buf[(0, 0)].fg, styles.hit.fg.unwrap());
        }
    }

    #[test]
    fn right_to_left_lines_are_reversed_and_right_aligned() {
        let styles = Styles::new(Theme::get("system"), ColorMode::Ansi256);
        let c = chars("אב");
        let view = TextView {
            chars: &c,
            styles: &styles,
            whitespace: WhitespaceStyle::Space,
            caret: CaretShape::Block,
            rtl: true,
            typo: false,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 1));
        view.render(buf.area, &mut buf);
        let row: String = (0..6).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert_eq!(row, "    בא");
    }

    #[test]
    fn a_single_line_scrolls_to_the_cursor() {
        let styles = Styles::new(Theme::get("light"), ColorMode::Truecolor);
        let mut c = chars("abcdefghijklmnopqrstuvwxyz");
        c[20].attr = Attr::Cursor;
        let view = TextView {
            chars: &c,
            styles: &styles,
            whitespace: WhitespaceStyle::Space,
            caret: CaretShape::Underline,
            rtl: false,
            typo: false,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        let pos = view.render_line(buf.area, &mut buf, true).unwrap();
        let row: String = (0..10).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert_eq!(buf[(pos.x, 0)].symbol(), "u");
        assert!(row.ends_with("uvw"), "{row}");
        // Without following, the line starts at its beginning.
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        assert!(view.render_line(buf.area, &mut buf, false).is_none());
        assert_eq!(buf[(0, 0)].symbol(), "a");
        // An area outside the buffer draws nothing.
        assert!(view.render(Rect::new(0, 5, 10, 1), &mut buf).is_none());
    }

    #[test]
    fn wide_and_combining_characters() {
        let styles = Styles::new(Theme::get("light"), ColorMode::Truecolor);
        let c = chars("あい");
        assert_eq!(wrap(&c, 3).len(), 2);
        let thai = chars("กั");
        let view = TextView {
            chars: &thai,
            styles: &styles,
            whitespace: WhitespaceStyle::Space,
            caret: CaretShape::Block,
            rtl: false,
            typo: false,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        view.render(buf.area, &mut buf);
        assert_eq!(buf[(1, 0)].symbol(), "◌\u{e31}");
    }
}
