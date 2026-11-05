//! Shared terminal UI pieces: styles, the keyboard and the text area.

pub mod text;

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

    /// The color of a key with the given confidence, from slow to fast.
    pub fn confidence_rgb(&self, confidence: f64) -> Rgb {
        self.palette
            .slow_key
            .lerp(self.palette.fast_key, confidence.clamp(0.0, 1.0))
    }

    /// A style with the given background and a readable foreground.
    pub fn on(&self, bg: Rgb) -> Style {
        Style::new().bg(self.color(bg)).fg(self.color(contrast(bg)))
    }

    /// The color of a finger zone.
    pub fn zone_rgb(&self, finger: &str) -> Option<Rgb> {
        let index = match finger {
            "pinky" => 0,
            "ring" => 1,
            "middle" => 2,
            "leftIndex" => 3,
            "rightIndex" => 4,
            "thumb" => 5,
            _ => return None,
        };
        Some(self.palette.zones[index])
    }

    pub fn syntax(&self, cls: Option<Class>) -> Style {
        let p = &self.palette;
        match cls {
            Some(Class::Keyword) => Style::new().fg(self.color(p.syntax_keyword)),
            Some(Class::String) => Style::new().fg(self.color(p.syntax_string)),
            Some(Class::Number) => Style::new().fg(self.color(p.syntax_number)),
            Some(Class::Comment) => Style::new().fg(self.color(p.syntax_comment)),
            _ => self.text,
        }
    }

    /// A value with its change: green when it grew, red when it fell.
    pub fn delta(&self, value: f64) -> Style {
        if value > 0.0 {
            self.more
        } else if value < 0.0 {
            self.less
        } else {
            self.name
        }
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

/// A `name: value` pair.
pub fn name_value<'a>(styles: &Styles, name: &str, value: String) -> Vec<Span<'a>> {
    vec![
        Span::styled(format!("{name}: "), styles.name),
        Span::styled(value, styles.value),
    ]
}

/// Formats a signed number with an explicit plus sign.
pub fn signed(value: f64, text: String) -> String {
    if value > 0.0 {
        format!("+{text}")
    } else {
        text
    }
}

/// Formats a whole number with thousands separators, e.g. `-12,345`.
pub fn thousands(value: f64) -> String {
    let n = value.round() as i64;
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Formats a duration in milliseconds, e.g. `1h 05m` or `3m 20s`.
pub fn format_duration(ms: u64) -> String {
    let s = ms / 1000;
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}

/// A rectangle of at most the given size, centered in the area.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

/// A horizontal bar of the given width filled to `value` (0 to 1).
pub fn bar(value: f64, width: usize) -> (String, String) {
    let filled = ((value.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    ("█".repeat(filled), "░".repeat(width - filled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting() {
        assert_eq!(format_duration(5_000), "5s");
        assert_eq!(format_duration(200_000), "3m 20s");
        assert_eq!(format_duration(3_900_000), "1h 05m");
        assert_eq!(signed(1.0, "1".into()), "+1");
        assert_eq!(signed(-1.0, "-1".into()), "-1");
        assert_eq!(thousands(15999.4), "15,999");
        assert_eq!(thousands(-1234567.0), "-1,234,567");
        assert_eq!(thousands(12.0), "12");
        assert_eq!(bar(0.5, 4), ("██".into(), "░░".into()));
        assert_eq!(bar(2.0, 2), ("██".into(), "".into()));
    }

    #[test]
    fn styles_for_every_theme() {
        for theme in Theme::all() {
            for mode in ColorMode::ALL {
                let styles = Styles::new(theme, mode);
                assert_eq!(styles.themed, theme.colors.is_some());
                let _ = styles.confidence_rgb(0.5);
            }
        }
        let s = Styles::new(Theme::get("dark"), ColorMode::Ansi256);
        assert!(matches!(s.color(Rgb(255, 0, 0)), Color::Indexed(196)));
        assert_eq!(contrast(Rgb(255, 255, 255)), Rgb(20, 20, 20));
        assert_eq!(contrast(Rgb(0, 0, 0)), Rgb(245, 245, 245));
    }

    #[test]
    fn centering() {
        let r = centered(Rect::new(0, 0, 100, 40), 60, 10);
        assert_eq!(r, Rect::new(20, 15, 60, 10));
        assert_eq!(
            centered(Rect::new(0, 0, 10, 5), 60, 10),
            Rect::new(0, 0, 10, 5)
        );
    }
}
