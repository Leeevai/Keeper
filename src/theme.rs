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
}
