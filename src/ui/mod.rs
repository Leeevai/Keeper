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

/// The styles of every UI element, resolved from a theme.
#[derive(Debug, Clone)]
pub struct Styles {
    pub depth: ColorDepth,
    pub palette: Palette,
    /// Whether the theme sets the background, or the terminal colors are used.
    pub themed: bool,
    pub base: Style,
    pub panel: Style,
    pub border: Style,
    pub title: Style,
    pub tab: Style,
    pub tab_active: Style,
    pub text: Style,
    pub hit: Style,
    pub miss: Style,
    pub garbage: Style,
    pub special: Style,
    pub name: Style,
    pub value: Style,
    pub more: Style,
    pub less: Style,
    pub accent: Style,
    pub dim: Style,
    pub selected: Style,
    pub error: Style,
}

fn to_color(depth: ColorDepth, rgb: Rgb) -> Color {
    match depth {
        ColorDepth::Truecolor => Color::Rgb(rgb.0, rgb.1, rgb.2),
        ColorDepth::Ansi256 => Color::Indexed(rgb.to_ansi256()),
    }
}

/// Black or white, whichever reads better on the background.
pub fn contrast(bg: Rgb) -> Rgb {
    let luminance = 0.299 * bg.0 as f64 + 0.587 * bg.1 as f64 + 0.114 * bg.2 as f64;
    if luminance > 140.0 {
        Rgb(20, 20, 20)
    } else {
        Rgb(245, 245, 245)
    }
}
