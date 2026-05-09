//! Persistent configuration stored via cosmic-config.

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    /// Recent picks as hex strings (`#RRGGBB`), newest first. Capped at the
    /// limit defined in `app.rs`. Stored as strings rather than packed ints
    /// so the on-disk config file stays human-readable and editable.
    pub history: Vec<String>,
    /// Per-format toggles for the result view. Defaults match PowerToys
    /// (HEX/RGB/HSL/HSV on, OKLCH off). Order in the UI is fixed; users who
    /// want a different order can edit the file directly.
    pub format_hex: bool,
    pub format_rgb: bool,
    pub format_hsl: bool,
    pub format_hsv: bool,
    pub format_oklch: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            format_hex: true,
            format_rgb: true,
            format_hsl: true,
            format_hsv: true,
            format_oklch: false,
        }
    }
}
