//! A picked color and its representations across formats.
//!
//! All conversions are sRGB-aware. OKLCH uses the canonical OKLab matrix
//! from Björn Ottosson's reference implementation.

#[derive(Debug, Clone, Copy)]
pub struct PickedColor {
    pub rgb: (u8, u8, u8),
}

impl PickedColor {
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        if s.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(Self { rgb: (r, g, b) })
    }

    pub fn hex(&self) -> String {
        format!(
            "#{:02X}{:02X}{:02X}",
            self.rgb.0, self.rgb.1, self.rgb.2
        )
    }

    pub fn rgb_str(&self) -> String {
        format!("rgb({}, {}, {})", self.rgb.0, self.rgb.1, self.rgb.2)
    }

    pub fn hsl_str(&self) -> String {
        let (h, s, l) = rgb_to_hsl(self.rgb.0, self.rgb.1, self.rgb.2);
        format!(
            "hsl({}, {}%, {}%)",
            h.round() as i32,
            (s * 100.0).round() as i32,
            (l * 100.0).round() as i32,
        )
    }

    pub fn oklch_str(&self) -> String {
        let (l, c, h) = rgb_to_oklch(self.rgb.0, self.rgb.1, self.rgb.2);
        // CSS Color 4 syntax: oklch(L C H) where L is 0-1 (or %), C is unbounded float, H in degrees.
        format!("oklch({:.3} {:.3} {:.1})", l, c, h)
    }
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) * 60.0
    } else if max == g {
        ((b - r) / d + 2.0) * 60.0
    } else {
        ((r - g) / d + 4.0) * 60.0
    };
    (h, s, l)
}

/// sRGB (0-255 each channel) → OKLCH (lightness 0-1, chroma ~0-0.4, hue 0-360°).
fn rgb_to_oklch(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    fn to_linear(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let r = to_linear(r as f32 / 255.0);
    let g = to_linear(g as f32 / 255.0);
    let b = to_linear(b as f32 / 255.0);

    // Linear sRGB → LMS via OKLab matrix M1
    let l_lin = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m_lin = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s_lin = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    let l_ = l_lin.cbrt();
    let m_ = m_lin.cbrt();
    let s_ = s_lin.cbrt();

    // LMS → OKLab via M2
    let lab_l = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_;
    let lab_a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_;
    let lab_b = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_;

    // OKLab → OKLCH
    let c = (lab_a * lab_a + lab_b * lab_b).sqrt();
    let h = lab_b.atan2(lab_a).to_degrees();
    let h = if h < 0.0 { h + 360.0 } else { h };

    (lab_l, c, h)
}
