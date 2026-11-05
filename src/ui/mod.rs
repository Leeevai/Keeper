//! Shared terminal UI pieces: styles, the keyboard and the text area.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::settings::ColorMode;
use crate::textinput::Class;
use crate::theme::{Palette, Rgb, Theme};

/// The colors the terminal can display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    Truecolor,
    Ansi256,
}

impl ColorDepth {
    /// Detects 24-bit color support from the environment.
    pub fn detect(mode: ColorMode) -> ColorDepth {
        match mode {
            ColorMode::Truecolor => ColorDepth::Truecolor,
            ColorMode::Ansi256 => ColorDepth::Ansi256,
            ColorMode::Auto => {
                let colorterm = std::env::var("COLORTERM")
                    .unwrap_or_default()
                    .to_lowercase();
                let term = std::env::var("TERM").unwrap_or_default();
                if colorterm.contains("truecolor")
                    || colorterm.contains("24bit")
                    || term.contains("direct")
                {
                    ColorDepth::Truecolor
                } else {
                    ColorDepth::Ansi256
                }
            }
        }
    }
}
