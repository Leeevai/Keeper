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
}
