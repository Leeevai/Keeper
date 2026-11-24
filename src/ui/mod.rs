//! Shared terminal UI pieces: styles, the keyboard and the text area.

pub mod basic_colors;
pub mod keyboard;
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
    /// The 16 basic colors.
    Ansi16,
    /// No colors at all.
    Monochrome,
}

impl ColorDepth {
    /// The colors to use in the given mode, detected from the environment
    /// for `ColorMode::Auto`.
    pub fn detect(mode: ColorMode) -> ColorDepth {
        Self::detect_with(mode, |name| std::env::var(name).ok())
    }

    fn detect_with(mode: ColorMode, var: impl Fn(&str) -> Option<String>) -> ColorDepth {
        match mode {
            ColorMode::Truecolor => ColorDepth::Truecolor,
            ColorMode::Ansi256 => ColorDepth::Ansi256,
            ColorMode::Ansi16 => ColorDepth::Ansi16,
            ColorMode::Monochrome => ColorDepth::Monochrome,
            ColorMode::Auto => {
                // https://no-color.org: a chosen color mode still wins.
                if var("NO_COLOR").is_some_and(|v| !v.is_empty()) {
                    return ColorDepth::Monochrome;
                }
                let colorterm = var("COLORTERM").unwrap_or_default().to_lowercase();
                let term = var("TERM").unwrap_or_default();
                // TERM names a terminal that cannot do more, which wins over
                // a COLORTERM passed along from another terminal.
                if term == "dumb" || term.starts_with("vt") {
                    // The VT100 and VT220 show no colors.
                    ColorDepth::Monochrome
                } else if term == "linux"
                    || term == "ansi"
                    || term == "xterm-color"
                    || term.contains("16color")
                    || term.starts_with("cons")
                {
                    // The Linux console and old color terminals.
                    ColorDepth::Ansi16
                } else if colorterm.contains("truecolor")
                    || colorterm.contains("24bit")
                    || term.contains("direct")
                {
                    ColorDepth::Truecolor
                } else {
                    // xterm-256color, tmux-256color and the like, and the
                    // safest guess for any other terminal.
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
        Self::with_depth(theme, ColorDepth::detect(mode))
    }

    pub fn with_depth(theme: &Theme, depth: ColorDepth) -> Self {
        let c = |rgb: Rgb| to_color(depth, rgb);
        // Without theme colors, the highlights come from the light palette,
        // which reads well on both light and dark terminal backgrounds.
        let palette = theme
            .colors
            .clone()
            .unwrap_or_else(|| Theme::get("light").colors.clone().expect("light palette"));
        match (&theme.colors, depth) {
            (_, ColorDepth::Monochrome) => Styles {
                depth,
                palette,
                themed: false,
                base: Style::new(),
                panel: Style::new(),
                border: Style::new(),
                title: Style::new().add_modifier(Modifier::BOLD),
                tab: Style::new(),
                tab_active: Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD),
                text: Style::new(),
                hit: Style::new().add_modifier(Modifier::DIM),
                miss: Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                garbage: Style::new().add_modifier(Modifier::CROSSED_OUT | Modifier::UNDERLINED),
                special: Style::new().add_modifier(Modifier::DIM),
                name: Style::new(),
                value: Style::new().add_modifier(Modifier::BOLD),
                more: Style::new(),
                less: Style::new(),
                accent: Style::new().add_modifier(Modifier::BOLD),
                dim: Style::new().add_modifier(Modifier::DIM),
                selected: Style::new().add_modifier(Modifier::REVERSED),
                error: Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            },
            (Some(p), ColorDepth::Truecolor | ColorDepth::Ansi256) => {
                let base = Style::new().fg(c(p.text)).bg(c(p.background));
                Styles {
                    depth,
                    palette,
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
            // The terminal's own background and text, with the basic colors
            // for accents: the system theme, and any theme in 16 colors,
            // whose backgrounds would collapse into a few flat shades.
            _ => Styles {
                depth,
                palette,
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
            },
        }
    }

    /// Whether colors are off and states show as bold, underline and
    /// reverse video instead.
    pub fn monochrome(&self) -> bool {
        self.depth == ColorDepth::Monochrome
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
        match self.depth {
            ColorDepth::Monochrome => Style::new(),
            // The terminal shows its own shade of the basic color, which can
            // be much lighter or darker than `bg`: the text has to read on that.
            ColorDepth::Ansi16 => {
                let (color, shade) = BASIC_COLORS[bg.to_ansi16() as usize];
                let fg = if contrast(shade) == Rgb(20, 20, 20) {
                    Color::Black
                } else {
                    Color::White
                };
                Style::new().bg(color).fg(fg)
            }
            _ => Style::new().bg(self.color(bg)).fg(self.color(contrast(bg))),
        }
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
        if self.monochrome() {
            return match cls {
                Some(Class::Keyword) => Style::new().add_modifier(Modifier::BOLD),
                Some(Class::Comment) => Style::new().add_modifier(Modifier::DIM),
                _ => self.text,
            };
        }
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

/// The 16 basic colors, with the shades xterm gives them by default.
const BASIC_COLORS: [(Color, Rgb); 16] = [
    (Color::Black, Rgb(0, 0, 0)),
    (Color::Red, Rgb(205, 0, 0)),
    (Color::Green, Rgb(0, 205, 0)),
    (Color::Yellow, Rgb(205, 205, 0)),
    (Color::Blue, Rgb(0, 0, 238)),
    (Color::Magenta, Rgb(205, 0, 205)),
    (Color::Cyan, Rgb(0, 205, 205)),
    (Color::Gray, Rgb(229, 229, 229)),
    (Color::DarkGray, Rgb(127, 127, 127)),
    (Color::LightRed, Rgb(255, 0, 0)),
    (Color::LightGreen, Rgb(0, 255, 0)),
    (Color::LightYellow, Rgb(255, 255, 0)),
    (Color::LightBlue, Rgb(92, 92, 255)),
    (Color::LightMagenta, Rgb(255, 0, 255)),
    (Color::LightCyan, Rgb(0, 255, 255)),
    (Color::White, Rgb(255, 255, 255)),
];

fn to_color(depth: ColorDepth, rgb: Rgb) -> Color {
    match depth {
        ColorDepth::Truecolor => Color::Rgb(rgb.0, rgb.1, rgb.2),
        ColorDepth::Ansi256 => Color::Indexed(rgb.to_ansi256()),
        // The named colors, written as the escape codes that every color
        // terminal understands (30 to 37 and 90 to 97).
        ColorDepth::Ansi16 => BASIC_COLORS[rgb.to_ansi16() as usize].0,
        ColorDepth::Monochrome => Color::Reset,
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
                let full_color =
                    matches!(styles.depth, ColorDepth::Truecolor | ColorDepth::Ansi256);
                assert_eq!(styles.themed, theme.colors.is_some() && full_color);
                let _ = styles.confidence_rgb(0.5);
            }
        }
        let s = Styles::new(Theme::get("dark"), ColorMode::Ansi256);
        assert!(matches!(s.color(Rgb(255, 0, 0)), Color::Indexed(196)));
        assert_eq!(contrast(Rgb(255, 255, 255)), Rgb(20, 20, 20));
        assert_eq!(contrast(Rgb(0, 0, 0)), Rgb(245, 245, 245));
    }

    #[test]
    fn detects_the_colors_of_the_terminal() {
        let detect = |mode, vars: &[(&str, &str)]| {
            let vars: Vec<(String, String)> = vars
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            ColorDepth::detect_with(mode, |name| {
                vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
            })
        };
        use ColorDepth::*;
        let auto = ColorMode::Auto;
        assert_eq!(
            detect(
                auto,
                &[("COLORTERM", "truecolor"), ("TERM", "xterm-256color")]
            ),
            Truecolor
        );
        assert_eq!(detect(auto, &[("TERM", "xterm-direct")]), Truecolor);
        assert_eq!(detect(auto, &[("TERM", "xterm-256color")]), Ansi256);
        assert_eq!(detect(auto, &[("TERM", "tmux-256color")]), Ansi256);
        assert_eq!(detect(auto, &[("TERM", "xterm")]), Ansi256);
        assert_eq!(detect(auto, &[]), Ansi256);
        assert_eq!(detect(auto, &[("TERM", "linux")]), Ansi16);
        assert_eq!(detect(auto, &[("TERM", "vt220")]), Monochrome);
        assert_eq!(detect(auto, &[("TERM", "vt100")]), Monochrome);
        // TERM wins over a COLORTERM passed along from another terminal.
        let console = [("TERM", "linux"), ("COLORTERM", "truecolor")];
        assert_eq!(detect(auto, &console), Ansi16);
        assert_eq!(
            detect(auto, &[("TERM", "vt220"), ("COLORTERM", "24bit")]),
            Monochrome
        );
        assert_eq!(detect(auto, &[("TERM", "xterm-16color")]), Ansi16);
        assert_eq!(detect(auto, &[("TERM", "dumb")]), Monochrome);
        // NO_COLOR turns colors off unless it is empty.
        let no_color = [("NO_COLOR", "1"), ("COLORTERM", "truecolor")];
        assert_eq!(detect(auto, &no_color), Monochrome);
        assert_eq!(
            detect(auto, &[("NO_COLOR", ""), ("COLORTERM", "truecolor")]),
            Truecolor
        );
        // A color mode chosen in the settings wins over the environment.
        assert_eq!(detect(ColorMode::Ansi16, &no_color), Ansi16);
        assert_eq!(
            detect(ColorMode::Monochrome, &[("COLORTERM", "truecolor")]),
            Monochrome
        );
    }

    #[test]
    fn basic_and_no_colors() {
        let dark = Theme::get("dark");
        let s = Styles::with_depth(dark, ColorDepth::Ansi16);
        assert!(!s.themed);
        assert_eq!(s.color(Rgb::hex(0x60d788)), Color::LightGreen);
        assert_eq!(s.color(Rgb(0, 0, 0)), Color::Black);
        // The theme still picks the highlight colors.
        assert_eq!(s.palette, dark.colors.clone().unwrap());
        // Key labels read on the terminal's shade of the basic color: this
        // gray is dark enough for white text, but shows as light gray.
        let gray = Rgb(135, 135, 135);
        assert_eq!(contrast(gray), Rgb(245, 245, 245));
        assert_eq!(s.on(gray), Style::new().bg(Color::Gray).fg(Color::Black));
        assert_eq!(
            s.on(Rgb(90, 90, 90)),
            Style::new().bg(Color::DarkGray).fg(Color::White)
        );
        assert_eq!(
            s.on(Rgb::hex(0x2060c0)),
            Style::new().bg(Color::Blue).fg(Color::White)
        );
        let s = Styles::with_depth(dark, ColorDepth::Monochrome);
        assert!(s.monochrome() && !s.themed);
        assert_eq!(s.color(Rgb(255, 0, 0)), Color::Reset);
        assert_eq!(s.on(Rgb(255, 0, 0)), Style::new());
        for style in [
            s.base, s.text, s.hit, s.miss, s.garbage, s.selected, s.error,
        ] {
            assert_eq!(style.fg, None);
            assert_eq!(style.bg, None);
        }
        // Typed text, errors and the selection still look different.
        assert_ne!(s.hit, s.text);
        assert_ne!(s.miss, s.text);
        assert_ne!(s.miss, s.hit);
        assert_ne!(s.selected, s.text);
        assert_eq!(
            s.syntax(Some(Class::Keyword)),
            Style::new().add_modifier(Modifier::BOLD)
        );
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
