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

impl Styles {
    pub fn new(theme: &Theme, mode: ColorMode) -> Self {
        let depth = ColorDepth::detect(mode);
        let c = |rgb: Rgb| to_color(depth, rgb);
        match &theme.colors {
            Some(p) => {
                let base = Style::new().fg(c(p.text)).bg(c(p.background));
                Styles {
                    depth,
                    palette: p.clone(),
                    themed: true,
                    base,
                    panel: Style::new().fg(c(p.text)).bg(c(p.panel)),
                    border: Style::new().fg(c(p.border)),
                    title: Style::new().fg(c(p.accent)).add_modifier(Modifier::BOLD),
                    tab: Style::new().fg(c(p.name)),
                    tab_active: Style::new()
                        .fg(c(p.background))
                        .bg(c(p.accent))
                        .add_modifier(Modifier::BOLD),
                    text: Style::new().fg(c(p.text)),
                    hit: Style::new().fg(c(p.hit)),
                    miss: Style::new().fg(c(p.error)),
                    garbage: Style::new()
                        .fg(c(p.error))
                        .add_modifier(Modifier::CROSSED_OUT),
                    special: Style::new().fg(c(p.special)),
                    name: Style::new().fg(c(p.name)),
                    value: Style::new().fg(c(p.value)).add_modifier(Modifier::BOLD),
                    more: Style::new().fg(c(p.more)),
                    less: Style::new().fg(c(p.less)),
                    accent: Style::new().fg(c(p.accent)),
                    dim: Style::new().fg(c(p.special)),
                    selected: Style::new().bg(c(p.panel)).add_modifier(Modifier::BOLD),
                    error: Style::new().fg(c(p.error)).add_modifier(Modifier::BOLD),
                }
            }
            None => {
                // The terminal's own colors, with accents that read well on
                // both light and dark backgrounds.
                let p = Theme::get("light").colors.clone().expect("light palette");
                Styles {
                    depth,
                    palette: p.clone(),
                    themed: false,
                    base: Style::new(),
                    panel: Style::new(),
                    border: Style::new().fg(Color::DarkGray),
                    title: Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    tab: Style::new().fg(Color::Gray),
                    tab_active: Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD),
                    text: Style::new(),
                    hit: Style::new().fg(Color::DarkGray),
                    miss: Style::new().fg(Color::Red),
                    garbage: Style::new()
                        .fg(Color::Red)
                        .add_modifier(Modifier::CROSSED_OUT),
                    special: Style::new().fg(Color::DarkGray),
                    name: Style::new().fg(Color::Gray),
                    value: Style::new().add_modifier(Modifier::BOLD),
                    more: Style::new().fg(Color::Green),
                    less: Style::new().fg(Color::Red),
                    accent: Style::new().fg(Color::Cyan),
                    dim: Style::new().fg(Color::DarkGray),
                    selected: Style::new().add_modifier(Modifier::REVERSED),
                    error: Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
                }
            }
        }
    }

    pub fn color(&self, rgb: Rgb) -> Color {
        to_color(self.depth, rgb)
    }
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
