//! Cut-down picked-color type. Just enough to render a swatch and produce
//! the hex on click. The full multi-format conversions live in the GUI.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}
