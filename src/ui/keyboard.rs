//! The on-screen keyboard.

use std::collections::{HashMap, HashSet};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use super::Styles;
use crate::keyboard::{Character, KeyShape, Keyboard};
use crate::theme::Rgb;

/// Terminal columns per key unit.
const UNIT: f64 = 4.0;

pub struct KeyboardView<'a> {
    pub keyboard: &'a Keyboard,
    pub styles: &'a Styles,
    /// Color the keys by finger zones.
    pub zone_colors: bool,
    /// Keys to press next.
    pub pointers: Vec<String>,
    /// A key that was just pressed, and whether it was the right one.
    pub flash: Option<(String, bool)>,
    /// Custom key colors, such as a heatmap.
    pub heat: Option<HashMap<String, Rgb>>,
    /// The characters of the lesson; keys without any of them are dimmed.
    pub active: Option<HashSet<u32>>,
}

impl<'a> KeyboardView<'a> {
    pub fn new(keyboard: &'a Keyboard, styles: &'a Styles) -> Self {
        Self {
            keyboard,
            styles,
            zone_colors: true,
            pointers: Vec::new(),
            flash: None,
            heat: None,
            active: None,
        }
    }

    /// The size of the keyboard, with or without blank lines between rows.
    pub fn size(keyboard: &Keyboard, gaps: bool) -> (u16, u16) {
        let width = keyboard
            .shapes
            .iter()
            .map(|s| ((s.x + s.w) * UNIT).round() as u16)
            .max()
            .unwrap_or(0);
        let rows = keyboard
            .shapes
            .iter()
            .map(|s| (s.y + s.h).ceil() as u16)
            .max()
            .unwrap_or(0);
        let height = if gaps {
            (rows * 2).saturating_sub(1)
        } else {
            rows
        };
        (width.saturating_sub(1), height)
    }

    /// The keys to highlight to type a character: the key itself, Shift
    /// on the other hand, AltGr, and a dead key.
    pub fn pointers_for(keyboard: &Keyboard, cp: u32) -> Vec<String> {
        let Some(combo) = keyboard.combo(cp) else {
            return Vec::new();
        };
        let mut keys = vec![combo.key.clone()];
        if combo.modifier.shift() {
            let hand = keyboard.shape(&combo.key).and_then(KeyShape::hand);
            keys.push(if hand == Some("left") {
                "ShiftRight".into()
            } else {
                "ShiftLeft".into()
            });
        }
        if combo.modifier.alt() {
            keys.push("AltRight".into());
        }
        if let Some(prefix) = &combo.prefix {
            keys.push(prefix.key.clone());
        }
        keys
    }

    fn label(&self, shape: &KeyShape, width: usize) -> String {
        let label = if !shape.labels.is_empty() {
            match shape.labels.join(" ").as_str() {
                "Backspace" | "Back space" => "⌫".to_string(),
                "Tab" => "⇥".into(),
                "Caps Lock" => "⇪".into(),
                "Enter" => "⏎".into(),
                "Shift" => "⇧".into(),
                other => other.to_string(),
            }
        } else {
            match self.keyboard.characters(&shape.id).first() {
                Some(Some(Character::CodePoint(cp))) if *cp == 0x20 => String::new(),
                Some(Some(ch @ Character::CodePoint(_))) => {
                    let s = ch.label();
                    let upper = s.to_uppercase();
                    if upper.chars().count() == 1 { upper } else { s }
                }
                Some(Some(ch)) => ch.label(),
                _ => String::new(),
            }
        };
        truncate(&label, width)
    }

    fn background(&self, shape: &KeyShape) -> Rgb {
        let p = &self.styles.palette;
        if let Some((id, ok)) = &self.flash
            && *id == shape.id
        {
            return if *ok { p.fast_key } else { p.error };
        }
        if self.pointers.contains(&shape.id) {
            return p.pointer;
        }
        if let Some(heat) = &self.heat {
            let none = if self.styles.themed {
                p.panel.mix(p.border, 50.0)
            } else {
                Rgb(90, 90, 90)
            };
            return heat.get(&shape.id).copied().unwrap_or(none);
        }
        let neutral = if self.styles.themed {
            p.border
        } else {
            Rgb(150, 150, 150)
        };
        let color = if self.zone_colors {
            shape
                .finger()
                .and_then(|f| self.styles.zone_rgb(f))
                .unwrap_or(neutral)
        } else {
            neutral
        };
        if self.is_dimmed(shape) {
            let background = if self.styles.themed {
                p.background
            } else {
                Rgb(128, 128, 128)
            };
            color.mix(background, 35.0)
        } else {
            color
        }
    }

    fn is_dimmed(&self, shape: &KeyShape) -> bool {
        match &self.active {
            Some(active) if shape.labels.is_empty() => !self
                .keyboard
                .characters(&shape.id)
                .iter()
                .flatten()
                .any(|c| match c {
                    Character::CodePoint(cp) => active.contains(cp),
                    _ => false,
                }),
            _ => false,
        }
    }
}

fn truncate(s: &str, width: usize) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if (out.clone() + &ch.to_string()).width() > width {
            break;
        }
        out.push(ch);
    }
    out
}

impl Widget for KeyboardView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (_, tall) = Self::size(self.keyboard, true);
        let gaps = area.height >= tall;
        let (width, height) = Self::size(self.keyboard, gaps);
        if width == 0 || area.width < width || area.height < height {
            return;
        }
        let origin = super::centered(area, width, height);
        let row = |y: f64| -> u16 {
            if gaps {
                (y * 2.0).round() as u16
            } else {
                y.round() as u16
            }
        };
        for shape in &self.keyboard.shapes {
            let x0 = (shape.x * UNIT).round() as u16;
            let x1 = ((shape.x + shape.w) * UNIT).round() as u16 - 1;
            let y0 = row(shape.y);
            let y1 = row(shape.y + shape.h)
                .saturating_sub(if gaps { 2 } else { 1 })
                .max(y0);
            let bg = self.background(shape);
            let style = self.styles.on(bg);
            for y in y0..=y1 {
                for x in x0..x1 {
                    if let Some(cell) = buf.cell_mut((origin.x + x, origin.y + y)) {
                        cell.set_symbol(" ").set_style(style);
                    }
                }
            }
            let key_width = (x1 - x0) as usize;
            let label = self.label(shape, key_width);
            let label_width = label.width() as u16;
            let lx = origin.x + x0 + (x1 - x0).saturating_sub(label_width) / 2;
            let mut label_style = style;
            if shape.homing {
                label_style = label_style.add_modifier(Modifier::UNDERLINED);
            }
            if self.pointers.contains(&shape.id) {
                label_style = label_style.add_modifier(Modifier::BOLD);
            }
            buf.set_string(lx, origin.y + y0, &label, label_style);
        }
    }
}

/// The style of a key label in the list of lesson keys.
pub fn key_style(styles: &Styles, included: bool, confidence: Option<f64>) -> Style {
    match (included, confidence) {
        (true, Some(c)) => styles.on(styles.confidence_rgb(c)),
        (true, None) => styles.text,
        (false, _) => styles.dim,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::Layout;
    use crate::settings::ColorMode;
    use crate::theme::Theme;

    fn render(keyboard: &Keyboard, view: KeyboardView, width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        view.render(buf.area, &mut buf);
        let _ = keyboard;
        buf
    }

    fn text(buf: &Buffer) -> String {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_the_us_keyboard() {
        let keyboard = Keyboard::new(Layout::get("en-us"), "ansi101", "standard");
        let styles = Styles::new(Theme::get("light"), ColorMode::Truecolor);
        assert_eq!(KeyboardView::size(&keyboard, true), (59, 9));
        assert_eq!(KeyboardView::size(&keyboard, false), (59, 5));
        let buf = render(&keyboard, KeyboardView::new(&keyboard, &styles), 60, 9);
        let t = text(&buf);
        assert!(t.contains(" Q   W   E   R   T   Y "), "{t}");
        assert!(t.contains(" A   S   D   F "), "{t}");
        assert!(t.contains('⇧') && t.contains('⌫'), "{t}");
        // Without room for gaps the rows are packed.
        let packed = text(&render(
            &keyboard,
            KeyboardView::new(&keyboard, &styles),
            60,
            5,
        ));
        assert_eq!(packed.lines().count(), 5);
        assert!(packed.lines().nth(2).unwrap().contains(" A "));
        // Too small: nothing is drawn.
        assert!(
            text(&render(
                &keyboard,
                KeyboardView::new(&keyboard, &styles),
                30,
                9
            ))
            .trim()
            .is_empty()
        );
    }

    #[test]
    fn highlights_keys_to_press() {
        let keyboard = Keyboard::new(Layout::get("en-us"), "ansi101", "standard");
        assert_eq!(
            KeyboardView::pointers_for(&keyboard, u32::from('a')),
            vec!["KeyA"]
        );
        assert_eq!(
            KeyboardView::pointers_for(&keyboard, u32::from('A')),
            vec!["KeyA", "ShiftRight"]
        );
        assert_eq!(
            KeyboardView::pointers_for(&keyboard, u32::from('P')),
            vec!["KeyP", "ShiftLeft"]
        );
        assert!(KeyboardView::pointers_for(&keyboard, u32::from('é')).is_empty());
        let styles = Styles::new(Theme::get("light"), ColorMode::Truecolor);
        let mut view = KeyboardView::new(&keyboard, &styles);
        view.pointers = vec!["KeyA".into()];
        let pointer = styles.color(styles.palette.pointer);
        let buf = render(&keyboard, view, 60, 9);
        let a = (0..60).find(|&x| buf[(x, 4)].symbol() == "A").unwrap();
        assert_eq!(buf[(a, 4)].bg, pointer);
    }

    #[test]
    fn renders_every_geometry() {
        let styles = Styles::new(Theme::get("dark"), ColorMode::Ansi256);
        for layout in Layout::all() {
            for geometry in &layout.geometries {
                let keyboard = Keyboard::new(layout, geometry, "standard");
                let (w, h) = KeyboardView::size(&keyboard, true);
                let buf = render(&keyboard, KeyboardView::new(&keyboard, &styles), w, h);
                assert!(!text(&buf).trim().is_empty(), "{} {}", layout.id, geometry);
            }
        }
    }
}
