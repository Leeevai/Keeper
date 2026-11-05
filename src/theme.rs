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
}
