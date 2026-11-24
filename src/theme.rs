//! Color themes, computed from the palettes of keybr.com.

/// An RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(v: u32) -> Rgb {
        Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    fn to_hsl(self) -> (f64, f64, f64) {
        let r = self.0 as f64 / 255.0;
        let g = self.1 as f64 / 255.0;
        let b = self.2 as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if max == min {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h / 6.0, s, l)
    }

    fn from_hsl(h: f64, s: f64, l: f64) -> Rgb {
        fn hue(p: f64, q: f64, mut t: f64) -> f64 {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        }
        let (r, g, b) = if s == 0.0 {
            (l, l, l)
        } else {
            let q = if l < 0.5 {
                l * (1.0 + s)
            } else {
                l + s - l * s
            };
            let p = 2.0 * l - q;
            (
                hue(p, q, h + 1.0 / 3.0),
                hue(p, q, h),
                hue(p, q, h - 1.0 / 3.0),
            )
        };
        let c = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
        Rgb(c(r), c(g), c(b))
    }

    /// Increases the HSL lightness by `amount` percent, like LESS `lighten`.
    pub fn lighten(self, amount: f64) -> Rgb {
        let (h, s, l) = self.to_hsl();
        Rgb::from_hsl(h, s, (l + amount / 100.0).clamp(0.0, 1.0))
    }

    /// Decreases the HSL lightness by `amount` percent, like LESS `darken`.
    pub fn darken(self, amount: f64) -> Rgb {
        self.lighten(-amount)
    }

    /// Mixes `weight` percent of `self` with the rest of `other`, like LESS `mix`.
    pub fn mix(self, other: Rgb, weight: f64) -> Rgb {
        let w = weight / 100.0;
        let m = |a: u8, b: u8| (a as f64 * w + b as f64 * (1.0 - w)).round() as u8;
        Rgb(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }

    /// Linear interpolation from `self` at 0 to `other` at 1.
    pub fn lerp(self, other: Rgb, t: f64) -> Rgb {
        self.mix(other, (1.0 - t.clamp(0.0, 1.0)) * 100.0)
    }

    /// The nearest color of the xterm 256 color palette.
    pub fn to_ansi256(self) -> u8 {
        let level = |v: u8| -> u8 {
            if v < 48 {
                0
            } else if v < 115 {
                1
            } else {
                ((v as u16 - 35) / 40) as u8
            }
        };
        let (r, g, b) = (level(self.0), level(self.1), level(self.2));
        let steps = [0u8, 95, 135, 175, 215, 255];
        let cube = Rgb(steps[r as usize], steps[g as usize], steps[b as usize]);
        let avg = (self.0 as u16 + self.1 as u16 + self.2 as u16) / 3;
        let gray_index = if avg > 238 {
            23
        } else {
            avg.saturating_sub(3) / 10
        } as u8;
        let gray = 8 + 10 * gray_index;
        let dist = |c: Rgb| {
            let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
            d(c.0, self.0) + d(c.1, self.1) + d(c.2, self.2)
        };
        if dist(Rgb(gray, gray, gray)) < dist(cube) {
            232 + gray_index
        } else {
            16 + 36 * r + 6 * g + b
        }
    }

    /// The closest of the 16 basic terminal colors, 0 to 15. Terminals
    /// choose their own shades for these, so the match goes by hue and
    /// lightness instead of by distance to some particular palette: a muted
    /// green stays green instead of turning gray.
    pub fn to_ansi16(self) -> u8 {
        let max = self.0.max(self.1).max(self.2);
        let min = self.0.min(self.1).min(self.2);
        let (h, _, l) = self.to_hsl();
        if max - min < 40 {
            return match l {
                l if l < 0.2 => 0,  // black
                l if l < 0.5 => 8,  // dark gray
                l if l < 0.85 => 7, // gray
                _ => 15,            // white
            };
        }
        // Red, yellow, green, cyan, blue and magenta, every 60 degrees.
        let base = match ((h * 6.0 + 0.5) as u8) % 6 {
            0 => 1,
            1 => 3,
            2 => 2,
            3 => 6,
            4 => 4,
            _ => 5,
        };
        if l > 0.6 { base + 8 } else { base }
    }
}

/// The colors of a theme. The `system` theme has no colors and uses the
/// terminal palette instead.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub colors: Option<Palette>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub background: Rgb,
    pub panel: Rgb,
    pub border: Rgb,
    pub text: Rgb,
    pub accent: Rgb,
    pub error: Rgb,
    /// Characters typed correctly.
    pub hit: Rgb,
    /// Whitespace symbols.
    pub special: Rgb,
    pub name: Rgb,
    pub value: Rgb,
    pub more: Rgb,
    pub less: Rgb,
    pub slow_key: Rgb,
    pub fast_key: Rgb,
    pub pointer: Rgb,
    pub zones: [Rgb; 6],
    pub chart_speed: Rgb,
    pub chart_accuracy: Rgb,
    pub chart_complexity: Rgb,
    pub chart_threshold: Rgb,
    pub chart_hits: Rgb,
    pub chart_misses: Rgb,
    pub chart_ratio: Rgb,
    pub syntax_keyword: Rgb,
    pub syntax_string: Rgb,
    pub syntax_number: Rgb,
    pub syntax_comment: Rgb,
}

struct Base {
    primary: u32,
    secondary: u32,
    accent: u32,
    error: u32,
    slow: u32,
    fast: u32,
    /// How much of the primary color is mixed into the chart and zone colors.
    mix: f64,
    dark: bool,
}

impl Base {
    fn palette(&self) -> Palette {
        let primary = Rgb::hex(self.primary);
        let secondary = Rgb::hex(self.secondary);
        let accent = Rgb::hex(self.accent);
        let mixed = |c: u32| primary.mix(Rgb::hex(c), self.mix);
        let panel = if self.dark {
            primary.lighten(5.0)
        } else {
            primary.darken(5.0)
        };
        Palette {
            background: primary,
            panel,
            border: primary.mix(secondary, 70.0),
            text: secondary,
            accent,
            error: Rgb::hex(self.error),
            hit: primary.mix(secondary, 40.0),
            special: primary.mix(secondary, 50.0),
            name: if self.dark {
                secondary.darken(20.0)
            } else {
                secondary.lighten(20.0)
            },
            value: if self.dark {
                secondary.lighten(10.0)
            } else {
                secondary.darken(10.0)
            },
            more: if self.dark {
                Rgb::hex(0x5fb356)
            } else {
                Rgb::hex(0x2a7e21)
            },
            less: if self.dark {
                Rgb::hex(0xd0646d)
            } else {
                Rgb::hex(0xa1464e)
            },
            slow_key: Rgb::hex(self.slow),
            fast_key: Rgb::hex(self.fast),
            pointer: Rgb::hex(0x4ba0f2),
            zones: [
                mixed(0x8ec07c),
                mixed(0xb8bb26),
                mixed(0xfabd2f),
                mixed(0x83a698),
                mixed(0xd3869b),
                mixed(0xd66354),
            ],
            chart_speed: mixed(0x6fb48c),
            chart_accuracy: mixed(0xef522f),
            chart_complexity: mixed(0xac71d0),
            chart_threshold: mixed(0xd2649a),
            chart_hits: mixed(0x5f6cb4),
            chart_misses: mixed(0xb43f3e),
            chart_ratio: mixed(0xb140b4),
            syntax_keyword: Rgb::hex(if self.dark { 0x5991cd } else { 0x56a1f4 }),
            syntax_string: Rgb::hex(0x72b172),
            syntax_number: Rgb::hex(if self.dark { 0xb281d3 } else { 0x763a9e }),
            syntax_comment: Rgb::hex(0x9f8484),
        }
    }
}

/// A dark editor palette: a background, a text color, a muted color for
/// comments and eight accents, which also color the keys, charts and code.
struct Scheme {
    bg: u32,
    fg: u32,
    muted: u32,
    red: u32,
    orange: u32,
    yellow: u32,
    green: u32,
    cyan: u32,
    blue: u32,
    purple: u32,
    /// The color of titles, tabs and the caret: the palette's signature.
    accent: u32,
}

impl Scheme {
    fn palette(&self) -> Palette {
        let bg = Rgb::hex(self.bg);
        let fg = Rgb::hex(self.fg);
        let c = Rgb::hex;
        // Key and chart colors are softened a little toward the background.
        let soft = |v: u32| bg.mix(c(v), 25.0);
        Palette {
            background: bg,
            panel: bg.lighten(5.0),
            border: bg.mix(fg, 70.0),
            text: fg,
            accent: c(self.accent),
            error: c(self.red),
            hit: bg.mix(fg, 40.0),
            special: bg.mix(fg, 55.0),
            name: c(self.muted).lighten(15.0),
            value: fg.lighten(10.0),
            more: c(self.green),
            less: c(self.red),
            slow_key: soft(self.red),
            fast_key: soft(self.green),
            pointer: c(self.blue),
            zones: [
                soft(self.cyan),
                soft(self.green),
                soft(self.yellow),
                soft(self.blue),
                soft(self.purple),
                soft(self.orange),
            ],
            chart_speed: c(self.green),
            chart_accuracy: c(self.orange),
            chart_complexity: c(self.purple),
            chart_threshold: c(self.red),
            chart_hits: c(self.blue),
            chart_misses: c(self.red),
            chart_ratio: c(self.cyan),
            syntax_keyword: c(self.purple),
            syntax_string: c(self.green),
            syntax_number: c(self.orange),
            syntax_comment: c(self.muted),
        }
    }
}

impl Theme {
    pub fn all() -> &'static [Theme] {
        static ALL: std::sync::OnceLock<Vec<Theme>> = std::sync::OnceLock::new();
        ALL.get_or_init(|| {
            let theme = |id, name, base: Option<Base>| Theme { id, name, colors: base.map(|b| b.palette()) };
            #[rustfmt::skip]
            let themes = vec![
                theme("system", "System", None),
                theme("light", "Light", Some(Base { primary: 0xf4f0f0, secondary: 0x282640, accent: 0x3d475c, error: 0xff3333, slow: 0xcc0000, fast: 0x60d788, mix: 0.0, dark: false })),
                theme("dark", "Dark", Some(Base { primary: 0x333333, secondary: 0x9f9999, accent: 0x6c6666, error: 0x9b4545, slow: 0x8c1818, fast: 0x448154, mix: 50.0, dark: true })),
                theme("gray", "Gray", Some(Base { primary: 0xb3b3b3, secondary: 0x262626, accent: 0x4d4d4d, error: 0xa34729, slow: 0xcf5f5f, fast: 0x6fa882, mix: 20.0, dark: false })),
                theme("yellow", "Yellow", Some(Base { primary: 0xffe666, secondary: 0x26220d, accent: 0x736626, error: 0xb83d14, slow: 0xcc8b00, fast: 0x96d760, mix: 30.0, dark: false })),
                theme("garden", "Garden", Some(Base { primary: 0xcee0a4, secondary: 0x334532, accent: 0x45633f, error: 0xe49874, slow: 0xcc0000, fast: 0x60d788, mix: 0.0, dark: false })),
                theme("coffee", "Coffee", Some(Base { primary: 0xfaecd7, secondary: 0x262626, accent: 0x4d4d4d, error: 0xa34729, slow: 0xcf5f5f, fast: 0x6fa882, mix: 20.0, dark: false })),
                theme("chocolate", "Chocolate", Some(Base { primary: 0x604a43, secondary: 0xcfcfcf, accent: 0xe3a390, error: 0xa34729, slow: 0xcf5f5f, fast: 0x6fa882, mix: 20.0, dark: true })),
                theme("honey", "Honey", Some(Base { primary: 0xffbf00, secondary: 0x402609, accent: 0x754c22, error: 0xb83d14, slow: 0xcc8b00, fast: 0x8cc64e, mix: 30.0, dark: false })),
            ];
            let scheme = |id, name, s: Scheme| Theme { id, name, colors: Some(s.palette()) };
            #[rustfmt::skip]
            let schemes = [
                scheme("dracula", "Dracula", Scheme { bg: 0x282a36, fg: 0xf8f8f2, muted: 0x6272a4, red: 0xff5555, orange: 0xffb86c, yellow: 0xf1fa8c, green: 0x50fa7b, cyan: 0x8be9fd, blue: 0xff79c6, purple: 0xbd93f9, accent: 0xbd93f9 }),
                scheme("nord", "Nord", Scheme { bg: 0x2e3440, fg: 0xd8dee9, muted: 0x7b88a1, red: 0xbf616a, orange: 0xd08770, yellow: 0xebcb8b, green: 0xa3be8c, cyan: 0x8fbcbb, blue: 0x81a1c1, purple: 0xb48ead, accent: 0x88c0d0 }),
                scheme("gruvbox", "Gruvbox", Scheme { bg: 0x282828, fg: 0xebdbb2, muted: 0x928374, red: 0xfb4934, orange: 0xfe8019, yellow: 0xfabd2f, green: 0xb8bb26, cyan: 0x8ec07c, blue: 0x83a598, purple: 0xd3869b, accent: 0xfabd2f }),
                scheme("solarized", "Solarized", Scheme { bg: 0x002b36, fg: 0x93a1a1, muted: 0x657b83, red: 0xdc322f, orange: 0xcb4b16, yellow: 0xb58900, green: 0x859900, cyan: 0x2aa198, blue: 0x268bd2, purple: 0x6c71c4, accent: 0x268bd2 }),
                scheme("catppuccin", "Catppuccin", Scheme { bg: 0x1e1e2e, fg: 0xcdd6f4, muted: 0x7f849c, red: 0xf38ba8, orange: 0xfab387, yellow: 0xf9e2af, green: 0xa6e3a1, cyan: 0x94e2d5, blue: 0x89b4fa, purple: 0xcba6f7, accent: 0xcba6f7 }),
                scheme("tokyo-night", "Tokyo Night", Scheme { bg: 0x1a1b26, fg: 0xc0caf5, muted: 0x565f89, red: 0xf7768e, orange: 0xff9e64, yellow: 0xe0af68, green: 0x9ece6a, cyan: 0x7dcfff, blue: 0x7aa2f7, purple: 0xbb9af7, accent: 0x7aa2f7 }),
            ];
            let mut themes = themes;
            themes.extend(schemes);
            themes
        })
    }

    pub fn find(id: &str) -> Option<&'static Theme> {
        Self::all().iter().find(|t| t.id == id)
    }

    pub fn get(id: &str) -> &'static Theme {
        Self::find(id).unwrap_or(&Self::all()[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_math_matches_less() {
        // Expected values computed independently with Python's colorsys.
        assert_eq!(Rgb::hex(0xf4f0f0).darken(5.0), Rgb::hex(0xe9e1e1));
        assert_eq!(Rgb::hex(0x282640).lighten(20.0), Rgb::hex(0x504c80));
        assert_eq!(
            Rgb::hex(0xf4f0f0).mix(Rgb::hex(0x282640), 40.0),
            Rgb::hex(0x7a7786)
        );
        assert_eq!(
            Rgb::hex(0x000000).lerp(Rgb::hex(0xffffff), 0.5),
            Rgb(128, 128, 128)
        );
    }

    #[test]
    fn ansi256_approximation() {
        assert_eq!(Rgb(0, 0, 0).to_ansi256(), 16);
        assert_eq!(Rgb(255, 255, 255).to_ansi256(), 231);
        assert_eq!(Rgb(255, 0, 0).to_ansi256(), 196);
        assert_eq!(Rgb(128, 128, 128).to_ansi256(), 244);
    }

    #[test]
    fn basic_terminal_colors() {
        assert_eq!(Rgb(0, 0, 0).to_ansi16(), 0);
        assert_eq!(Rgb(90, 90, 90).to_ansi16(), 8);
        assert_eq!(Rgb(160, 160, 160).to_ansi16(), 7);
        assert_eq!(Rgb(250, 250, 250).to_ansi16(), 15);
        assert_eq!(Rgb::hex(0xcc0000).to_ansi16(), 1);
        assert_eq!(Rgb::hex(0xff6666).to_ansi16(), 9);
        // The fast key green of the light theme, a muted green.
        assert_eq!(Rgb::hex(0x60d788).to_ansi16(), 10);
        assert_eq!(Rgb::hex(0x448154).to_ansi16(), 2);
        assert_eq!(Rgb::hex(0xcc8b00).to_ansi16(), 3);
        assert_eq!(Rgb::hex(0x2060c0).to_ansi16(), 4);
        assert_eq!(Rgb::hex(0x20b0b0).to_ansi16(), 6);
        assert_eq!(Rgb::hex(0xa040a0).to_ansi16(), 5);
        for v in 0..=255u8 {
            assert!(Rgb(v, 255 - v, v / 2).to_ansi16() < 16);
        }
    }

    #[test]
    fn themes() {
        assert_eq!(Theme::all().len(), 15);
        assert!(Theme::get("system").colors.is_none());
        assert!(Theme::get("dark").colors.is_some());
        assert_eq!(Theme::get("nope").id, "system");
        let ids: Vec<&str> = Theme::all().iter().map(|t| t.id).collect();
        let unique: std::collections::HashSet<&&str> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "{ids:?}");
        for id in [
            "dracula",
            "nord",
            "gruvbox",
            "solarized",
            "catppuccin",
            "tokyo-night",
        ] {
            assert!(Theme::find(id).is_some(), "{id}");
        }
    }

    /// The WCAG contrast ratio of two colors, from 1 to 21.
    fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
        let luminance = |c: Rgb| {
            let channel = |v: u8| {
                let v = v as f64 / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(c.0) + 0.7152 * channel(c.1) + 0.0722 * channel(c.2)
        };
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn dark_palettes_are_readable() {
        for id in [
            "dracula",
            "nord",
            "gruvbox",
            "solarized",
            "catppuccin",
            "tokyo-night",
        ] {
            let p = Theme::get(id).colors.clone().unwrap();
            let bg = p.background;
            // Text to type, typed text, errors and code all stand out.
            assert!(contrast_ratio(p.text, bg) >= 4.5, "{id} text");
            assert!(contrast_ratio(p.hit, bg) >= 2.5, "{id} typed text");
            assert!(contrast_ratio(p.error, bg) >= 3.0, "{id} errors");
            assert!(contrast_ratio(p.accent, bg) >= 3.0, "{id} accent");
            for (name, c) in [
                ("keyword", p.syntax_keyword),
                ("string", p.syntax_string),
                ("number", p.syntax_number),
                ("comment", p.syntax_comment),
            ] {
                assert!(contrast_ratio(c, bg) >= 2.5, "{id} {name}");
            }
            // Typed text is dimmer than the text still to type.
            assert!(
                contrast_ratio(p.hit, bg) < contrast_ratio(p.text, bg),
                "{id}"
            );
            // Every finger zone has its own color.
            let mut zones: Vec<(u8, u8, u8)> = p.zones.iter().map(|c| (c.0, c.1, c.2)).collect();
            zones.sort();
            zones.dedup();
            assert_eq!(zones.len(), 6, "{id}");
        }
    }
}
